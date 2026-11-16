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
    #[cfg(any(windows, target_os = "macos", test))]
    IncomingPolicyConflict,
    #[cfg(any(windows, target_os = "macos", test))] // Internal publication remains gated.
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
            #[cfg(any(windows, target_os = "macos", test))]
            Self::Held(RuntimeHold::IncomingPolicyConflict) => "runtime_policy_conflict",
            #[cfg(any(windows, target_os = "macos", test))]
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
    transport_epoch: u64,
}

struct Inner {
    root: PathBuf,
    owner: Weak<File>,
    state: Mutex<State>,
    changed: Condvar,
    #[cfg(target_os = "macos")]
    namespace: std::sync::OnceLock<crate::import_staging::macos_fs::RuntimeIdentity>,
    #[cfg(target_os = "macos")]
    namespace_failed: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    _test_owner: Option<Arc<File>>,
}

#[derive(Clone)]
pub(crate) struct RuntimeAdmission(Arc<Inner>);

impl RuntimeAdmission {
    /// Publish the original native capability set before exposing any workers.
    /// Clones share later verified device pins; no caller can refresh the root.
    #[cfg(all(target_os = "macos", any(feature = "m8-local-startup-check", test)))]
    pub(crate) fn bind_namespace(
        &self,
        namespace: crate::import_staging::macos_fs::RuntimeIdentity,
    ) -> Result<(), AdmissionError> {
        if !namespace.belongs_to(&self.0.root) {
            return Err(AdmissionError::WrongRuntime);
        }
        self.0
            .namespace
            .set(namespace)
            .map_err(|_| AdmissionError::WrongRuntime)?;
        self.check_namespace()
    }

    /// This check also applies to an already admitted, asynchronously queued
    /// worker and its actual reply. A detected replacement is a monotone hold.
    pub(crate) fn check_namespace(&self) -> Result<(), AdmissionError> {
        #[cfg(target_os = "macos")]
        if self
            .0
            .namespace_failed
            .load(std::sync::atomic::Ordering::Acquire)
            || self
                .0
                .namespace
                .get()
                .is_some_and(|namespace| namespace.validate().is_err())
        {
            self.0
                .namespace_failed
                .store(true, std::sync::atomic::Ordering::Release);
            if let Ok(mut state) = self.0.state.lock()
                && state.hold != Some(RuntimeHold::Stopped)
            {
                state.hold = Some(RuntimeHold::RecoveryRequired);
            }
            self.0.changed.notify_all();
            return Err(AdmissionError::Held(RuntimeHold::RecoveryRequired));
        }
        Ok(())
    }
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
        self.check_namespace()
    }

    pub(crate) fn new(root: &Path, owner: Weak<File>) -> Self {
        Self(Arc::new(Inner {
            root: root.to_owned(),
            owner,
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
            #[cfg(target_os = "macos")]
            namespace: std::sync::OnceLock::new(),
            #[cfg(target_os = "macos")]
            namespace_failed: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            _test_owner: None,
        }))
    }

    pub(crate) fn enter(&self, root: &Path) -> Result<OperationPermit, AdmissionError> {
        if self.0.root != root {
            return Err(AdmissionError::WrongRuntime);
        }
        self.check_namespace()?;
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
    #[cfg(any(windows, target_os = "macos", test))]
    pub(crate) fn exclusive(
        &self,
        root: &Path,
        deadline: Instant,
        recovery: bool,
    ) -> Result<ExclusivePermit, AdmissionError> {
        if self.0.root != root {
            return Err(AdmissionError::WrongRuntime);
        }
        self.check_namespace()?;
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
        // Existing transport authority never revives after any reconcile attempt,
        // even when that attempt times out before acquiring exclusive ownership.
        state.transport_epoch = state.transport_epoch.checked_add(1).ok_or_else(|| {
            state.pending = false;
            state.hold = Some(RuntimeHold::RecoveryRequired);
            self.0.changed.notify_all();
            AdmissionError::Held(RuntimeHold::RecoveryRequired)
        })?;
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

    /// Close new admission before releasing the real reply lease after an
    /// operation that may have changed canonical policy. Existing holds are
    /// monotone here; only full verified recovery/rebuild may clear them.
    #[cfg(any(
        windows,
        all(target_os = "macos", feature = "m8-local-startup-check"),
        test
    ))]
    pub(crate) fn require_derived_rebuild(
        &self,
        root: &Path,
        permit: &OperationPermit,
    ) -> Result<(), AdmissionError> {
        self.validate_operation(root, permit)?;
        let mut state = self
            .0
            .state
            .lock()
            .map_err(|_| AdmissionError::Held(RuntimeHold::RecoveryRequired))?;
        if let Some(hold) = state.hold {
            return Err(AdmissionError::Held(hold));
        }
        state.hold = Some(RuntimeHold::DerivedRebuildRequired);
        self.0.changed.notify_all();
        Ok(())
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
            #[cfg(target_os = "macos")]
            namespace: std::sync::OnceLock::new(),
            #[cfg(target_os = "macos")]
            namespace_failed: std::sync::atomic::AtomicBool::new(false),
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

impl OperationPermit {
    pub(crate) fn check_namespace(&self) -> Result<(), AdmissionError> {
        self._lease.admission.check_namespace()
    }
}

impl Drop for OperationPermit {
    fn drop(&mut self) {
        // Late workers retain their own lease even after the caller timed out.
        let _ = self.check_namespace(); // Failure latches recovery in the gate.
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

#[cfg(any(windows, target_os = "macos", test))]
pub(crate) struct ExclusivePermit {
    admission: RuntimeAdmission,
    _owner: Option<Arc<File>>,
    finished: bool,
}

/// Live-session cancellation authority only, not canonical peer authority. It
/// survives release of snapshot locks but cannot authorize another installation.
#[cfg(any(
    windows,
    all(target_os = "macos", feature = "m8-local-startup-check"),
    test
))]
#[derive(Clone)]
pub(crate) struct TransportGuard {
    admission: RuntimeAdmission,
    epoch: u64,
    // Retain OS exclusion while a cancelled worker (or its status view) drains.
    // InstanceLock::drop still latches Stopped; ownership never grants readiness.
    _owner: Arc<File>,
}
#[cfg(any(
    windows,
    all(target_os = "macos", feature = "m8-local-startup-check"),
    test
))]
impl TransportGuard {
    pub(crate) fn check(&self) -> Result<(), AdmissionError> {
        self.admission.check_namespace()?;
        let state = self
            .admission
            .0
            .state
            .lock()
            .map_err(|_| AdmissionError::Held(RuntimeHold::RecoveryRequired))?;
        if let Some(hold) = state.hold {
            return Err(AdmissionError::Held(hold));
        }
        if state.pending || state.transport_epoch != self.epoch {
            return Err(AdmissionError::Busy);
        }
        if self.admission.0.owner.upgrade().is_none() {
            return Err(AdmissionError::Held(RuntimeHold::Stopped));
        }
        Ok(())
    }
}

#[cfg(any(windows, target_os = "macos", test))]
impl ExclusivePermit {
    #[cfg(any(
        windows,
        all(target_os = "macos", feature = "m8-local-startup-check"),
        test
    ))]
    pub(crate) fn transport_guard(&self, root: &Path) -> Result<TransportGuard, AdmissionError> {
        if self.admission.0.root != root {
            return Err(AdmissionError::WrongRuntime);
        }
        self.admission.check_namespace()?;
        let state = self
            .admission
            .0
            .state
            .lock()
            .map_err(|_| AdmissionError::Held(RuntimeHold::RecoveryRequired))?;
        if !state.exclusive || self.finished {
            return Err(AdmissionError::Busy);
        }
        if let Some(hold) = state.hold {
            return Err(AdmissionError::Held(hold));
        }
        Ok(TransportGuard {
            admission: self.admission.clone(),
            epoch: state.transport_epoch,
            _owner: self
                .admission
                .0
                .owner
                .upgrade()
                .ok_or(AdmissionError::Held(RuntimeHold::Stopped))?,
        })
    }
    /// Called only by transaction orchestration after verified recovery and
    /// rebuild, a clean pre-prepare rejection, or a classified failure.
    pub(crate) fn finish(mut self, hold: Option<RuntimeHold>) {
        let namespace_valid = self.admission.check_namespace().is_ok();
        drop(self._owner.take());
        if let Ok(mut state) = self.admission.0.state.lock() {
            if state.hold != Some(RuntimeHold::Stopped) {
                state.hold = if namespace_valid {
                    hold
                } else {
                    Some(RuntimeHold::RecoveryRequired)
                };
            }
            state.exclusive = false;
        }
        self.finished = true;
        self.admission.0.changed.notify_all();
    }
}

#[cfg(any(windows, target_os = "macos", test))]
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

    #[test]
    fn transport_guard_never_revives_after_reconcile_timeout_or_new_epoch() {
        let root = crate::test_temp::exclusive_temp_dir("transport-admission-epoch");
        let owner = try_acquire_instance(&root).unwrap();
        let gate = owner.admission();
        let permit = gate.exclusive(&root, deadline(), false).unwrap();
        assert!(matches!(
            permit.transport_guard(&root.join("alien")),
            Err(AdmissionError::WrongRuntime)
        ));
        let guard = permit.transport_guard(&root).unwrap();
        guard.check().unwrap();
        permit.finish(None);
        guard.check().unwrap();
        let active = gate.enter(&root).unwrap();
        assert!(matches!(
            gate.exclusive(&root, Instant::now(), false),
            Err(AdmissionError::Busy)
        ));
        assert!(guard.check().is_err());
        drop(active);
        let next = gate.exclusive(&root, deadline(), false).unwrap();
        let fresh = next.transport_guard(&root).unwrap();
        next.finish(None);
        fresh.check().unwrap();
        assert!(guard.check().is_err());
        drop(owner);
        assert!(fresh.check().is_err());
        assert!(try_acquire_instance(&root).is_err());
        drop((guard, fresh));
        drop(try_acquire_instance(&root).unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn transport_guard_refuses_holds_and_unfinished_installation() {
        let root = crate::test_temp::exclusive_temp_dir("transport-admission-hold");
        let owner = try_acquire_instance(&root).unwrap();
        let gate = owner.admission();
        let permit = gate.exclusive(&root, deadline(), false).unwrap();
        let guard = permit.transport_guard(&root).unwrap();
        drop(permit);
        assert_eq!(
            guard.check(),
            Err(AdmissionError::Held(RuntimeHold::RecoveryRequired))
        );
        let recovery = gate.exclusive(&root, deadline(), true).unwrap();
        assert!(recovery.transport_guard(&root).is_err());
        recovery.finish(None);
        assert!(guard.check().is_err());
        let permit = gate.exclusive(&root, deadline(), false).unwrap();
        let fresh = permit.transport_guard(&root).unwrap();
        permit.finish(Some(RuntimeHold::IncomingPolicyConflict));
        assert_eq!(
            fresh.check(),
            Err(AdmissionError::Held(RuntimeHold::IncomingPolicyConflict))
        );
        drop(owner);
        drop((guard, fresh));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn original_namespace_cannot_rebind_or_clear_drift_through_recovery() {
        use crate::import_staging::macos_fs::RuntimeIdentity;
        let root = crate::test_temp::exclusive_temp_dir("namespace-admission");
        let admission = RuntimeAdmission::detached_for_tests(&root);
        admission
            .bind_namespace(RuntimeIdentity::capture(&root).unwrap())
            .unwrap();
        assert_eq!(
            admission.bind_namespace(RuntimeIdentity::capture(&root).unwrap()),
            Err(AdmissionError::WrongRuntime)
        );
        let recovery = admission.exclusive(&root, deadline(), true).unwrap();
        let preserved = root.with_extension("original");
        std::fs::rename(&root, &preserved).unwrap();
        std::fs::create_dir(&root).unwrap();
        recovery.finish(None);
        std::fs::remove_dir(&root).unwrap();
        std::fs::rename(preserved, &root).unwrap();
        assert!(matches!(
            admission.enter(&root),
            Err(AdmissionError::Held(RuntimeHold::RecoveryRequired))
        ));
        assert!(matches!(
            admission.exclusive(&root, deadline(), true),
            Err(AdmissionError::Held(RuntimeHold::RecoveryRequired))
        ));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn canonical_reply_closes_new_admission_before_its_real_lease_is_released() {
        let root = crate::test_temp::exclusive_temp_dir("admission-domain-rebuild");
        let owner = try_acquire_instance(&root).unwrap();
        let gate = owner.admission();
        let reply = gate.enter(&root).unwrap();
        let alien = RuntimeAdmission::detached_for_tests(&root);
        let foreign = alien.enter(&root).unwrap();
        assert_eq!(
            gate.require_derived_rebuild(&root, &foreign),
            Err(AdmissionError::WrongRuntime)
        );
        gate.require_derived_rebuild(&root, &reply).unwrap();
        assert!(matches!(
            gate.enter(&root),
            Err(AdmissionError::Held(RuntimeHold::DerivedRebuildRequired))
        ));
        assert!(matches!(
            gate.exclusive(&root, Instant::now(), true),
            Err(AdmissionError::Busy)
        ));
        drop(reply);
        assert!(matches!(
            gate.enter(&root),
            Err(AdmissionError::Held(RuntimeHold::DerivedRebuildRequired))
        ));
        let exclusive = gate.exclusive(&root, deadline(), true).unwrap();
        exclusive.finish(None); // Test-only stand-in for the verified orchestrator.
        assert!(gate.enter(&root).is_ok());
        drop(owner);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn incoming_policy_hold_denies_all_shared_ordinary_admission() {
        let root = crate::test_temp::exclusive_temp_dir("admission-policy-hold");
        let owner = try_acquire_instance(&root).unwrap();
        let gate = owner.admission();
        let clone = gate.clone();
        gate.exclusive(&root, deadline(), false)
            .unwrap()
            .finish(Some(RuntimeHold::IncomingPolicyConflict));
        for shared in [gate, clone] {
            let error = shared.enter(&root).err().unwrap();
            assert_eq!(
                error,
                AdmissionError::Held(RuntimeHold::IncomingPolicyConflict)
            );
            assert_eq!(error.code(), "runtime_policy_conflict");
        }
        drop(owner);
        std::fs::remove_dir_all(root).unwrap();
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
