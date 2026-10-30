//! Explicit daemon-owned Windows worker for an existing ready profile.
//! No discovery, provisioning, IPC authority, peer configuration or startup hook.
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::controller::{Control, ControllerError, Outcome, Status};
use super::native::{NativeConfigError, NativePorts};
use super::{ManagedProfile, ProfileError};
use crate::secure_store::{SecureStoreEnvironment, WindowsCredentialStore};
use crate::syncthing_package::{PackageError, PinnedSyncthingPackage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerError {
    Spawn,
    Panicked,
    StatusUnavailable,
    InvalidDeadline,
    ShutdownDeadline,
    Package(PackageError),
    Profile(ProfileError),
    Native(NativeConfigError),
    Controller(ControllerError),
}
impl std::fmt::Display for WorkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "managed_worker_{self:?}")
    }
}
impl std::error::Error for WorkerError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerState {
    Opening,
    Active,
    Stopping,
    Finished,
    Failed,
}

/// Fixed-size, in-process diagnostic only. Never trust/policy or durable state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub state: WorkerState,
    pub stop_requested: bool,
    pub controller: Option<Status>,
    pub transport_device_id: Option<String>,
    pub outcome: Option<Outcome>,
    pub error: Option<WorkerError>,
}
struct Shared {
    stop: AtomicBool,
    snapshot: Mutex<Snapshot>,
}
/// Read-only view: no stop, launch, paths or credentials. IPC receives only this
/// handle, never the owning worker. Nonblocking read respects request budgets.
#[derive(Clone)]
pub struct StatusReader(Arc<Shared>);
impl StatusReader {
    pub fn try_snapshot(&self) -> Result<Snapshot, WorkerError> {
        let mut snapshot = self
            .0
            .snapshot
            .try_lock()
            .map_err(|_| WorkerError::StatusUnavailable)?
            .clone();
        snapshot.stop_requested = self.0.stop.load(Ordering::Acquire);
        Ok(snapshot)
    }

    /// Returns the exact local ID observed on the authenticated owned REST
    /// socket, and only while that worker remains active and ready.
    pub fn try_transport_device_id(&self) -> Result<String, WorkerError> {
        let snapshot = self.try_snapshot()?;
        if snapshot.state != WorkerState::Active
            || snapshot.stop_requested
            || snapshot
                .controller
                .is_none_or(|status| status.phase != super::controller::Phase::LocalReady)
        {
            return Err(WorkerError::StatusUnavailable);
        }
        let identity = snapshot
            .transport_device_id
            .ok_or(WorkerError::StatusUnavailable)?;
        if !crate::syncthing_rest::native_id_shape(&identity) {
            return Err(WorkerError::StatusUnavailable);
        }
        Ok(identity)
    }
}
impl Shared {
    fn publish(
        &self,
        status: Status,
        transport_device_id: Option<&str>,
    ) -> Result<Control, WorkerError> {
        let stop = self.stop.load(Ordering::Acquire);
        let mut snapshot = self
            .snapshot
            .lock()
            .map_err(|_| WorkerError::StatusUnavailable)?;
        snapshot.controller = Some(status);
        snapshot.transport_device_id = if status.phase == super::controller::Phase::LocalReady {
            transport_device_id.map(str::to_owned)
        } else {
            None
        };
        snapshot.state = if stop {
            WorkerState::Stopping
        } else {
            WorkerState::Active
        };
        snapshot.stop_requested = stop;
        Ok(if stop {
            Control::Stop
        } else {
            Control::Continue
        })
    }
}

/// Dropping requests cancellation and waits at most three seconds, then leaves
/// the stopping thread owning its profile/package/job. It never frees those
/// locks to start a replacement behind an unresolved worker.
pub struct ManagedTransportWorker {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<Result<Outcome, WorkerError>>>,
    result: Option<Result<Outcome, WorkerError>>,
}
impl std::fmt::Debug for ManagedTransportWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ManagedTransportWorker(<private backend worker>)")
    }
}
impl ManagedTransportWorker {
    pub fn status_reader(&self) -> StatusReader {
        StatusReader(self.shared.clone())
    }
    /// Backend-selected paths only; all validation/handles and OS credentials
    /// live inside the worker. Missing profile/config/key refuses without repair.
    pub fn spawn_existing(
        profile: PathBuf,
        package: PathBuf,
        environment: SecureStoreEnvironment,
        ports: NativePorts,
    ) -> Result<Self, WorkerError> {
        Self::spawn_task(move |shared| {
            if shared.stop.load(Ordering::Acquire) {
                return Ok(Outcome::Cancelled);
            }
            let package = PinnedSyncthingPackage::open(&package).map_err(WorkerError::Package)?;
            let mut profile =
                ManagedProfile::open(&profile, environment).map_err(WorkerError::Profile)?;
            let store = WindowsCredentialStore::new();
            let mut prepared = profile
                .reopen_native(&package, &store, ports)
                .map_err(WorkerError::Native)?;
            let transport_device_id = prepared.configured_transport_device_id().to_owned();
            let mut status_failed = false;
            let result = prepared
                .supervise(&package, &store, |status| {
                    match shared.publish(
                        status,
                        (status.phase == super::controller::Phase::LocalReady)
                            .then_some(transport_device_id.as_str()),
                    ) {
                        Ok(control) => control,
                        Err(_) => {
                            status_failed = true;
                            Control::Stop
                        }
                    }
                })
                .map_err(WorkerError::Controller);
            if status_failed {
                Err(WorkerError::StatusUnavailable)
            } else {
                result
            }
        })
    }

    fn spawn_task(
        task: impl FnOnce(&Shared) -> Result<Outcome, WorkerError> + Send + 'static,
    ) -> Result<Self, WorkerError> {
        let shared = Arc::new(Shared {
            stop: AtomicBool::new(false),
            snapshot: Mutex::new(Snapshot {
                state: WorkerState::Opening,
                stop_requested: false,
                controller: None,
                transport_device_id: None,
                outcome: None,
                error: None,
            }),
        });
        let worker_shared = shared.clone();
        let thread = std::thread::Builder::new()
            .name("cryptex-managed-transport".into())
            .spawn(move || {
                // Unwinding drops native session ownership before reporting failure.
                // Do not install a panic hook or serialize panic payloads here.
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| task(&worker_shared)))
                        .unwrap_or(Err(WorkerError::Panicked));
                if let Ok(mut snapshot) = worker_shared.snapshot.lock() {
                    snapshot.state = if result.is_ok() {
                        WorkerState::Finished
                    } else {
                        WorkerState::Failed
                    };
                    snapshot.stop_requested = worker_shared.stop.load(Ordering::Acquire);
                    snapshot.outcome = result.as_ref().ok().copied();
                    snapshot.error = result.as_ref().err().copied();
                }
                result
            })
            .map_err(|_| WorkerError::Spawn)?;
        Ok(Self {
            shared,
            thread: Some(thread),
            result: None,
        })
    }

    pub fn snapshot(&self) -> Result<Snapshot, WorkerError> {
        let mut snapshot = self
            .shared
            .snapshot
            .lock()
            .map_err(|_| WorkerError::StatusUnavailable)?
            .clone();
        snapshot.stop_requested = self.shared.stop.load(Ordering::Acquire);
        Ok(snapshot)
    }
    pub fn request_stop(&self) -> Result<(), WorkerError> {
        self.shared.stop.store(true, Ordering::Release);
        let mut snapshot = self
            .shared
            .snapshot
            .lock()
            .map_err(|_| WorkerError::StatusUnavailable)?;
        snapshot.stop_requested = true;
        if !matches!(snapshot.state, WorkerState::Finished | WorkerState::Failed) {
            snapshot.state = WorkerState::Stopping;
        }
        Ok(())
    }

    /// Stop and join before releasing daemon/runtime ownership. Deadline failure
    /// retains this handle and permits another wait; it does not claim cleanup.
    pub fn shutdown(&mut self, budget: Duration) -> Result<Outcome, WorkerError> {
        if budget.is_zero() || budget > Duration::from_secs(45) {
            return Err(WorkerError::InvalidDeadline);
        }
        let status_error = self.request_stop().err();
        let outcome = self.wait(budget)?;
        if let Some(error) = status_error {
            Err(error)
        } else {
            Ok(outcome)
        }
    }
    fn wait(&mut self, budget: Duration) -> Result<Outcome, WorkerError> {
        if let Some(result) = self.result {
            return result;
        }
        let deadline = Instant::now() + budget;
        let thread = self.thread.as_ref().ok_or(WorkerError::Panicked)?;
        while !thread.is_finished() {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .filter(|d| !d.is_zero())
                .ok_or(WorkerError::ShutdownDeadline)?;
            std::thread::sleep(remaining.min(Duration::from_millis(10)));
        }
        // is_finished includes thread completion; joining cannot await task IO.
        let result = self
            .thread
            .take()
            .ok_or(WorkerError::Panicked)?
            .join()
            .unwrap_or(Err(WorkerError::Panicked));
        self.result = Some(result);
        result
    }
}
impl Drop for ManagedTransportWorker {
    fn drop(&mut self) {
        let _ = self.request_stop();
        if let Err(error) = self.wait(Duration::from_secs(3))
            && self.thread.is_some()
        {
            eprintln!(
                "component=managed_transport operation=worker_drop status=unresolved error={error}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_identity_is_available_only_for_an_active_ready_worker() {
        let shared = Arc::new(Shared {
            stop: AtomicBool::new(false),
            snapshot: Mutex::new(Snapshot {
                state: WorkerState::Active,
                stop_requested: false,
                controller: Some(Status {
                    phase: super::super::controller::Phase::LocalReady,
                    attempt: 1,
                    pid: Some(42),
                    error: None,
                }),
                transport_device_id: Some(
                    "AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA".into(),
                ),
                outcome: None,
                error: None,
            }),
        });
        let reader = StatusReader(shared.clone());
        assert_eq!(
            reader.try_transport_device_id().unwrap(),
            "AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA"
        );
        shared.snapshot.lock().unwrap().state = WorkerState::Stopping;
        assert_eq!(
            reader.try_transport_device_id(),
            Err(WorkerError::StatusUnavailable)
        );
    }
    #[test]
    fn cancellation_and_join_are_idempotent() {
        let mut worker = ManagedTransportWorker::spawn_task(|s| {
            while !s.stop.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(1));
            }
            Ok(Outcome::Cancelled)
        })
        .unwrap();
        assert_eq!(
            worker.shutdown(Duration::from_secs(1)),
            Ok(Outcome::Cancelled)
        );
        assert_eq!(
            worker.shutdown(Duration::from_secs(1)),
            Ok(Outcome::Cancelled)
        );
        assert_eq!(worker.snapshot().unwrap().state, WorkerState::Finished);
        assert!(worker.thread.is_none());
    }
    #[test]
    fn deadline_retains_worker_for_later_join() {
        let (release, receiver) = std::sync::mpsc::channel();
        let mut worker = ManagedTransportWorker::spawn_task(move |_| {
            receiver.recv().unwrap();
            Ok(Outcome::Cancelled)
        })
        .unwrap();
        assert_eq!(
            worker.shutdown(Duration::from_millis(10)),
            Err(WorkerError::ShutdownDeadline)
        );
        assert!(worker.thread.is_some());
        release.send(()).unwrap();
        assert_eq!(
            worker.shutdown(Duration::from_secs(1)),
            Ok(Outcome::Cancelled)
        );
    }
    #[test]
    fn errors_stay_typed_and_terminal() {
        let mut worker = ManagedTransportWorker::spawn_task(|_| {
            Err(WorkerError::Profile(ProfileError::InvalidState))
        })
        .unwrap();
        assert_eq!(
            worker.shutdown(Duration::from_secs(1)),
            Err(WorkerError::Profile(ProfileError::InvalidState))
        );
        let snapshot = worker.snapshot().unwrap();
        assert_eq!(snapshot.state, WorkerState::Failed);
        assert_eq!(snapshot.outcome, None);
    }
    #[test]
    fn invalid_deadlines_do_not_cancel() {
        let mut worker = ManagedTransportWorker::spawn_task(|_| Ok(Outcome::Cancelled)).unwrap();
        assert_eq!(
            worker.shutdown(Duration::ZERO),
            Err(WorkerError::InvalidDeadline)
        );
        assert!(!worker.shared.stop.load(Ordering::Acquire));
        assert_eq!(
            worker.shutdown(Duration::from_secs(46)),
            Err(WorkerError::InvalidDeadline)
        );
        assert_eq!(
            worker.shutdown(Duration::from_secs(1)),
            Ok(Outcome::Cancelled)
        );
    }

    #[test]
    fn cancellation_is_sticky_when_controller_publishes_again() {
        let mut worker = ManagedTransportWorker::spawn_task(|s| {
            while !s.stop.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(1));
            }
            assert_eq!(
                s.publish(
                    Status {
                        phase: super::super::controller::Phase::Starting,
                        attempt: 1,
                        pid: None,
                        error: None
                    },
                    None
                )?,
                Control::Stop
            );
            assert_eq!(s.snapshot.lock().unwrap().state, WorkerState::Stopping);
            Ok(Outcome::Cancelled)
        })
        .unwrap();
        assert_eq!(
            worker.shutdown(Duration::from_secs(1)),
            Ok(Outcome::Cancelled)
        );
        assert!(worker.snapshot().unwrap().stop_requested);
        assert_eq!(worker.snapshot().unwrap().state, WorkerState::Finished);
    }

    #[test]
    fn panic_is_typed_failure_and_joined() {
        let mut worker =
            ManagedTransportWorker::spawn_task(|_| panic!("synthetic worker panic")).unwrap();
        assert_eq!(
            worker.shutdown(Duration::from_secs(1)),
            Err(WorkerError::Panicked)
        );
        assert_eq!(
            worker.snapshot().unwrap().error,
            Some(WorkerError::Panicked)
        );
        assert!(worker.thread.is_none());
    }

    #[test]
    fn poisoned_diagnostics_still_cancel_and_join() {
        let mut worker = ManagedTransportWorker::spawn_task(|s| {
            let _guard = s.snapshot.lock().unwrap();
            panic!("synthetic diagnostic poison");
        })
        .unwrap();
        // Wait without joining so request_stop also exercises the poisoned lock.
        let deadline = Instant::now() + Duration::from_secs(1);
        while !worker.thread.as_ref().unwrap().is_finished() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            worker.shutdown(Duration::from_secs(1)),
            Err(WorkerError::Panicked)
        );
        assert!(worker.thread.is_none());
        assert!(worker.shared.stop.load(Ordering::Acquire));
        assert_eq!(worker.snapshot(), Err(WorkerError::StatusUnavailable));
    }
}
