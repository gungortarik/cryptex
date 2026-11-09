//! Windows internal incoming-to-canonical transaction integration.
//!
//! No CLI, IPC, watcher or managed-transport caller enables this path. Activation
//! still requires the ADR-0019 native durability and independent review gates.

use std::path::{Path, PathBuf};

use cryptex_events::TrustedDaemonKeyResolver;

use crate::import_staging::{ImportStagingError, ImportStagingSession};
use crate::runtime_admission::{AdmissionError, RuntimeHold};
use crate::secure_store::{SecureStore, SecureStoreEnvironment};

#[derive(Debug)]
pub(crate) enum IncomingPublicationError {
    Admission(AdmissionError),
    Rejected(ImportStagingError),
    /// Preparation may have reached disk, or publication may be partially
    /// visible. Preserve transaction evidence; never report a clean rollback.
    RecoveryRequired(ImportStagingError),
    /// The final canonical commit exists. Only derived-state refresh failed.
    CommittedNeedsRebuild(String),
}

impl std::fmt::Display for IncomingPublicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admission(error) => write!(f, "incoming admission refused: {error}"),
            Self::Rejected(error) => write!(f, "incoming import rejected: {error}"),
            Self::RecoveryRequired(error) => {
                write!(f, "incoming import requires recovery: {error}")
            }
            Self::CommittedNeedsRebuild(error) => write!(
                f,
                "incoming import committed; derived rebuild required: {error}"
            ),
        }
    }
}

impl std::error::Error for IncomingPublicationError {}

#[derive(Debug)]
pub(crate) struct IncomingPublicationReport {
    pub(crate) published_segments: usize,
    pub(crate) published_events: u64,
    pub(crate) identical_segments: usize,
}

/// Own the runtime, reconcile prior transactions before canonical inspection,
/// snapshot candidates, retain the exact local trust/M3/M4 lock-bearing token
/// through preparation and no-replace commit, then clean and refresh derived
/// state before releasing instance ownership. Incoming files are never moved
/// or removed. Rejection cleanup applies only before manifest preparation.
pub(crate) fn publish_incoming<S: SecureStore + ?Sized>(
    root: &Path,
    paths: &[PathBuf],
    store: &S,
    environment: SecureStoreEnvironment,
) -> Result<IncomingPublicationReport, IncomingPublicationError> {
    use IncomingPublicationError::Rejected;
    let _pins = ImportStagingSession::pin_existing_runtime(root).map_err(Rejected)?;
    let mut instance = crate::instance::try_acquire_instance(root)
        .map_err(|error| Rejected(ImportStagingError::Invalid(error.to_string())))?;
    publish_incoming_with_instance(root, paths, &mut instance, store, environment)
}

/// Long-lived daemon integration must reuse its existing instance ownership.
/// Reuses that instance's shared IPC/derived-worker admission barrier. It drains
/// ordinary work before staging and latches failure before reopening admission.
/// This entry point is not connected to a transport controller yet.
/// Exclusive borrowing also prevents two import/recovery operations from
/// concurrently using one instance guard through this safe-Rust API.
pub(crate) fn publish_incoming_with_instance<S: SecureStore + ?Sized>(
    root: &Path,
    paths: &[PathBuf],
    instance: &mut crate::instance::InstanceLock,
    store: &S,
    environment: SecureStoreEnvironment,
) -> Result<IncomingPublicationReport, IncomingPublicationError> {
    publish_incoming_until(
        root,
        paths,
        instance,
        store,
        environment,
        std::time::Instant::now() + std::time::Duration::from_secs(10),
    )
}

fn publish_incoming_until<S: SecureStore + ?Sized>(
    root: &Path,
    paths: &[PathBuf],
    instance: &mut crate::instance::InstanceLock,
    store: &S,
    environment: SecureStoreEnvironment,
    deadline: std::time::Instant,
) -> Result<IncomingPublicationReport, IncomingPublicationError> {
    if instance.path() != crate::paths::lock_path(root) {
        return Err(IncomingPublicationError::Rejected(
            ImportStagingError::Invalid("publication instance belongs to another runtime".into()),
        ));
    }
    let exclusive = instance
        .admission()
        .exclusive(root, deadline, false)
        .map_err(IncomingPublicationError::Admission)?;
    let result = publish_exclusively(root, paths, instance, store, environment);
    exclusive.finish(match &result {
        Ok(_) | Err(IncomingPublicationError::Rejected(_)) => None,
        Err(IncomingPublicationError::RecoveryRequired(_)) => Some(RuntimeHold::RecoveryRequired),
        Err(IncomingPublicationError::CommittedNeedsRebuild(_)) => {
            Some(RuntimeHold::DerivedRebuildRequired)
        }
        Err(IncomingPublicationError::Admission(_)) => Some(RuntimeHold::RecoveryRequired),
    });
    result
}

fn publish_exclusively<S: SecureStore + ?Sized>(
    root: &Path,
    paths: &[PathBuf],
    instance: &mut crate::instance::InstanceLock,
    store: &S,
    environment: SecureStoreEnvironment,
) -> Result<IncomingPublicationReport, IncomingPublicationError> {
    use IncomingPublicationError::{RecoveryRequired, Rejected};
    let _pins = ImportStagingSession::pin_existing_runtime(root).map_err(Rejected)?;
    if instance.path() != crate::paths::lock_path(root) {
        return Err(Rejected(ImportStagingError::Invalid(
            "publication instance belongs to another runtime".into(),
        )));
    }
    crate::import_staging::recover_import_staging_with_local_state(
        root,
        instance,
        store,
        environment,
    )
    .map_err(RecoveryRequired)?;
    let mut session = ImportStagingSession::begin(root).map_err(Rejected)?;
    let admission = (|| {
        let brain = crate::identity_runtime::load_brain_identity(root)
            .map_err(|error| ImportStagingError::Invalid(error.to_string()))?;
        let mut resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            root,
            &brain.brain_id,
            store,
            environment,
        )
        .map_err(|error| ImportStagingError::Invalid(error.to_string()))?;
        let local_device = resolver
            .local_trust_anchor()
            .ok_or_else(|| ImportStagingError::Invalid("local trust anchor unavailable".into()))?
            .device_id()
            .clone();
        let db = cryptex_projection::ProjectionDb::open_existing_read_only(root)
            .map_err(|error| ImportStagingError::Invalid(error.to_string()))?;
        let mut devices = session
            .stage_batch_from_transport_paths(paths)?
            .iter()
            .map(|candidate| candidate.device_id.clone())
            .collect::<Vec<_>>();
        devices.sort_by(|a, b| a.as_str().as_bytes().cmp(b.as_str().as_bytes()));
        devices.dedup();
        session.release_staging_slot()?;
        let safety = crate::knowledge_materialization::evaluate_import_retrieval_safety(
            root,
            &brain.brain_id,
            &devices,
            &mut resolver,
            &db,
        );
        let identical = safety.remove_identical_import_replays(&mut session)?;
        if session
            .candidates()
            .iter()
            .any(|candidate| candidate.device_id == local_device)
        {
            return Err(ImportStagingError::Invalid(
                "incoming cannot extend this installation's local stream; restore requires allocator reconciliation".into()));
        }
        let verified = if session.candidates().is_empty() {
            None
        } else {
            Some(safety.verify_import_batch(&session, &brain.brain_id)?)
        };
        Ok::<_, ImportStagingError>((brain.brain_id, identical, verified))
    })();
    let (brain, identical, verified) = match admission {
        Ok(result) => result,
        Err(error) => {
            if let Err(cleanup) = session.discard() {
                return Err(RecoveryRequired(ImportStagingError::RecoveryConflict(
                    format!("admission failed: {error}; staging cleanup failed: {cleanup}"),
                )));
            }
            return Err(Rejected(error));
        }
    };
    let Some(verified) = verified else {
        session.discard().map_err(RecoveryRequired)?;
        return Ok(IncomingPublicationReport {
            published_segments: 0,
            published_events: 0,
            identical_segments: identical,
        });
    };
    let count = verified.verified().segments().len();
    let events = verified
        .verified()
        .segments()
        .iter()
        .map(|s| s.event_count)
        .sum();
    // From this point forward even a preparation error preserves all staged
    // evidence: its final manifest may already have reached disk.
    session
        .prepare_verified_manifest(verified.verified())
        .map_err(RecoveryRequired)?;
    session
        .publish_verified_batch(&verified)
        .map_err(RecoveryRequired)?;
    drop(verified);
    drop(session);
    // Final-marker recovery checks exact destinations and performs the existing
    // receipt-bound cleanup. Instance ownership excludes supported readers.
    crate::import_staging::recover_import_staging_with_local_state(
        root,
        instance,
        store,
        environment,
    )
    .map_err(RecoveryRequired)?;
    rebuild_imported_state(root, &brain, store, environment)?;
    Ok(IncomingPublicationReport {
        published_segments: count,
        published_events: events,
        identical_segments: identical,
    })
}

fn rebuild_imported_state<S: SecureStore + ?Sized>(
    root: &Path,
    brain: &str,
    store: &S,
    environment: SecureStoreEnvironment,
) -> Result<(), IncomingPublicationError> {
    use IncomingPublicationError::CommittedNeedsRebuild;
    let resolver =
        crate::local_device_identity::LocalDeviceKeyResolver::load(root, brain, store, environment)
            .map_err(|error| CommittedNeedsRebuild(error.to_string()))?;
    let anchor = resolver
        .local_trust_anchor()
        .ok_or_else(|| CommittedNeedsRebuild("local trust anchor unavailable".into()))?;
    let mut db = cryptex_projection::ProjectionDb::open_or_create(root)
        .map_err(|error| CommittedNeedsRebuild(error.to_string()))?;
    crate::knowledge_materialization::rebuild_knowledge_from_local_approval_history(
        root, brain, &anchor, &mut db,
    )
    .map_err(|error| CommittedNeedsRebuild(error.to_string()))?;
    Ok(())
}

/// Internal recovery only: no renderer/CLI/autonomous recovery entry point.
/// A failed repair keeps admission closed; only complete verified recovery plus
/// cutoff-aware M3/M4 rebuild permits ordinary operations to resume.
pub(crate) fn recover_live_with_instance<S: SecureStore + ?Sized>(
    root: &Path,
    instance: &mut crate::instance::InstanceLock,
    store: &S,
    environment: SecureStoreEnvironment,
) -> Result<(), IncomingPublicationError> {
    let exclusive = instance
        .admission()
        .exclusive(
            root,
            std::time::Instant::now() + std::time::Duration::from_secs(10),
            true,
        )
        .map_err(IncomingPublicationError::Admission)?;
    crate::import_staging::recover_import_staging_with_local_state(
        root,
        instance,
        store,
        environment,
    )
    .map_err(IncomingPublicationError::RecoveryRequired)?;
    let brain = crate::identity_runtime::load_brain_identity(root)
        .map_err(|error| IncomingPublicationError::CommittedNeedsRebuild(error.to_string()))?;
    if let Err(error) = rebuild_imported_state(root, &brain.brain_id, store, environment) {
        exclusive.finish(Some(RuntimeHold::DerivedRebuildRequired));
        return Err(error);
    }
    exclusive.finish(None);
    Ok(())
}

#[cfg(test)]
#[path = "incoming_publication_tests.rs"]
mod tests;
