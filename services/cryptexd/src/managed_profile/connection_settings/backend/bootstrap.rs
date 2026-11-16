//! Explicit identity-only Windows enrollment, never a failed-scan fallback.
use super::*;
use std::fs;
use std::os::windows::fs::MetadataExt;

fn metadata(path: &Path) -> Result<Option<fs::Metadata>, Error> {
    match fs::symlink_metadata(path) {
        Ok(meta) if !meta.file_type().is_symlink() && meta.file_attributes() & 0x400 == 0 => {
            Ok(Some(meta))
        }
        Ok(_) => Err(Error::Authority),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(Error::Authority),
    }
}
fn regular(path: &Path) -> Result<bool, Error> {
    match metadata(path)? {
        Some(meta) if meta.is_file() => Ok(true),
        None => Ok(false),
        _ => Err(Error::Authority),
    }
}

/// Startup already recovered import evidence and approval append receipts. This
/// check admits only empty history with valid optional enrollment metadata.
pub(super) fn identity_only(
    root: &Path,
    store: &dyn SecureStore,
    env: SecureStoreEnvironment,
) -> Result<bool, Error> {
    let _pins = crate::import_staging::ImportStagingSession::pin_existing_runtime(root)
        .map_err(|_| Error::Authority)?;
    if metadata(&root.join("connection-settings"))?.is_some() {
        return Ok(false);
    }
    let journal = crate::bootstrap_journal::BootstrapJournalStore::new(root)
        .and_then(|journal| journal.load())
        .map_err(|_| Error::Authority)?;
    if journal.transactions.iter().any(|t| {
        matches!(
            t.state,
            crate::bootstrap_journal::BootstrapState::AppendUnknown
                | crate::bootstrap_journal::BootstrapState::Anchored
        )
    }) {
        return Ok(false);
    }
    let brain_path = crate::identity_runtime::brain_identity_path(root);
    let local_path = crate::local_device_identity::local_device_identity_path(root);
    let brain = if regular(&brain_path)? {
        Some(crate::identity_runtime::load_brain_identity(root).map_err(|_| Error::Authority)?)
    } else {
        None
    };
    let local = if regular(&local_path)? {
        let brain = brain.as_ref().ok_or(Error::Authority)?;
        let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            root,
            &brain.brain_id,
            store,
            env,
        )
        .map_err(|_| Error::Authority)?;
        Some(
            resolver
                .local_trust_anchor()
                .ok_or(Error::Authority)?
                .device_id()
                .clone(),
        )
    } else {
        let devices = root.join("identity/devices");
        if let Some(meta) = metadata(&devices)?
            && (!meta.is_dir()
                || fs::read_dir(devices)
                    .map_err(|_| Error::Authority)?
                    .next()
                    .is_some())
        {
            return Err(Error::Authority);
        }
        None
    };
    if local.is_none() {
        // Do not create inventory.lock before explicit enrollment: the existing
        // join contract correctly refuses any unbound canonical device state.
        for path in [root.join("canonical"), root.join("canonical/events")] {
            if let Some(meta) = metadata(&path)?
                && !meta.is_dir()
            {
                return Err(Error::Authority);
            }
        }
        match fs::read_dir(root.join("canonical/events")) {
            Ok(mut entries) => {
                return if entries.next().is_none() {
                    Ok(true)
                } else {
                    Err(Error::Authority)
                };
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(true),
            _ => return Err(Error::Authority),
        }
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    let _inventory =
        crate::canonical_writer::acquire_canonical_inventory_lock_until(root, deadline)
            .map_err(|_| Error::Unavailable)?;
    let events = root.join("canonical/events");
    for entry in fs::read_dir(&events).map_err(|_| Error::Authority)? {
        let entry = entry.map_err(|_| Error::Authority)?;
        let meta = metadata(&entry.path())?.ok_or(Error::Authority)?;
        if entry.file_name() == "inventory.lock" && meta.is_file() && meta.len() == 0 {
            continue;
        }
        let Some(local) = &local else {
            return Err(Error::Authority);
        };
        if !meta.is_dir()
            || entry.path() != cryptex_storage::canonical_events_device_dir(root, local)
        {
            return Ok(false);
        }
        let _device = crate::canonical_writer::acquire_existing_device_canonical_lock_until(
            root, local, deadline,
        )
        .map_err(|_| Error::Unavailable)?
        .ok_or(Error::Authority)?;
        for leaf in fs::read_dir(entry.path()).map_err(|_| Error::Authority)? {
            let leaf = leaf.map_err(|_| Error::Authority)?;
            let meta = metadata(&leaf.path())?.ok_or(Error::Authority)?;
            if leaf.file_name() != "writer.lock" || !meta.is_file() || meta.len() != 0 {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
