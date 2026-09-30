//! M0 P0-2 lifecycle foundation for `cryptexd`.
//!
//! This crate provides testable abstractions for single-instance enforcement,
//! structured lifecycle status, crash/restart harness helpers, and
//! version-compatibility fixtures.
//!
//! Supervision architecture (LaunchAgent vs tray sidecar vs hybrid) remains
//! intentionally unresolved until ADR-0012 evidence is complete.

pub mod compat;
pub mod error;
pub mod instance;
pub mod paths;
pub mod platform;
pub mod status;
pub mod supervision;

pub use compat::{CompatDecision, CompatError, ComponentVersions, evaluate_compat};
pub use error::{LifecycleError, LifecycleResult};
pub use instance::{InstanceLock, InstanceObservation, observe_instance, try_acquire_instance};
pub use paths::{
    assert_outside_repository, default_p0_2_runtime_root, ensure_runtime_root, lock_path,
    marker_path, status_path,
};
pub use status::{HealthState, InstanceState, LifecycleReport, write_status_file};
pub use supervision::{RegistrationStatus, SupervisionStrategy, selected_supervision};
