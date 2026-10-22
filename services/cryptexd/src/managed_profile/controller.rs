//! Blocking backend supervision of an already prepared, isolated native home.
//! No startup/IPC hook, peer configuration or trust/protection claim.
use std::time::Duration;

use super::native::NativeConfigError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Starting,
    LocalReady,
    RestartWaiting,
    Stopped,
    Failed,
}

/// Public backend metadata only. LocalReady means owned local API readiness;
/// it says nothing about peers, canonical admission or protected replication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    pub phase: Phase,
    pub attempt: u8,
    pub pid: Option<u32>,
    pub error: Option<ControllerError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Continue,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// No live session when cancellation was requested. Not a clean-exit claim.
    Cancelled,
    /// Authenticated shutdown, full owned-tree exit and native log checks passed.
    CleanShutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerError {
    Native(NativeConfigError),
    RestartLimit,
}
impl std::fmt::Display for ControllerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "managed_controller_{self:?}")
    }
}
impl std::error::Error for ControllerError {}

trait Session {
    fn pid(&self) -> u32;
    /// True only after the whole owned job has exited. No external PID adoption.
    fn exited(&mut self) -> Result<bool, NativeConfigError>;
    fn observe(&mut self) -> Result<(), NativeConfigError>;
    fn stop(&mut self) -> Result<(), NativeConfigError>;
}
trait Driver {
    type Running<'a>: Session
    where
        Self: 'a;
    fn start(&mut self) -> Result<Self::Running<'_>, NativeConfigError>;
}

// Three replacement starts maximum over this controller invocation. A healthy
// interval never resets the budget. Only a confirmed whole-job exit retries;
// start/credential/ownership/health/shutdown failures are terminal.
fn run<D: Driver>(
    driver: &mut D,
    mut control: impl FnMut(Status) -> Control,
    mut wait: impl FnMut(Duration),
) -> Result<Outcome, ControllerError> {
    let mut attempt = 1;
    let result = (|| {
        loop {
            if control(Status {
                phase: Phase::Starting,
                attempt,
                pid: None,
                error: None,
            }) == Control::Stop
            {
                return Ok(Outcome::Cancelled);
            }
            {
                let mut session = driver.start().map_err(ControllerError::Native)?;
                loop {
                    if session.exited().map_err(ControllerError::Native)? {
                        break;
                    }
                    session.observe().map_err(ControllerError::Native)?;
                    if control(Status {
                        phase: Phase::LocalReady,
                        attempt,
                        pid: Some(session.pid()),
                        error: None,
                    }) == Control::Stop
                    {
                        session.stop().map_err(ControllerError::Native)?;
                        return Ok(Outcome::CleanShutdown);
                    }
                    wait(Duration::from_millis(250));
                }
                // Session destruction precedes backoff and replacement. The
                // native driver retains/poisons exclusion on unresolved cleanup.
            }
            if attempt == 4 {
                return Err(ControllerError::RestartLimit);
            }
            // Fixed 1/2/4 seconds, interruptible by backend cancellation every
            // 100ms; independent of wall clock and remote input.
            for _ in 0..(10_u32 << (attempt - 1)) {
                if control(Status {
                    phase: Phase::RestartWaiting,
                    attempt,
                    pid: None,
                    error: None,
                }) == Control::Stop
                {
                    return Ok(Outcome::Cancelled);
                }
                wait(Duration::from_millis(100));
            }
            attempt += 1;
        }
    })();
    let _ = control(Status {
        phase: if result.is_ok() {
            Phase::Stopped
        } else {
            Phase::Failed
        },
        attempt,
        pid: None,
        error: result.as_ref().err().copied(),
    });
    result
}

#[cfg(windows)]
mod windows {
    use super::*;
    use crate::managed_profile::native::{NativeSession, PreparedNativeProfile};
    use crate::secure_store::SecureStore;
    use crate::syncthing_package::PinnedSyncthingPackage;

    struct NativeDriver<'a, 'profile, S: SecureStore + ?Sized> {
        prepared: &'a mut PreparedNativeProfile<'profile>,
        package: &'a PinnedSyncthingPackage,
        store: &'a S,
    }
    struct Running<'a, 'profile, S: SecureStore + ?Sized> {
        session: NativeSession<'a, 'profile>,
        store: &'a S,
    }
    impl<S: SecureStore + ?Sized> Session for Running<'_, '_, S> {
        fn pid(&self) -> u32 {
            self.session.pid()
        }
        fn exited(&mut self) -> Result<bool, NativeConfigError> {
            Ok(self.session.try_exit()?.is_some())
        }
        fn observe(&mut self) -> Result<(), NativeConfigError> {
            self.session.observe(self.store).map(|_| ())
        }
        fn stop(&mut self) -> Result<(), NativeConfigError> {
            self.session.stop(self.store).map(|_| ())
        }
    }
    impl<'profile, S: SecureStore + ?Sized> Driver for NativeDriver<'_, 'profile, S> {
        type Running<'a>
            = Running<'a, 'profile, S>
        where
            Self: 'a;
        fn start(&mut self) -> Result<Self::Running<'_>, NativeConfigError> {
            Ok(Running {
                session: self.prepared.start(self.package, self.store)?,
                store: self.store,
            })
        }
    }
    impl PreparedNativeProfile<'_> {
        /// Run on a dedicated blocking backend worker. Callback is trusted local
        /// scheduling/cancellation, never an IPC or renderer-supplied closure.
        /// Every start revalidates private config, identity and OS credentials.
        /// No automatic provisioning, profile repair, peer or folder addition.
        pub fn supervise<S: SecureStore + ?Sized>(
            &mut self,
            package: &PinnedSyncthingPackage,
            store: &S,
            control: impl FnMut(Status) -> Control,
        ) -> Result<Outcome, ControllerError> {
            run(
                &mut NativeDriver {
                    prepared: self,
                    package,
                    store,
                },
                control,
                std::thread::sleep,
            )
        }

        /// Actual owned-job exits between healthy observations, for the explicit
        /// synthetic native fixture only. Uses production scheduling and driver;
        /// no artificial wait, PID termination or launch-guard bypass.
        #[cfg(test)]
        pub(crate) fn supervise_with_test_crashes<S: SecureStore + ?Sized>(
            &mut self,
            package: &PinnedSyncthingPackage,
            store: &S,
            crashes: u8,
            control: impl FnMut(Status) -> Control,
        ) -> Result<Outcome, ControllerError> {
            run(
                &mut FaultDriver {
                    inner: NativeDriver {
                        prepared: self,
                        package,
                        store,
                    },
                    remaining: crashes,
                },
                control,
                std::thread::sleep,
            )
        }
    }

    #[cfg(test)]
    struct FaultDriver<'a, 'profile, S: SecureStore + ?Sized> {
        inner: NativeDriver<'a, 'profile, S>,
        remaining: u8,
    }
    #[cfg(test)]
    struct FaultRunning<'a, 'profile, S: SecureStore + ?Sized> {
        inner: Running<'a, 'profile, S>,
        crash: bool,
        observed: bool,
    }
    #[cfg(test)]
    impl<'profile, S: SecureStore + ?Sized> Driver for FaultDriver<'_, 'profile, S> {
        type Running<'a>
            = FaultRunning<'a, 'profile, S>
        where
            Self: 'a;
        fn start(&mut self) -> Result<Self::Running<'_>, NativeConfigError> {
            let crash = self.remaining > 0;
            self.remaining = self.remaining.saturating_sub(1);
            Ok(FaultRunning {
                inner: self.inner.start()?,
                crash,
                observed: false,
            })
        }
    }
    #[cfg(test)]
    impl<S: SecureStore + ?Sized> Session for FaultRunning<'_, '_, S> {
        fn pid(&self) -> u32 {
            self.inner.pid()
        }
        fn exited(&mut self) -> Result<bool, NativeConfigError> {
            if self.crash && self.observed {
                self.crash = false;
                self.inner.session.crash_owned_job_for_test()?;
            }
            self.inner.exited()
        }
        fn observe(&mut self) -> Result<(), NativeConfigError> {
            self.inner.observe()?;
            self.observed = true;
            Ok(())
        }
        fn stop(&mut self) -> Result<(), NativeConfigError> {
            self.inner.stop()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::VecDeque, rc::Rc};
    #[derive(Default)]
    struct Trace {
        starts: u8,
        stops: u8,
        drops: u8,
        waits: Duration,
    }
    struct Fake {
        exits: VecDeque<bool>,
        fail_start: bool,
        fail_observe: bool,
        fail_stop: bool,
        trace: Rc<RefCell<Trace>>,
    }
    struct FakeSession {
        exited: bool,
        observe: bool,
        stop: bool,
        trace: Rc<RefCell<Trace>>,
    }
    impl Drop for FakeSession {
        fn drop(&mut self) {
            self.trace.borrow_mut().drops += 1;
        }
    }
    impl Session for FakeSession {
        fn pid(&self) -> u32 {
            123
        }
        fn exited(&mut self) -> Result<bool, NativeConfigError> {
            Ok(self.exited)
        }
        fn observe(&mut self) -> Result<(), NativeConfigError> {
            if self.observe {
                Err(NativeConfigError::IdentityChanged)
            } else {
                Ok(())
            }
        }
        fn stop(&mut self) -> Result<(), NativeConfigError> {
            self.trace.borrow_mut().stops += 1;
            if self.stop {
                Err(NativeConfigError::UncleanShutdown)
            } else {
                Ok(())
            }
        }
    }
    impl Driver for Fake {
        type Running<'a> = FakeSession;
        fn start(&mut self) -> Result<FakeSession, NativeConfigError> {
            let mut trace = self.trace.borrow_mut();
            assert_eq!(
                trace.starts, trace.drops,
                "replacement before session released"
            );
            trace.starts += 1;
            if self.fail_start {
                return Err(NativeConfigError::CredentialChanged);
            }
            Ok(FakeSession {
                exited: self.exits.pop_front().unwrap_or(false),
                observe: self.fail_observe,
                stop: self.fail_stop,
                trace: self.trace.clone(),
            })
        }
    }
    fn fake(exits: &[bool]) -> Fake {
        Fake {
            exits: exits.iter().copied().collect(),
            fail_start: false,
            fail_observe: false,
            fail_stop: false,
            trace: Rc::default(),
        }
    }
    fn execute(
        fake: &mut Fake,
        mut callback: impl FnMut(Status) -> Control,
    ) -> (Result<Outcome, ControllerError>, Vec<Status>) {
        let trace = fake.trace.clone();
        let mut statuses = vec![];
        let result = run(
            fake,
            |s| {
                statuses.push(s);
                callback(s)
            },
            |d| trace.borrow_mut().waits += d,
        );
        (result, statuses)
    }
    fn stop_ready(s: Status) -> Control {
        if s.phase == Phase::LocalReady {
            Control::Stop
        } else {
            Control::Continue
        }
    }
    #[test]
    fn clean_stop_requires_session_stop_and_releases_before_final_status() {
        let mut fake = fake(&[false]);
        let (result, statuses) = execute(&mut fake, stop_ready);
        assert_eq!(result, Ok(Outcome::CleanShutdown));
        assert_eq!(fake.trace.borrow().stops, 1);
        assert_eq!(fake.trace.borrow().drops, 1);
        assert_eq!(statuses.last().unwrap().phase, Phase::Stopped);
        assert!(statuses.last().unwrap().pid.is_none());
    }
    #[test]
    fn confirmed_exit_restarts_after_release_and_backoff() {
        let mut fake = fake(&[true, true, false]);
        assert_eq!(execute(&mut fake, stop_ready).0, Ok(Outcome::CleanShutdown));
        assert_eq!(fake.trace.borrow().starts, 3);
        assert_eq!(fake.trace.borrow().waits, Duration::from_secs(3));
    }
    #[test]
    fn repeated_exits_exhaust_budget_without_clean_claim() {
        let mut fake = fake(&[true; 4]);
        let (result, statuses) = execute(&mut fake, |_| Control::Continue);
        assert_eq!(result, Err(ControllerError::RestartLimit));
        assert_eq!(fake.trace.borrow().starts, 4);
        assert_eq!(fake.trace.borrow().waits, Duration::from_secs(7));
        assert_eq!(
            statuses.last().unwrap().error,
            Some(ControllerError::RestartLimit)
        );
    }
    #[test]
    fn cancel_before_start_does_not_launch_or_claim_clean_shutdown() {
        let mut fake = fake(&[]);
        assert_eq!(
            execute(&mut fake, |_| Control::Stop).0,
            Ok(Outcome::Cancelled)
        );
        assert_eq!(fake.trace.borrow().starts, 0);
    }
    #[test]
    fn cancel_backoff_does_not_launch_replacement() {
        let mut fake = fake(&[true]);
        assert_eq!(
            execute(&mut fake, |s| if s.phase == Phase::RestartWaiting {
                Control::Stop
            } else {
                Control::Continue
            })
            .0,
            Ok(Outcome::Cancelled)
        );
        assert_eq!(fake.trace.borrow().starts, 1);
        assert_eq!(fake.trace.borrow().waits, Duration::ZERO);
    }
    #[test]
    fn start_validation_error_is_terminal() {
        let mut fake = fake(&[]);
        fake.fail_start = true;
        assert_eq!(
            execute(&mut fake, stop_ready).0,
            Err(ControllerError::Native(
                NativeConfigError::CredentialChanged
            ))
        );
        assert_eq!(fake.trace.borrow().starts, 1);
    }
    #[test]
    fn health_identity_error_is_terminal_and_session_released() {
        let mut fake = fake(&[false]);
        fake.fail_observe = true;
        let (result, statuses) = execute(&mut fake, stop_ready);
        assert_eq!(
            result,
            Err(ControllerError::Native(NativeConfigError::IdentityChanged))
        );
        assert_eq!(fake.trace.borrow().starts, 1);
        assert_eq!(fake.trace.borrow().drops, 1);
        assert!(!statuses.iter().any(|s| s.phase == Phase::LocalReady));
    }
    #[test]
    fn failed_shutdown_never_reports_clean() {
        let mut fake = fake(&[false]);
        fake.fail_stop = true;
        assert_eq!(
            execute(&mut fake, stop_ready).0,
            Err(ControllerError::Native(NativeConfigError::UncleanShutdown))
        );
        assert_eq!(fake.trace.borrow().starts, 1);
        assert_eq!(fake.trace.borrow().drops, 1);
    }
}
