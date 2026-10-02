//! Windows platform adapters for P0-2 Mode A comparison harnesses.
//!
//! Production registration remains [`RegistrationStatus::NotConfigured`].
//! Historical Task Scheduler (`schtasks`) evidence is retained under [`logon_task`].
//! The next login-registration probe is [`hkcu_run`] (HKCU Run; no OS crash restart).

use crate::supervision::RegistrationStatus;

pub mod hkcu_run;
pub mod logon_task;

pub use hkcu_run::{
    HKCU_RUN_SUBKEY, HkcuRunSpec, HkcuRunStatus, TEST_HKCU_RUN_VALUE_NAME, build_run_command_line,
    quote_windows_arg, register as register_hkcu_run, status as status_hkcu_run,
    unregister as unregister_hkcu_run,
};
pub use logon_task::{
    LogonTaskSpec, LogonTaskStatus, TEST_LOGON_TASK_NAME, install, register, remove,
    request_clean_stop, run_now, status, unregister, wait_until_ready,
};

/// Production registration remains intentionally not configured.
pub fn registration_status() -> RegistrationStatus {
    RegistrationStatus::NotConfigured
}
