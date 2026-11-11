//! Process-local admission for one instance-owned runtime. This is acquired
//! before canonical inventory/device locks, never while holding those locks.
//! It is separate from the short-lived import-staging reservation.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeHold {
    RecoveryRequired,
    #[cfg(any(windows, test))] // Production publication remains Windows-only.
    DerivedRebuildRequired,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdmissionError {
    Busy,
    Held(RuntimeHold),
    WrongRuntime,
}

impl AdmissionError {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::Busy => "runtime_busy",
            Self::Held(RuntimeHold::RecoveryRequired) => "runtime_recovery_required",
            #[cfg(any(windows, test))]
            Self::Held(RuntimeHold::DerivedRebuildRequired) => "runtime_rebuild_required",
            Self::Held(RuntimeHold::Stopped) => "runtime_stopped",
            Self::WrongRuntime => "runtime_mismatch",
        }
    }
}

impl std::fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for AdmissionError {}

#[derive(Default)]
struct State {
    operations: usize,
    pending: bool,
    exclusive: bool,
    hold: Option<RuntimeHold>,
}

struct Inner {
    root: PathBuf,
    owner: Weak<File>,
    state: Mutex<State>,
    changed: Condvar,
    #[cfg(test)]
    _test_owner: Option<Arc<File>>,
}

#[derive(Clone)]
pub(crate) struct RuntimeAdmission(Arc<Inner>);

impl RuntimeAdmission {
    /// Bind domain work to this exact instance-owned admission allocation.
    /// A valid lease from another root or another incarnation is not authority
    /// for this runtime. Existing admitted work can still drain after stop.
    pub(crate) fn validate_operation(
        &self,
        root: &Path,
        permit: &OperationPermit,
    ) -> Result<(), AdmissionError> {
        if self.0.root != root || !Arc::ptr_eq(&self.0, &permit._lease.admission.0) {
            return Err(AdmissionError::WrongRuntime);
        }
        Ok(())
    }

    pub(crate) fn new(root: &Path, owner: Weak<File>) -> Self {
        Self(Arc::new(Inner {
            root: root.to_owned(),
            owner,
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
            #[cfg(test)]
            _test_owner: None,
        }))
    }

    pub(crate) fn enter(&self, root: &Path) -> Result<OperationPermit, AdmissionError> {
        if self.0.root != root {
            return Err(AdmissionError::WrongRuntime);
        }
        let mut state = self
            .0
            .state
            .lock()
            .map_err(|_| AdmissionError::Held(RuntimeHold::RecoveryRequired))?;
        if let Some(reason) = state.hold {
            return Err(AdmissionError::Held(reason));
        }
        if state.pending || state.exclusive {
            return Err(AdmissionError::Busy);
        }
        let owner = self
            .0
            .owner
            .upgrade()
            .ok_or(AdmissionError::Held(RuntimeHold::Stopped))?;
        state.operations += 1;
        Ok(OperationPermit {
            _lease: Arc::new(OperationLease {
                admission: self.clone(),
                _owner: Some(owner),
            }),
        })
    }

    /// Block new operations first, then drain existing leases. A deadline or
    /// competing import returns before any canonical mutation. No unbounded
    /// writer starvation and no admission mutex held during disk/network work.
    #[cfg(any(windows, test))]
    pub(crate) fn exclusive(
        &self,
        root: &Path,
        deadline: Instant,
        recovery: bool,
    ) -> Result<ExclusivePermit, AdmissionError> {
        if self.0.root != root {
            return Err(AdmissionError::WrongRuntime);
        }
        let mut state = self
            .0
            .state
            .lock()
            .map_err(|_| AdmissionError::Held(RuntimeHold::RecoveryRequired))?;
        if state.hold == Some(RuntimeHold::Stopped) || !recovery && state.hold.is_some() {
            return Err(AdmissionError::Held(state.hold.unwrap()));
        }
        if state.pending || state.exclusive {
            return Err(AdmissionError::Busy);
        }
        let owner = self
            .0
            .owner
            .upgrade()
            .ok_or(AdmissionError::Held(RuntimeHold::Stopped))?;
        state.pending = true;
        self.0.changed.notify_all();
        while state.operations != 0 {
            if Instant::now() >= deadline {
                state.pending = false;
                self.0.changed.notify_all();
                return Err(AdmissionError::Busy);
            }
            let (next, _) = self
                .0
                .changed
                .wait_timeout(state, deadline.saturating_duration_since(Instant::now()))
                .map_err(|_| AdmissionError::Held(RuntimeHold::RecoveryRequired))?;
            state = next;
            if state.hold == Some(RuntimeHold::Stopped) || !recovery && state.hold.is_some() {
                state.pending = false;
                self.0.changed.notify_all();
                return Err(AdmissionError::Held(state.hold.unwrap()));
            }
        }
        if Instant::now() >= deadline {
            state.pending = false;
            self.0.changed.notify_all();
            return Err(AdmissionError::Busy);
        }
        state.pending = false;
        state.exclusive = true;
        Ok(ExclusivePermit {
            admission: self.clone(),
            _owner: Some(owner),
            finished: false,
        })
    }

    pub(crate) fn stop(&self) {
        // Poison is never cleared; ordinary/exclusive admission stays refused.
        if let Ok(mut state) = self.0.state.lock() {
            state.hold = Some(RuntimeHold::Stopped);
        }
        self.0.changed.notify_all();
    }

    pub(crate) fn stop_and_drain(&self, deadline: Instant) -> Result<(), AdmissionError> {
        self.stop();
        let mut state = self
            .0
            .state
            .lock()
            .map_err(|_| AdmissionError::Held(RuntimeHold::RecoveryRequired))?;
        while state.operations != 0 || state.exclusive || state.pending {
            if Instant::now() >= deadline {
                return Err(AdmissionError::Busy);
            }
            let (next, _) = self
                .0
                .changed
                .wait_timeout(state, deadline.saturating_duration_since(Instant::now()))
                .map_err(|_| AdmissionError::Held(RuntimeHold::RecoveryRequired))?;
            state = next;
        }
        Ok(())
    }

    /// IPC authorization/parser fixtures do not run a daemon. Supply only a
    /// lifetime pin for their in-memory gate; real ownership tests use InstanceLock.
    #[cfg(test)]
    pub(crate) fn detached_for_tests(root: &Path) -> Self {
        let owner = Arc::new(File::open(std::env::current_exe().unwrap()).unwrap());
        Self(Arc::new(Inner {
            root: root.to_owned(),
            owner: Arc::downgrade(&owner),
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
            _test_owner: Some(owner),
        }))
    }
}

/// Clones retain the same operation across response serialization/write and
/// orphaned bounded workers. Only the last clone releases admission/instance.
#[derive(Clone)]
pub(crate) struct OperationPermit {
    _lease: Arc<OperationLease>,
}

impl Drop for OperationPermit {
    fn drop(&mut self) {
        // A worker may panic while the connection still retains another clone.
        // Latch here, rather than waiting for the last clone on a healthy thread.
        if std::thread::panicking() {
            if let Ok(mut state) = self._lease.admission.0.state.lock()
                && state.hold != Some(RuntimeHold::Stopped)
            {
                state.hold = Some(RuntimeHold::RecoveryRequired);
            }
            self._lease.admission.0.changed.notify_all();
        }
    }
}

struct OperationLease {
    admission: RuntimeAdmission,
    _owner: Option<Arc<File>>,
}

impl Drop for OperationLease {
    fn drop(&mut self) {
        // Release the file pin before advertising logical completion to a
        // shutdown waiter; otherwise observation can race field destruction.
        drop(self._owner.take());
        if let Ok(mut state) = self.admission.0.state.lock() {
            state.operations -= 1;
            if std::thread::panicking() && state.hold != Some(RuntimeHold::Stopped) {
                state.hold = Some(RuntimeHold::RecoveryRequired);
            }
        }
        self.admission.0.changed.notify_all();
    }
}

#[cfg(any(windows, test))]
pub(crate) struct ExclusivePermit {
    admission: RuntimeAdmission,
    _owner: Option<Arc<File>>,
    finished: bool,
}

#[cfg(any(windows, test))]
impl ExclusivePermit {
    /// Called only by transaction orchestration after verified recovery and
    /// rebuild, a clean pre-prepare rejection, or a classified failure.
    pub(crate) fn finish(mut self, hold: Option<RuntimeHold>) {
        drop(self._owner.take());
        if let Ok(mut state) = self.admission.0.state.lock() {
            if state.hold != Some(RuntimeHold::Stopped) {
                state.hold = hold;
            }
            state.exclusive = false;
        }
        self.finished = true;
        self.admission.0.changed.notify_all();
    }
}

#[cfg(any(windows, test))]
impl Drop for ExclusivePermit {
    fn drop(&mut self) {
        drop(self._owner.take());
        if !self.finished {
            if let Ok(mut state) = self.admission.0.state.lock() {
                if state.hold != Some(RuntimeHold::Stopped) {
                    state.hold = Some(RuntimeHold::RecoveryRequired);
                }
                state.exclusive = false;
            }
            self.admission.0.changed.notify_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance::try_acquire_instance;
    use std::time::Duration;

    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(3)
    }

    fn wait_pending(gate: &RuntimeAdmission) {
        let (state, timeout) = gate
            .0
            .changed
            .wait_timeout_while(gate.0.state.lock().unwrap(), Duration::from_secs(3), |s| {
                !s.pending
            })
            .unwrap();
        assert!(!timeout.timed_out() && state.pending);
    }

    #[test]
    fn exclusive_drains_all_clones_blocks_new_work_and_releases_only_on_finish() {
        let root = crate::test_temp::exclusive_temp_dir("admission-drain");
        let owner = try_acquire_instance(&root).unwrap();
        let gate = owner.admission();
        let first = gate.enter(&root).unwrap();
        let orphan = first.clone();
        let worker_gate = gate.clone();
        let worker_root = root.clone();
        let (send, recv) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            send.send(
                worker_gate
                    .exclusive(&worker_root, deadline(), false)
                    .unwrap(),
            )
            .unwrap();
        });
        wait_pending(&gate);
        assert!(matches!(gate.enter(&root), Err(AdmissionError::Busy)));
        assert!(matches!(
            gate.exclusive(&root, deadline(), false),
            Err(AdmissionError::Busy)
        ));
        drop(first);
        assert!(recv.try_recv().is_err());
        drop(orphan);
        let exclusive = recv.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(matches!(gate.enter(&root), Err(AdmissionError::Busy)));
        exclusive.finish(None);
        drop(gate.enter(&root).unwrap());
        worker.join().unwrap();
        drop(owner);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn timeout_and_foreign_runtime_never_start_a_transaction_or_clear_a_hold() {
        let root = crate::test_temp::exclusive_temp_dir("admission-timeout");
        let owner = try_acquire_instance(&root).unwrap();
        let gate = owner.admission();
        let permit = gate.enter(&root).unwrap();
        assert!(matches!(
            gate.exclusive(&root, Instant::now(), false),
            Err(AdmissionError::Busy)
        ));
        assert!(matches!(
            gate.enter(&root.join("other")),
            Err(AdmissionError::WrongRuntime)
        ));
        assert!(matches!(
            gate.exclusive(&root.join("other"), deadline(), true),
            Err(AdmissionError::WrongRuntime)
        ));
        drop(gate.enter(&root).unwrap());
        drop(permit);
        gate.exclusive(&root, deadline(), false)
            .unwrap()
            .finish(Some(RuntimeHold::DerivedRebuildRequired));
        assert!(matches!(
            gate.enter(&root),
            Err(AdmissionError::Held(RuntimeHold::DerivedRebuildRequired))
        ));
        assert!(matches!(
            gate.exclusive(&root, deadline(), false),
            Err(AdmissionError::Held(RuntimeHold::DerivedRebuildRequired))
        ));
        assert!(matches!(
            gate.exclusive(&root, Instant::now(), true),
            Err(AdmissionError::Busy)
        ));
        assert!(matches!(
            gate.enter(&root),
            Err(AdmissionError::Held(RuntimeHold::DerivedRebuildRequired))
        ));
        drop(owner);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn abandoned_exclusive_and_worker_panic_latch_recovery_even_with_live_clones() {
        let root = crate::test_temp::exclusive_temp_dir("admission-panic");
        let owner = try_acquire_instance(&root).unwrap();
        let gate = owner.admission();
        drop(gate.exclusive(&root, deadline(), false).unwrap());
        assert!(matches!(
            gate.enter(&root),
            Err(AdmissionError::Held(RuntimeHold::RecoveryRequired))
        ));
        gate.exclusive(&root, deadline(), true)
            .unwrap()
            .finish(None);
        let response = gate.enter(&root).unwrap();
        let worker = response.clone();
        assert!(
            std::thread::spawn(move || {
                let _permit = worker;
                panic!("synthetic worker failure");
            })
            .join()
            .is_err()
        );
        assert!(matches!(
            gate.enter(&root),
            Err(AdmissionError::Held(RuntimeHold::RecoveryRequired))
        ));
        drop(response);
        gate.exclusive(&root, deadline(), true)
            .unwrap()
            .finish(None);
        drop(gate.enter(&root).unwrap());
        drop(owner);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn instance_shutdown_refuses_new_work_and_pins_os_lock_until_orphans_exit() {
        let root = crate::test_temp::exclusive_temp_dir("admission-owner-exit");
        let owner = try_acquire_instance(&root).unwrap();
        let gate = owner.admission();
        let permit = gate.enter(&root).unwrap();
        assert_eq!(
            gate.stop_and_drain(Instant::now()),
            Err(AdmissionError::Busy)
        );
        drop(owner);
        assert!(matches!(
            gate.enter(&root),
            Err(AdmissionError::Held(RuntimeHold::Stopped))
        ));
        assert!(matches!(
            gate.exclusive(&root, deadline(), true),
            Err(AdmissionError::Held(RuntimeHold::Stopped))
        ));
        assert!(try_acquire_instance(&root).is_err());
        drop(permit);
        gate.stop_and_drain(deadline()).unwrap();
        drop(try_acquire_instance(&root).unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn poisoned_state_is_never_reopened_by_recovery() {
        let root = crate::test_temp::exclusive_temp_dir("admission-poison");
        let owner = try_acquire_instance(&root).unwrap();
        let gate = owner.admission();
        let worker = gate.clone();
        assert!(
            std::thread::spawn(move || {
                let _state = worker.0.state.lock().unwrap();
                panic!("synthetic state poison");
            })
            .join()
            .is_err()
        );
        assert!(matches!(
            gate.enter(&root),
            Err(AdmissionError::Held(RuntimeHold::RecoveryRequired))
        ));
        assert!(matches!(
            gate.exclusive(&root, deadline(), true),
            Err(AdmissionError::Held(RuntimeHold::RecoveryRequired))
        ));
        drop(owner);
        std::fs::remove_dir_all(root).unwrap();
    }
}
