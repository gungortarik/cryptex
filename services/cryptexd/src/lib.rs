//! M0 P0-2 lifecycle foundation for `cryptexd`.
//!
//! This crate provides testable abstractions for single-instance enforcement,
//! structured lifecycle status, crash/restart harness helpers,
//! version-compatibility fixtures, Mode A/B supervision comparison helpers,
//! and a test-only binary update/rollback harness.
//!
//! Secure-store (ADR-0014 ACCEPTED â€” P1) is owned by this daemon crate.

pub mod canonical_writer;
pub mod capture;
pub mod capture_receipt;
pub mod compare;
pub mod compat;
pub mod error;
pub mod event_signing_key;
pub mod identity_runtime;
pub mod install_runtime;
pub mod instance;
pub mod ipc;
pub mod knowledge_materialization;
pub mod local_device_identity;
mod metadata_persistence;
pub mod pairing_store;
pub mod paths;
pub mod platform;
pub mod projection;
pub mod recovery_store;
pub mod secure_store;
pub mod sequence_runtime;
pub mod sequence_store;
pub mod status;
pub mod supervision;

pub mod supervisor;
#[cfg(test)]
mod test_temp;
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
#[cfg(target_os = "macos")]
pub use secure_store::MacosKeychainStore;
#[cfg(windows)]
#[cfg(windows)]
pub use secure_store::WindowsCredentialStore;
pub use secure_store::{
    AccountLabel, FakeSecureStore, SecureStore, SecureStoreEnvironment, SecureStoreError,
    SecureStoreResult, ServiceLabel,
};
pub use status::{HealthState, InstanceState, LifecycleReport, write_status_file};
pub use supervision::{RegistrationStatus, SupervisionStrategy, selected_supervision};
pub use supervisor::DesktopSupervisor;
pub use update::{
    UpdateLayout, UpdateProofReport, activate_staged, install_known_good, marker_digest,
    rollback_to_previous, run_update_rollback_proof, stage_candidate, stage_compatible_candidate,
    stage_failing_candidate,
};
