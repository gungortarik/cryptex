//! Windows namespace adapter for admitted empty streams during live recovery.
//! Only the daemon recovery orchestrator calls this under exclusive admission.
use super::*;
use crate::import_staging::export_fs::Directory;

pub(super) fn initialize<S: SecureStore + ?Sized>(
    root: &Path,
    store: &S,
    environment: SecureStoreEnvironment,
) -> Result<(), IncomingPublicationError> {
    let prepare = || -> Result<(), Box<dyn std::error::Error>> {
        // Retain non-reparse, current-owner namespace handles across the scan
        // and lock transition. No canonical body or signing key is created here.
        let open = |path: &Path| Directory::open(path).map_err(|_| "stream namespace unavailable");
        let mut pins = vec![open(root)?];
        let events = open(&root.join("canonical/events"))?;
        let brain = crate::identity_runtime::load_brain_identity(root)?;
        let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            root,
            &brain.brain_id,
            store,
            environment,
        )?;
        let anchor = resolver
            .local_trust_anchor()
            .ok_or("local anchor missing")?;
        let snapshot =
            crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
                root,
                &brain.brain_id,
                &anchor,
                None,
            )?;
        let (history, _) = snapshot
            .verified_trust_context()
            .ok_or("stream policy held")?;
        let mut devices = history
            .approved_peer_bindings
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        devices.push(anchor.device_id().clone());
        for device in snapshot.locked_devices() {
            pins.push(open(&cryptex_storage::canonical_events_device_dir(
                root, device,
            ))?);
        }
        if !snapshot.locks_runtime(root) {
            return Err("stream snapshot namespace changed".into());
        }
        drop(snapshot);
        // Creation follows the canonical inventory lock order. Use the existing
        // directory adapter, then retain each validated child through rebuild
        // preparation; directory names derive only from verified DeviceIds.
        {
            let _inventory = crate::canonical_writer::acquire_canonical_inventory_lock(root)?;
            for device in &devices {
                pins.push(
                    events
                        .child(&device.as_filename_stem(), true)
                        .map_err(|_| "admitted stream directory unavailable")?,
                );
            }
        }
        let _slots = crate::canonical_writer::acquire_canonical_write_lock_set(root, &devices)?;
        for pin in pins.iter().chain(std::iter::once(&events)) {
            pin.validate().map_err(|_| "stream namespace changed")?;
        }
        Ok(())
    };
    prepare().map_err(|_| {
        IncomingPublicationError::CommittedNeedsRebuild(
            "Windows admitted stream initialization failed".into(),
        )
    })
}
