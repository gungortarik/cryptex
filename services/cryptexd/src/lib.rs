//! M0 P0-2 lifecycle foundation for `cryptexd`.
//!
//! This crate provides testable abstractions for single-instance enforcement,
//! structured lifecycle status, crash/restart harness helpers,
//! version-compatibility fixtures, Mode A/B supervision comparison helpers,
//! and a test-only binary update/rollback harness.
//!
//! Supervision architecture (LaunchAgent vs tray sidecar vs hybrid) remains
//! intentionally unresolved until ADR-0012 evidence is complete.

pub mod compare;
pub mod compat;
pub mod error;
pub mod instance;
pub mod ipc;
pub mod paths;
pub mod platform;
pub mod status;
pub mod supervision;
pub mod supervisor;
pub mod update;

pub use compare::{
    ComparisonMeasurement, EvidenceClass, MEASUREMENT_SCHEMA, SupervisorMode, measurement_path,
    mode_runtime_root, unix_now_ms,
};
pub use compat::{CompatDecision, CompatError, ComponentVersions, evaluate_compat};
pub use error::{LifecycleError, LifecycleResult};
pub use instance::{InstanceLock, InstanceObservation, observe_instance, try_acquire_instance};
pub use paths::{
    assert_outside_repository, default_p0_2_runtime_root, ensure_runtime_root, lock_path,
    marker_path, status_path,
};
pub use status::{HealthState, InstanceState, LifecycleReport, write_status_file};
pub use supervision::{RegistrationStatus, SupervisionStrategy, selected_supervision};
pub use supervisor::DesktopSupervisor;
pub use update::{
    UpdateLayout, UpdateProofReport, activate_staged, install_known_good, marker_digest,
    rollback_to_previous, run_update_rollback_proof, stage_candidate, stage_compatible_candidate,
    stage_failing_candidate,
};
