//! Explicit first note for a fresh local enrollment. No normal-startup seeding,
//! trust override, import, recovery repair or existing-history append route.
use crate::capture::{CaptureOutcome, CaptureRequest};
use crate::import_staging::export_fs::Directory;
use crate::secure_store::{SecureStore, SecureStoreEnvironment};
use cryptex_events::envelope::Classification;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Arguments,
    Runtime,
    Busy,
    Identity,
    ExistingState,
    CaptureRejected,
    CaptureUncertain,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "first_capture_{self:?}")
    }
}
impl std::error::Error for Error {}

fn names(root: &Path) -> Result<Vec<String>, Error> {
    let entries = std::fs::read_dir(root).map_err(|_| Error::Runtime)?;
    let mut result = Vec::new();
    for entry in entries {
        if result.len() >= 32 {
            return Err(Error::ExistingState);
        }
        let name = entry
            .map_err(|_| Error::Runtime)?
            .file_name()
            .into_string()
            .map_err(|_| Error::ExistingState)?;
        result.push(name);
    }
    Ok(result)
}

pub fn capture_first_local_dev<S: SecureStore>(
    root: &Path,
    store: &S,
    text: String,
) -> Result<(), Error> {
    if text.is_empty() || text.len() > 4096 || text.contains('\0') {
        return Err(Error::Arguments);
    }
    crate::paths::assert_outside_repository(root).map_err(|_| Error::Arguments)?;
    let namespace = Directory::open(root).map_err(|_| Error::Runtime)?;
    let _instance = crate::instance::try_acquire_instance(root).map_err(|_| Error::Busy)?;
    // Exhaustive fresh layout, not a failed scan/recovery fallback. In
    // particular any bootstrap, transport, capture receipt or derived state
    // refuses. On uncertain capture the retained evidence prevents a retry.
    for name in names(root)? {
        if ![
            "identity",
            "canonical",
            "sequence",
            "daemon.lock",
            "pilot-runtime.marker",
            "bootstrap",
        ]
        .contains(&name.as_str())
        {
            return Err(Error::ExistingState);
        }
    }
    let identity = namespace
        .child("identity", false)
        .map_err(|_| Error::Runtime)?;
    if namespace.exists("bootstrap").map_err(|_| Error::Runtime)? {
        let bootstrap = namespace
            .child("bootstrap", false)
            .map_err(|_| Error::Runtime)?;
        if names(&root.join("bootstrap"))? != ["pending.lock"] {
            return Err(Error::ExistingState);
        }
        let lock = bootstrap
            .open_file("pending.lock", false)
            .map_err(|_| Error::ExistingState)?;
        if lock.metadata().map_err(|_| Error::Runtime)?.len() != 0 {
            return Err(Error::ExistingState);
        }
    }
    let canonical = namespace
        .child("canonical", false)
        .map_err(|_| Error::Runtime)?;
    if names(&root.join("canonical"))? != ["events"] {
        return Err(Error::ExistingState);
    }
    let events = canonical
        .child("events", false)
        .map_err(|_| Error::Runtime)?;
    let sequence = namespace
        .child("sequence", false)
        .map_err(|_| Error::Runtime)?;
    if !names(&root.join("sequence"))?.is_empty() {
        return Err(Error::ExistingState);
    }
    let brain = crate::identity_runtime::load_brain_identity(root).map_err(|_| Error::Identity)?;
    let local = crate::local_device_identity::load_local_device_identity(root, &brain.brain_id)
        .map_err(|_| Error::Identity)?;
    crate::local_device_identity::LocalDeviceKeyResolver::load(
        root,
        &brain.brain_id,
        store,
        SecureStoreEnvironment::Dev,
    )
    .map_err(|_| Error::Identity)?;
    let device = cryptex_core::DeviceId::new(local.device_id).map_err(|_| Error::Identity)?;
    let device_name = device
        .as_str()
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let devices = identity
        .child("devices", false)
        .map_err(|_| Error::Identity)?;
    if names(&root.join("identity/devices"))? != [device_name.clone()] {
        return Err(Error::ExistingState);
    }
    let device_pin = devices
        .child(&device_name, false)
        .map_err(|_| Error::Identity)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    {
        let _inventory =
            crate::canonical_writer::acquire_canonical_inventory_lock_until(root, deadline)
                .map_err(|_| Error::Busy)?;
        for name in names(&root.join("canonical/events"))? {
            if name != "inventory.lock" {
                return Err(Error::ExistingState);
            }
        }
    }
    let inventory = events
        .open_file("inventory.lock", false)
        .map_err(|_| Error::ExistingState)?;
    if inventory.metadata().map_err(|_| Error::Runtime)?.len() != 0 {
        return Err(Error::ExistingState);
    }
    drop(inventory);
    // Sole daemon instance remains held. Capture acquires inventory/device
    // locks itself, exactly once, and owns receipt + append + durability.
    for pin in [
        &namespace,
        &identity,
        &canonical,
        &events,
        &sequence,
        &devices,
        &device_pin,
    ] {
        pin.validate().map_err(|_| Error::Runtime)?;
    }
    let mut random = [0u8; 16];
    getrandom::getrandom(&mut random).map_err(|_| Error::Runtime)?;
    random[6] = (random[6] & 15) | 64;
    random[8] = (random[8] & 63) | 128;
    let hex = random
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let capture_id = format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    );
    let outcome = crate::capture::capture_memory_note(
        store,
        SecureStoreEnvironment::Dev,
        root,
        CaptureRequest {
            capture_id,
            text,
            classification: Classification::Public,
        },
        &crate::canonical_writer::CanonicalWriterConfig::default(),
    );
    for pin in [
        &namespace,
        &identity,
        &canonical,
        &events,
        &sequence,
        &devices,
        &device_pin,
    ] {
        pin.validate().map_err(|_| Error::CaptureUncertain)?;
    }
    match outcome {
        CaptureOutcome::Committed(_) => Ok(()),
        CaptureOutcome::Rejected { .. } => Err(Error::CaptureRejected),
        CaptureOutcome::Uncertain { .. } => Err(Error::CaptureUncertain),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fresh() -> (std::path::PathBuf, crate::FakeSecureStore) {
        let root = crate::test_temp::exclusive_temp_dir("first-local-capture");
        let store = crate::FakeSecureStore::new();
        crate::install_runtime::prepare_m1_runtime_foundation(&root).unwrap();
        crate::identity_runtime::write_brain_identity(
            &root,
            &cryptex_core::BrainIdentity::new(
                "49b66fe0-df17-46c3-93d5-e88a8330d02c",
                "2026-11-16T00:00:00Z",
            )
            .unwrap(),
        )
        .unwrap();
        crate::local_device_identity::enroll_local_device(
            &root,
            &store,
            SecureStoreEnvironment::Dev,
        )
        .unwrap();
        std::fs::create_dir_all(root.join("canonical/events")).unwrap();
        (root, store)
    }
    #[test]
    fn first_capture_commits_once_and_reopening_refuses_without_appending() {
        let (root, store) = fresh();
        assert_eq!(
            capture_first_local_dev(&root, &store, "Owner first public note".into()),
            Ok(())
        );
        assert_eq!(
            capture_first_local_dev(&root, &store, "different note".into()),
            Err(Error::ExistingState)
        );
        let brain = crate::identity_runtime::load_brain_identity(&root).unwrap();
        let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            &root,
            &brain.brain_id,
            &store,
            SecureStoreEnvironment::Dev,
        )
        .unwrap();
        use cryptex_events::TrustedDaemonKeyResolver;
        let snapshot =
            crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
                &root,
                &brain.brain_id,
                &resolver.local_trust_anchor().unwrap(),
                None,
            )
            .unwrap();
        assert!(snapshot.verified_trust_context().is_some());
        drop(snapshot);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn first_capture_refuses_foreign_history_recovery_and_transport() {
        for entry in [
            "canonical/import-journal",
            "canonical/events/foreign",
            "bootstrap",
            "transport",
            "managed-transport",
            "connection-settings",
            "capture-receipts",
        ] {
            let (root, store) = fresh();
            std::fs::create_dir(root.join(entry)).unwrap();
            assert_eq!(
                capture_first_local_dev(&root, &store, "first".into()),
                Err(Error::ExistingState)
            );
            assert!(root.join(entry).is_dir());
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn first_capture_missing_key_and_invalid_text_do_not_create_note() {
        let (root, store) = fresh();
        assert_eq!(
            capture_first_local_dev(&root, &store, "".into()),
            Err(Error::Arguments)
        );
        assert_eq!(
            capture_first_local_dev(&root, &store, "x".repeat(4097)),
            Err(Error::Arguments)
        );
        assert_eq!(
            capture_first_local_dev(&root, &crate::FakeSecureStore::new(), "first".into()),
            Err(Error::Identity)
        );
        assert_eq!(
            std::fs::read_dir(root.join("canonical/events"))
                .unwrap()
                .count(),
            0
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn first_capture_interrupted_receipt_blocks_new_note_without_erasing_evidence() {
        let (root, store) = fresh();
        crate::capture::fail_after_pending_before_append();
        assert_eq!(
            capture_first_local_dev(&root, &store, "first".into()),
            Err(Error::CaptureUncertain)
        );
        assert_eq!(
            capture_first_local_dev(&root, &store, "retry".into()),
            Err(Error::ExistingState)
        );
        assert!(root.join("canonical/events").is_dir());
        std::fs::remove_dir_all(root).unwrap();
    }
}
