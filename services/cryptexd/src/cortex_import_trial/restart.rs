//! Restart fixture: one native credential per disposable Brain, outside product labels.
use super::{
    DEV, LifecycleError, TrialMode, emit, fault, initialize, provenance, session, temp_root,
    trial_temp,
};
use cryptex_core::{DeviceId, validate::parse_uuid_str};
#[cfg(target_os = "macos")]
use cryptexd::MacosKeychainStore as NativeStore;
#[cfg(windows)]
use cryptexd::WindowsCredentialStore as NativeStore;
use cryptexd::{SecureStore, SecureStoreError, SecureStoreResult};
use serde_json::json;
use std::{fs, io, path::Path};

const SERVICE: &str = "Cryptex-M15-Disposable-Restart";

struct TrialKey<'a> {
    native: NativeStore,
    id: &'a str,
    create: bool,
}

impl TrialKey<'_> {
    fn check_label(&self, service: &str, account: &str) -> SecureStoreResult<()> {
        if service == cryptexd::ServiceLabel::device_credentials(DEV).as_str()
            && account
                .strip_prefix("device-event-signing-seed:")
                .is_some_and(|id| parse_uuid_str(id).is_ok())
        {
            return Ok(());
        }
        Err(SecureStoreError::InvalidName("unexpected_trial_key".into()))
    }
}

impl SecureStore for TrialKey<'_> {
    fn put(&self, service: &str, account: &str, secret: &[u8]) -> SecureStoreResult<()> {
        self.check_label(service, account)?;
        if !self.create {
            return Err(SecureStoreError::InvalidName(
                "replay_key_creation_refused".into(),
            ));
        }
        self.native.put(SERVICE, self.id, secret)
    }
    fn get(&self, service: &str, account: &str) -> SecureStoreResult<Vec<u8>> {
        self.check_label(service, account)?;
        self.native.get(SERVICE, self.id)
    }
    fn delete(&self, service: &str, account: &str) -> SecureStoreResult<()> {
        self.check_label(service, account)?;
        if !self.create {
            return Err(SecureStoreError::InvalidName(
                "replay_key_deletion_refused".into(),
            ));
        }
        self.native.delete(SERVICE, self.id)
    }
}

pub(super) fn run(source: Option<&Path>, mode: &str, id: &str) -> Result<(), LifecycleError> {
    parse_uuid_str(id).map_err(|_| fault("invalid_trial_id"))?;
    let temp = match source {
        Some(source) => trial_temp(source)?,
        None => temp_root()?,
    };
    let root = temp.join(format!("cryptex-m15-restart-{id}"));
    if let Some(source) = source {
        let source_directory = fs::canonicalize(source)?;
        if source_directory.starts_with(&root) || root.starts_with(&source_directory) {
            return Err(fault("source_and_trial_profile_overlap"));
        }
    }
    #[cfg(target_os = "macos")]
    let _noninteractive =
        security_framework::os::macos::keychain::SecKeychain::disable_user_interaction()
            .map_err(|_| fault("trial_keychain_interaction_policy_failed"))?;
    let native = NativeStore::new();
    if mode == "--remove-trial-key" {
        match native.delete(SERVICE, id) {
            Ok(()) | Err(SecureStoreError::Missing) => (),
            Err(_) => return Err(fault("trial_key_cleanup_failed")),
        }
        return match native.get(SERVICE, id) {
            Err(SecureStoreError::Missing) => Ok(()),
            _ => Err(fault("trial_key_cleanup_unconfirmed")),
        };
    }
    let mode = match mode {
        "--restart-create" => TrialMode::Capture,
        "--exit-before-reply" => TrialMode::ExitBeforeReply,
        "--provenance" if source.is_none() => TrialMode::Replay,
        "--restart-replay" => TrialMode::Replay,
        "--batch-create" => TrialMode::Batch,
        "--batch-exit-before-source-receipt" => TrialMode::BatchExitBeforeSourceReceipt,
        _ => return Err(fault("invalid_restart_operation")),
    };
    let store = TrialKey {
        native,
        id,
        create: mode != TrialMode::Replay,
    };
    match mode {
        TrialMode::Capture
        | TrialMode::ExitBeforeReply
        | TrialMode::Batch
        | TrialMode::BatchExitBeforeSourceReceipt => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                fs::DirBuilder::new().mode(0o700).create(&root)?;
            }
            #[cfg(windows)]
            fs::create_dir(&root)?;
            if !matches!(
                store.native.get(SERVICE, id),
                Err(SecureStoreError::Missing)
            ) {
                return Err(fault("trial_key_not_fresh"));
            }
        }
        TrialMode::Replay => {
            if fs::canonicalize(&root)? != root || !fs::symlink_metadata(&root)?.is_dir() {
                return Err(fault("trial_profile_mismatch"));
            }
            let brain = cryptexd::identity_runtime::load_brain_identity(&root)
                .map_err(|_| fault("trial_brain_missing"))?;
            if brain.brain_id != id {
                return Err(fault("trial_profile_mismatch"));
            }
        }
    }
    let _instance = cryptexd::instance::try_acquire_instance(&root)?;
    let device = if store.create {
        initialize(&root, &store, id.to_owned())?
    } else {
        let local = cryptexd::local_device_identity::load_local_device_identity(&root, id)
            .map_err(|_| fault("trial_device_missing"))?;
        DeviceId::new(local.device_id).map_err(|_| fault("trial_device_failed"))?
    };
    let (key, public) =
        cryptexd::event_signing_key::load_existing_event_verifying_key(&store, DEV, &root, &device)
            .map_err(|_| fault("trial_key_unavailable"))?;
    let Some(source) = source else {
        let report = provenance::report(&root, &device, &store, &public)?;
        return emit(&mut io::stdout().lock(), &report).map_err(Into::into);
    };
    emit(
        &mut io::stdout().lock(),
        &json!({"pid":std::process::id(), "brain_id":id,
          "device_id":device.as_str(), "signing_key_id":key.signing_key_id}),
    )?;
    session(source, &root, &device, &store, mode)
}
