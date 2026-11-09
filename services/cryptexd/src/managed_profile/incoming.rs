//! Gated daemon-side bridge from owned transport readiness to canonical import.
//! Hints are bounded relative paths, never transport trust or publication proof.
//! No normal startup, watcher, CLI or renderer constructs/feeds this controller.

use std::path::{Component, Path, PathBuf};

use cryptex_events::TrustedDaemonKeyResolver;

use super::worker::StatusReader;
use crate::incoming_publication::{IncomingPublicationError, IncomingPublicationReport};
use crate::instance::InstanceLock;
use crate::runtime_admission::AdmissionError;
use crate::secure_store::{SecureStore, SecureStoreEnvironment};

#[derive(Debug)]
pub(crate) enum IncomingControllerError {
    InvalidHint,
    QueueBusy,
    WrongRuntime,
    Admission(AdmissionError),
    BindingUnavailable,
    Publication(IncomingPublicationError),
}

impl std::fmt::Display for IncomingControllerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidHint => "incoming_hint_invalid",
            Self::QueueBusy => "incoming_queue_busy",
            Self::WrongRuntime => "incoming_runtime_mismatch",
            Self::Admission(error) => error.code(),
            Self::BindingUnavailable => "incoming_transport_binding_unavailable",
            Self::Publication(_) => "incoming_publication_failed",
        })
    }
}
impl std::error::Error for IncomingControllerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error),
            Self::Publication(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum IncomingTick {
    Idle,
    WaitingForTransport,
    Deferred,
    Published(IncomingPublicationReport),
}

/// One bounded backend batch at a time. Held/ambiguous publication retains hints
/// for verified repair. Clean rejection consumes hints, never transport bytes.
pub(crate) struct IncomingController {
    root: PathBuf,
    environment: SecureStoreEnvironment,
    pending: Option<Vec<PathBuf>>,
}

impl IncomingController {
    pub(crate) fn new(
        root: PathBuf,
        environment: SecureStoreEnvironment,
    ) -> Result<Self, IncomingControllerError> {
        if !root.is_absolute() || crate::paths::assert_outside_repository(&root).is_err() {
            return Err(IncomingControllerError::InvalidHint);
        }
        Ok(Self {
            root,
            environment,
            pending: None,
        })
    }

    pub(crate) fn enqueue(&mut self, paths: Vec<PathBuf>) -> Result<(), IncomingControllerError> {
        if paths.is_empty()
            || paths.len() > crate::import_limits::MAX_IMPORT_FILES_PER_BATCH
            || paths.iter().any(|path| {
                path.as_os_str().as_encoded_bytes().len() > 1024
                    || path.components().count() != 2
                    || !path
                        .components()
                        .all(|part| matches!(part, Component::Normal(_)))
                    || path.extension().is_none_or(|extension| extension != "seg")
            })
        {
            return Err(IncomingControllerError::InvalidHint);
        }
        if let Some(pending) = &self.pending {
            return if pending == &paths {
                Ok(())
            } else {
                Err(IncomingControllerError::QueueBusy)
            };
        }
        self.pending = Some(paths);
        Ok(())
    }

    pub(crate) fn poll<S: SecureStore + ?Sized>(
        &mut self,
        transport: &StatusReader,
        instance: &mut InstanceLock,
        store: &S,
    ) -> Result<IncomingTick, IncomingControllerError> {
        if instance.path() != crate::paths::lock_path(&self.root) {
            return Err(IncomingControllerError::WrongRuntime);
        }
        let Some(paths) = self.pending.as_ref() else {
            return Ok(IncomingTick::Idle);
        };
        let Ok(identity) = transport.try_transport_device_id() else {
            return Ok(IncomingTick::WaitingForTransport);
        };
        let admission = instance.admission();
        let permit = match admission.enter(&self.root) {
            Ok(permit) => permit,
            Err(AdmissionError::Busy) => return Ok(IncomingTick::Deferred),
            Err(error) => return Err(IncomingControllerError::Admission(error)),
        };
        if let Err(error) =
            require_transport_binding(&self.root, &identity, store, self.environment)
        {
            self.pending = None;
            return Err(error);
        }
        // Readiness is not authority. The immutable local approval roster binds
        // this worker identity; the publisher rechecks full policy under its own
        // exclusive admission and canonical locks. Never nest those admissions.
        if transport.try_transport_device_id().ok().as_deref() != Some(identity.as_str()) {
            return Ok(IncomingTick::WaitingForTransport);
        }
        drop(permit);
        match crate::incoming_publication::publish_incoming_with_instance(
            &self.root,
            paths,
            instance,
            store,
            self.environment,
        ) {
            Ok(report) => {
                self.pending = None;
                Ok(IncomingTick::Published(report))
            }
            Err(IncomingPublicationError::Admission(AdmissionError::Busy)) => {
                Ok(IncomingTick::Deferred)
            }
            Err(error @ IncomingPublicationError::Rejected(_)) => {
                // A bad untrusted batch must not be copied/verified every
                // transport heartbeat. Bytes remain in incoming for review;
                // an explicit new backend hint is required for another try.
                self.pending = None;
                Err(IncomingControllerError::Publication(error))
            }
            Err(error) => Err(IncomingControllerError::Publication(error)),
        }
    }
}

fn require_transport_binding<S: SecureStore + ?Sized>(
    root: &Path,
    expected: &str,
    store: &S,
    environment: SecureStoreEnvironment,
) -> Result<(), IncomingControllerError> {
    let verify = || {
        let brain = crate::identity_runtime::load_brain_identity(root).ok()?;
        let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            root,
            &brain.brain_id,
            store,
            environment,
        )
        .ok()?;
        let anchor = resolver.local_trust_anchor()?;
        let snapshot =
            crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
                root,
                &brain.brain_id,
                &anchor,
                None,
            )
            .ok()?;
        let (history, _) = snapshot.verified_trust_context()?;
        (!history.approved_peer_bindings.is_empty()
            && history
                .approved_peer_bindings
                .values()
                .all(|binding| binding.local_transport_device_id == expected))
        .then_some(())
    };
    verify().ok_or(IncomingControllerError::BindingUnavailable)
}

#[cfg(test)]
mod tests;
