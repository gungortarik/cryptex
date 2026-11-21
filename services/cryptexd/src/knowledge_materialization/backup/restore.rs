//! Reconstruct a manual export in temporary storage during an explicit owner
//! ceremony. The historical profile and its device credential are unnecessary.
use super::{Error, RecoverySelection, codec, independent_folders};
use crate::backup_inventory::{MAX_INVENTORY_BYTES, reconstruct_v2};
use crate::import_staging::{open_recovery_source, source_snapshot};
use std::io::{self, Read};
use std::path::Path;

fn read_selected(path: &Path, limit: u64) -> Result<Vec<u8>, Error> {
    let (mut input, before) = open_recovery_source(path).map_err(|_| Error::Archive)?;
    if before.length == 0 || before.length > limit {
        return Err(Error::Archive);
    }
    let mut bytes = Vec::new();
    (&mut input)
        .take(before.length + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Archive)?;
    if bytes.len() as u64 != before.length
        || source_snapshot(&input).map_err(|_| Error::Archive)? != before
    {
        return Err(Error::Archive);
    }
    Ok(bytes)
}

pub(crate) fn verify_manual(
    codec: &Path,
    backup: &Path,
    checkpoint: &Path,
    kit: &Path,
    unlock: &Path,
) -> Result<(), Error> {
    independent_folders(
        backup.parent().ok_or(Error::Checkpoint)?,
        checkpoint.parent().ok_or(Error::Checkpoint)?,
    )?;
    let selected: RecoverySelection =
        serde_json::from_slice(&read_selected(checkpoint, 8192)?).map_err(|_| Error::Checkpoint)?;
    let writer = selected.writer()?;
    let expected = selected.checkpoint(&writer)?;
    let mut header = expected.brain_identity_sha256.to_vec();
    let credential = read_selected(unlock, 128)?;
    let password = credential.trim_ascii();
    if password.len() != 64
        || !password
            .iter()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(Error::Credentials);
    }
    header.extend_from_slice(password);
    header.extend(read_selected(kit, 256 * 1024)?);
    let (file, before) = open_recovery_source(backup).map_err(|_| Error::Archive)?;
    let plain = codec::decrypt_manual(
        codec,
        &header,
        file.try_clone().map_err(|_| Error::Archive)?,
    )?;
    if source_snapshot(&file).map_err(|_| Error::Archive)? != before {
        return Err(Error::Archive);
    }
    let mut envelope = zip::ZipArchive::new(plain.reopen().map_err(|_| Error::Archive)?)
        .map_err(|_| Error::Archive)?;
    if envelope.len() != 2
        || envelope
            .has_overlapping_files()
            .map_err(|_| Error::Archive)?
    {
        return Err(Error::Archive);
    }
    let mut signed = Vec::new();
    let mut payload = tempfile::NamedTempFile::new().map_err(|_| Error::Archive)?;
    for i in 0..2 {
        let member = envelope.by_index(i).map_err(|_| Error::Archive)?;
        let size = member.size();
        if member.compression() != zip::CompressionMethod::Stored
            || size == 0
            || size != member.compressed_size()
        {
            return Err(Error::Archive);
        }
        let count = match member.name().map_err(|_| Error::Archive)?.as_ref() {
            "inventory.json" if size <= MAX_INVENTORY_BYTES as u64 && signed.is_empty() => {
                io::copy(&mut member.take(size + 1), &mut signed)
            }
            "content.zip"
                if payload
                    .as_file()
                    .metadata()
                    .map_err(|_| Error::Archive)?
                    .len()
                    == 0 =>
            {
                io::copy(&mut member.take(size + 1), &mut payload)
            }
            _ => return Err(Error::Archive),
        }
        .map_err(|_| Error::Archive)?;
        if count != size {
            return Err(Error::Archive);
        }
    }
    let mut private = tempfile::Builder::new();
    private.prefix("cryptex-backup-verify-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        private.permissions(std::fs::Permissions::from_mode(0o700));
    }
    let scratch = private.tempdir().map_err(|_| Error::Archive)?;
    let restored = reconstruct_v2(payload.path(), &signed, &expected, scratch.path())?;
    restored.close().map_err(|_| Error::Archive)?;
    scratch.close().map_err(|_| Error::Archive)
}
