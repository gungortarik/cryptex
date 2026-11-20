//! Construct a private backup payload from a locked, authenticated device
//! history. The ciphertext writer receives the resulting ZIP and signature.
use super::*;
use crate::backup_inventory::{Checkpoint, Error, RecoverySelection};
use crate::import_staging::{open_recovery_source, source_snapshot};
use crate::local_device_identity::LocalDeviceKeyResolver;
use std::collections::BTreeSet;
use std::io::{self, Write};
use tempfile::NamedTempFile;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

pub struct Prepared {
    payload: NamedTempFile,
    inventory: Vec<u8>,
    pub selection: RecoverySelection,
}

impl Prepared {
    /// Input for the public-key encryptor. The device signing credential is
    /// absent from both members.
    pub fn encryption_input(&self) -> Result<NamedTempFile, Error> {
        let mut output = NamedTempFile::new().map_err(|_| Error::Archive)?;
        let mut zip = ZipWriter::new(output.as_file_mut());
        append(
            &mut zip,
            "content.zip",
            self.payload.reopen().map_err(|_| Error::Archive)?,
        )?;
        append(&mut zip, "inventory.json", self.inventory.as_slice())?;
        zip.finish()
            .map_err(|_| Error::Archive)?
            .flush()
            .map_err(|_| Error::Archive)?;
        Ok(output)
    }
}

fn append(
    zip: &mut ZipWriter<&mut fs::File>,
    name: &str,
    mut reader: impl Read,
) -> Result<u64, Error> {
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .large_file(true);
    zip.start_file(name, options).map_err(|_| Error::Archive)?;
    io::copy(&mut reader, zip).map_err(|_| Error::Archive)
}

pub fn prepare(
    root: &Path,
    store: &(impl SecureStore + ?Sized),
    env: SecureStoreEnvironment,
    generation: u64,
) -> Result<Prepared, Error> {
    let (identity_file, _) =
        open_recovery_source(&crate::identity_runtime::brain_identity_path(root))
            .map_err(|_| Error::Archive)?;
    let mut bytes = Vec::new();
    identity_file
        .take(4097)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Archive)?;
    if bytes.len() > 4096 {
        return Err(Error::Archive);
    }
    let identity = cryptex_core::BrainIdentity::from_json(
        std::str::from_utf8(&bytes).map_err(|_| Error::Archive)?,
    )
    .map_err(|_| Error::Archive)?;
    let anchor = LocalDeviceKeyResolver::load(root, &identity.brain_id, store, env)
        .map_err(|_| Error::Credentials)?
        .local_trust_anchor()
        .ok_or(Error::Credentials)?;
    let snapshot =
        scan_verified_canonical_from_local_anchor_until(root, &identity.brain_id, &anchor, None)
            .map_err(|_| Error::Checkpoint)?;
    let (history, state) = snapshot
        .verified_trust_context()
        .ok_or(Error::Credentials)?;
    if history.local_device != *anchor.device_id()
        || !state.trusted_devices.contains(anchor.device_id())
        || history.restricted(state, anchor.device_id())
    {
        return Err(Error::Credentials);
    }
    let mut payload = NamedTempFile::new().map_err(|_| Error::Archive)?;
    let mut archive = ZipWriter::new(payload.as_file_mut());
    append(
        &mut archive,
        "identity/brain_identity.json",
        bytes.as_slice(),
    )?;
    for device in &snapshot.devices {
        let directory = canonical_events_device_dir(root, device);
        if !directory.try_exists().map_err(|_| Error::Archive)? {
            continue;
        }
        let mut packs = BTreeSet::new();
        for entry in fs::read_dir(&directory).map_err(|_| Error::Archive)? {
            let name = entry
                .map_err(|_| Error::Archive)?
                .file_name()
                .into_string()
                .map_err(|_| Error::Archive)?;
            if is_known_housekeeping_file(&name) {
                continue;
            }
            if parse_pack_index(&name, "seg").is_none() && parse_pack_index(&name, "open").is_none()
            {
                return Err(Error::Archive);
            }
            packs.insert(name);
        }
        for name in packs {
            let (mut file, before) =
                open_recovery_source(&directory.join(&name)).map_err(|_| Error::Archive)?;
            let member = format!("canonical/events/{}/{name}", device.as_filename_stem());
            let copied = append(&mut archive, &member, &mut file)?;
            if copied != before.length
                || source_snapshot(&file).map_err(|_| Error::Archive)? != before
            {
                return Err(Error::Archive);
            }
        }
    }
    for digest in snapshot.admitted_blob_digests().collect::<BTreeSet<_>>() {
        let hash = sha2::digest::Output::<Sha256>::from(digest);
        let file =
            crate::attachment_blobs::open_verified(root, &digest).map_err(|_| Error::Attachment)?;
        append(&mut archive, &format!("canonical/blobs/{hash:x}"), file)?;
    }
    archive
        .finish()
        .map_err(|_| Error::Archive)?
        .flush()
        .map_err(|_| Error::Archive)?;
    let checkpoint = Checkpoint {
        brain_id: &identity.brain_id,
        writer: &anchor,
        generation,
        history_sha256: recovered_search::history_digest(&snapshot),
        brain_identity_sha256: Sha256::digest(bytes).into(),
    };
    let inventory = crate::backup_inventory::sign(root, store, env, payload.path(), &checkpoint)?;
    Ok(Prepared {
        payload,
        inventory,
        selection: RecoverySelection::capture(&checkpoint),
    })
}
