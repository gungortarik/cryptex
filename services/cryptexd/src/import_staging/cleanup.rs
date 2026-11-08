//! Receipt-bound, restartable cleanup of an already committed import.
//! Callers retain runtime instance ownership; this module never changes events.
use super::*;

const MAX_RECEIPT_BYTES: u64 = 72 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Receipt {
    protocol_version: u32,
    transaction_id: String,
    committed_marker: CommittedImportMarker,
    prepared_manifest: PreparedImportManifest,
}

fn hold(message: &str) -> ImportStagingError {
    ImportStagingError::RecoveryConflict(message.into())
}

pub(super) fn is_receipt_name(name: &str) -> bool {
    name.strip_suffix(".cleanup.json")
        .or_else(|| name.strip_prefix('.')?.strip_suffix(".cleanup.tmp"))
        .is_some_and(canonical_transaction_id)
}

fn receipt_paths(root: &Path, id: &str) -> (PathBuf, PathBuf) {
    let journal = root.join("canonical/import-journal");
    (
        journal.join(format!("{id}.cleanup.json")),
        journal.join(format!(".{id}.cleanup.tmp")),
    )
}

fn read_receipt(path: &Path) -> Result<Receipt, ImportStagingError> {
    let parent = path.parent().ok_or_else(|| hold("receipt has no parent"))?;
    let handles = inspect_directory_chain(parent, true)?;
    let mut file = open_source_from_directory_chain(&handles, path)?;
    let before = source_snapshot(&file)?;
    if !before.is_regular_file || before.link_count != 1 || before.length > MAX_RECEIPT_BYTES {
        return Err(hold(
            "cleanup receipt is not an ordinary bounded single-link file",
        ));
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_RECEIPT_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 != before.length || source_snapshot(&file)? != before {
        return Err(hold("cleanup receipt changed while being read"));
    }
    let receipt: Receipt = serde_json::from_slice(&bytes)
        .map_err(|_| hold("cleanup receipt has invalid closed schema"))?;
    validate(&receipt)?;
    if serde_json_canonicalizer::to_vec(&receipt).map_err(|_| hold("receipt encoding failed"))?
        != bytes
    {
        return Err(hold("cleanup receipt is not canonical JSON"));
    }
    let name = path.file_name().and_then(|name| name.to_str());
    if name != Some(format!("{}.cleanup.json", receipt.transaction_id).as_str())
        && name != Some(format!(".{}.cleanup.tmp", receipt.transaction_id).as_str())
    {
        return Err(hold(
            "cleanup receipt filename does not match its transaction",
        ));
    }
    Ok(receipt)
}

fn validate(receipt: &Receipt) -> Result<(), ImportStagingError> {
    if receipt.protocol_version != 1
        || receipt.transaction_id != receipt.committed_marker.transaction_id
        || receipt.transaction_id != receipt.prepared_manifest.transaction_id
    {
        return Err(hold(
            "cleanup receipt version or transaction binding is invalid",
        ));
    }
    validate_committed_marker(&receipt.committed_marker).map_err(hold)?;
    validate_prepared_manifest(&receipt.prepared_manifest)?;
    let bytes = serde_json_canonicalizer::to_vec(&receipt.prepared_manifest)
        .map_err(|_| hold("cleanup manifest encoding failed"))?;
    if bytes.len() as u64 > MAX_PREPARED_MANIFEST_BYTES
        || encode_hex(&Sha256::digest(bytes)) != receipt.committed_marker.prepared_manifest_sha256
    {
        return Err(hold("cleanup receipt manifest digest or size is invalid"));
    }
    Ok(())
}

pub(super) fn lock_slot(root: &Path) -> Result<File, ImportStagingError> {
    let path = root.join("canonical/import-journal/import.lock");
    if !entry_exists(&path)? {
        return Err(hold("cleanup requires the existing import lock record"));
    }
    let lock = open_lock_file(&path)?;
    lock.try_lock_exclusive()
        .map_err(|_| hold("import slot is owned by another operation"))?;
    Ok(lock)
}

fn check_namespace(root: &Path, allowed: &[&Path]) -> Result<(), ImportStagingError> {
    let journal = root.join("canonical/import-journal");
    let lock = journal.join("import.lock");
    for entry in fs::read_dir(&journal)? {
        let path = entry?.path();
        if path != lock && !allowed.contains(&path.as_path()) {
            return Err(hold("unexpected or conflicting cleanup journal entry"));
        }
    }
    Ok(())
}

pub(super) fn check_initial_journal(
    root: &Path,
    marker: &Path,
    manifest: &Path,
) -> Result<(), ImportStagingError> {
    check_namespace(root, &[marker, manifest])
}

// No deletion until every remaining companion and every destination is checked.
fn verify_state(
    root: &Path,
    path: &Path,
    receipt: &Receipt,
    require_companions: bool,
) -> Result<(), ImportStagingError> {
    let journal = root.join("canonical/import-journal");
    let manifest_path = journal.join(format!("{}.prepared.json", receipt.transaction_id));
    let marker_path = journal.join(format!("{}.committed.json", receipt.transaction_id));
    check_namespace(root, &[path, &manifest_path, &marker_path])?;
    if read_receipt(path)? != *receipt {
        return Err(hold("cleanup receipt changed under locks"));
    }
    for (present, matches) in [
        (
            entry_exists(&manifest_path)?,
            if entry_exists(&manifest_path)? {
                read_prepared_manifest(&manifest_path)? == receipt.prepared_manifest
            } else {
                !require_companions
            },
        ),
        (
            entry_exists(&marker_path)?,
            if entry_exists(&marker_path)? {
                read_committed_marker(&marker_path)? == receipt.committed_marker
            } else {
                !require_companions
            },
        ),
    ] {
        if !matches || (require_companions && !present) {
            return Err(hold("cleanup companion is missing or mismatched"));
        }
    }
    let staging = root.join("canonical/import-staging");
    let transaction = staging.join(&receipt.transaction_id);
    if entry_exists(&staging)? {
        let _handles = inspect_directory_chain(&staging, true)?;
        for entry in fs::read_dir(&staging)? {
            if entry?.path() != transaction {
                return Err(hold("cleanup staging contains another transaction"));
            }
        }
    }
    if entry_exists(&transaction)? {
        let _handles = inspect_directory_chain(&transaction, true)?;
        ensure_manifest_staging_subset(&transaction, &receipt.prepared_manifest)?;
        for segment in &receipt.prepared_manifest.segments {
            manifest_file_matches(
                &transaction.join(&segment.staged_relative_path),
                segment,
                true,
            )?;
        }
    } else if require_companions {
        return Err(hold(
            "temporary cleanup receipt lacks its staging companion",
        ));
    }
    for segment in &receipt.prepared_manifest.segments {
        let destination = root
            .join("canonical/events")
            .join(&segment.destination_relative_path);
        if !manifest_file_matches(&destination, segment, true)? {
            return Err(hold("cleanup receipt destination is missing"));
        }
    }
    Ok(())
}

pub(super) fn resume(root: &Path, path: &Path) -> Result<(), ImportStagingError> {
    let receipt = read_receipt(path)?;
    let mut _pins = pin_roots(root)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let _inventory =
        crate::canonical_writer::acquire_canonical_inventory_lock_until(root, deadline)
            .map_err(|_| hold("cannot acquire cleanup canonical inventory lock"))?;
    let mut devices = receipt
        .prepared_manifest
        .segments
        .iter()
        .map(|segment| {
            DeviceId::new(segment.device_id.clone()).map_err(|_| hold("invalid cleanup device"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    devices.sort_by(|a, b| a.as_str().as_bytes().cmp(b.as_str().as_bytes()));
    devices.dedup();
    let mut _devices = Vec::new();
    for device in devices {
        let directory = canonical_events_device_dir(root, &device);
        if entry_exists(&directory)? {
            _pins.extend(inspect_directory_chain(&directory, true)?);
        }
        if let Some(lock) = crate::canonical_writer::acquire_existing_device_canonical_lock_until(
            root, &device, deadline,
        )
        .map_err(|_| hold("cannot acquire cleanup device lock"))?
        {
            _devices.push(lock);
        }
    }
    let _slot = lock_slot(root)?;
    let (final_path, _) = receipt_paths(root, &receipt.transaction_id);
    verify_state(root, path, &receipt, path != final_path)?;
    if path != final_path {
        publish_staged_no_replace(path, &final_path)?;
        sync_staged_parent(&final_path)?;
    }
    cleanup_locked(root, &final_path, &receipt)
}

fn pin_roots(root: &Path) -> Result<Vec<File>, ImportStagingError> {
    ensure_existing_plain_directory(&root.join("canonical/events"))?;
    let mut pins = inspect_directory_chain(root, true)?;
    for relative in [
        "canonical/import-journal",
        "canonical/import-staging",
        "canonical/events",
    ] {
        let path = root.join(relative);
        if entry_exists(&path)? {
            pins.extend(inspect_directory_chain(&path, true)?);
        }
    }
    Ok(pins)
}

pub(super) fn complete_locked(
    root: &Path,
    marker_path: &Path,
    marker: &CommittedImportMarker,
    manifest: &PreparedImportManifest,
) -> Result<(), ImportStagingError> {
    let _pins = pin_roots(root)?;
    let receipt = Receipt {
        protocol_version: 1,
        transaction_id: marker.transaction_id.clone(),
        committed_marker: marker.clone(),
        prepared_manifest: manifest.clone(),
    };
    validate(&receipt)?;
    let (final_path, temporary) = receipt_paths(root, &receipt.transaction_id);
    let manifest_path = root
        .join("canonical/import-journal")
        .join(format!("{}.prepared.json", receipt.transaction_id));
    check_initial_journal(root, marker_path, &manifest_path)?;
    let bytes =
        serde_json_canonicalizer::to_vec(&receipt).map_err(|_| hold("receipt encoding failed"))?;
    if bytes.len() as u64 > MAX_RECEIPT_BYTES {
        return Err(hold("receipt exceeds size bound"));
    }
    #[cfg(test)]
    interrupt(root, "m8_before_cleanup_receipt_creation");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    #[cfg(test)]
    interrupt(root, "m8_cleanup_receipt_created");
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    #[cfg(test)]
    interrupt(root, "m8_cleanup_receipt_temp_synced");
    verify_state(root, &temporary, &receipt, true)?;
    publish_staged_no_replace(&temporary, &final_path)?;
    #[cfg(test)]
    interrupt(root, "m8_cleanup_receipt_moved");
    sync_staged_parent(&final_path)?;
    #[cfg(test)]
    interrupt(root, "m8_cleanup_receipt_final_synced");
    cleanup_locked(root, &final_path, &receipt)
}

fn cleanup_locked(root: &Path, path: &Path, receipt: &Receipt) -> Result<(), ImportStagingError> {
    verify_state(root, path, receipt, false)?;
    let transaction = root
        .join("canonical/import-staging")
        .join(&receipt.transaction_id);
    if entry_exists(&transaction)? {
        // Delete only manifest-bound leaves; empty directories are valid after a crash.
        for segment in &receipt.prepared_manifest.segments {
            let staged = transaction.join(&segment.staged_relative_path);
            if entry_exists(&staged)? {
                manifest_file_matches(&staged, segment, false)?;
                fs::remove_file(&staged)?;
                #[cfg(test)]
                interrupt(root, "m8_cleanup_staged_file_removed_before_sync");
                sync_staged_parent(&staged)?;
                #[cfg(test)]
                interrupt(root, "m8_during_committed_staging_cleanup");
            }
        }
        let directories = receipt
            .prepared_manifest
            .segments
            .iter()
            .map(|segment| {
                transaction
                    .join(&segment.staged_relative_path)
                    .parent()
                    .unwrap()
                    .to_path_buf()
            })
            .collect::<std::collections::BTreeSet<_>>();
        for directory in directories {
            if entry_exists(&directory)? {
                ensure_existing_plain_directory(&directory)?;
                fs::remove_dir(&directory)?;
                #[cfg(test)]
                interrupt(root, "m8_cleanup_staged_directory_removed_before_sync");
                sync_staged_parent(&directory)?;
            }
        }
        fs::remove_dir(&transaction)?;
        #[cfg(test)]
        interrupt(root, "m8_cleanup_transaction_removed_before_sync");
    }
    // Persist absence even when the previous process exited between deletion
    // and parent sync. Never retire the receipt before that ordering is durable.
    if entry_exists(transaction.parent().unwrap())? {
        sync_staged_parent(&transaction)?;
    }
    #[cfg(test)]
    interrupt(root, "m8_after_committed_staging_cleanup");
    for (suffix, _boundary) in [
        ("prepared.json", "m8_after_committed_manifest_cleanup"),
        ("committed.json", "m8_after_committed_marker_cleanup"),
    ] {
        verify_state(root, path, receipt, false)?;
        let companion = path.with_file_name(format!("{}.{suffix}", receipt.transaction_id));
        if entry_exists(&companion)? {
            fs::remove_file(&companion)?;
            #[cfg(test)]
            interrupt(root, &format!("{_boundary}_before_sync"));
        }
        sync_staged_parent(&companion)?;
        #[cfg(test)]
        interrupt(root, _boundary);
    }
    verify_state(root, path, receipt, false)?;
    fs::remove_file(path)?;
    #[cfg(test)]
    interrupt(root, "m8_cleanup_receipt_removed_before_sync");
    sync_staged_parent(path)?;
    #[cfg(test)]
    interrupt(root, "m8_after_cleanup_receipt_removed");
    Ok(())
}

#[cfg(test)]
fn interrupt(root: &Path, boundary: &str) {
    crate::test_interrupt::maybe_abrupt_exit(
        boundary,
        root,
        "receipt-bound committed cleanup boundary",
    );
}
