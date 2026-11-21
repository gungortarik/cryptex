//! Reconstruct a manual export in temporary storage during an explicit owner
//! ceremony. The historical profile and its device credential are unnecessary.
use super::{Error, RecoverySelection, codec, independent_folders};
use crate::backup_inventory::{MAX_INVENTORY_BYTES, reconstruct_v2};
use crate::import_staging::{open_recovery_source, source_snapshot};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

pub(super) fn read_selected(path: &Path, limit: u64) -> Result<Vec<u8>, Error> {
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
) -> Result<usize, Error> {
    let mut private = tempfile::Builder::new();
    private.prefix("cryptex-backup-verify-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        private.permissions(std::fs::Permissions::from_mode(0o700));
    }
    let scratch = private.tempdir().map_err(|_| Error::Archive)?;
    let profiles = reconstruct_files(codec, backup, checkpoint, kit, unlock, scratch.path())?;
    let verified = profiles.len();
    for profile in profiles {
        profile.close().map_err(|_| Error::Archive)?;
    }
    scratch.close().map_err(|_| Error::Archive)?;
    Ok(verified)
}

pub(crate) fn reconstruct_files(
    codec: &Path,
    backup: &Path,
    checkpoint: &Path,
    kit: &Path,
    unlock: &Path,
    scratch: &Path,
) -> Result<Vec<tempfile::TempDir>, Error> {
    independent_folders(
        backup.parent().ok_or(Error::Checkpoint)?,
        checkpoint.parent().ok_or(Error::Checkpoint)?,
    )?;
    let bytes = read_selected(checkpoint, 32768)?;
    let keys = KitFiles { codec, kit, unlock };
    if let Ok(selected) = serde_json::from_slice::<RecoverySelection>(&bytes) {
        let (file, _) = open_recovery_source(backup).map_err(|_| Error::Archive)?;
        return Ok(vec![open_file(&keys, file, &selected, None, scratch)?]);
    }
    let pair: super::paired::Pair =
        serde_json::from_slice(&bytes).map_err(|_| Error::Checkpoint)?;
    pair.validate()?;
    // The native picker can return either file from the pair. The checkpoint
    // pins each file's digest independently of the selected filename.
    let stem = backup.with_extension("");
    let selected_prior = backup
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pgp"))
        && stem
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("prior"));
    let (older, newer) = if selected_prior {
        (backup.to_path_buf(), stem.with_extension("pgp"))
    } else {
        (backup.with_extension("prior.pgp"), backup.to_path_buf())
    };
    let old =
        super::paired::copy_verified_cipher(&older, pair.parent.approval.request.backup_sha256)?;
    let new = super::paired::copy_verified_cipher(&newer, pair.replacement_sha256)?;
    let pin = Some(&pair.parent.signing_key);
    let old = open_file(&keys, old, &pair.archived, pin, scratch)?;
    let new = open_file(&keys, new, &pair.replacement, pin, scratch)?;
    let brain = &pair.parent.approval.request.brain_id;
    let new_key = pair.replacement.writer()?;
    let a = super::snapshot(old.path(), brain, &pair.archived.writer()?)?;
    let b = super::snapshot(new.path(), brain, &new_key)?;
    if !super::recovered_search::disjoint_histories(&a, &b, new_key.device_id()) {
        return Err(Error::RecoveryHistory);
    }
    Ok(vec![old, new])
}

struct KitFiles<'a> {
    codec: &'a Path,
    kit: &'a Path,
    unlock: &'a Path,
}

fn open_file(
    keys: &KitFiles<'_>,
    file: File,
    selected: &RecoverySelection,
    signing_key: Option<&[u8; 32]>,
    scratch: &Path,
) -> Result<tempfile::TempDir, Error> {
    let writer = selected.writer()?;
    let expected = selected.checkpoint(&writer)?;
    let before = source_snapshot(&file).map_err(|_| Error::Archive)?;
    let mut header = expected.brain_identity_sha256.to_vec();
    let credential = read_selected(keys.unlock, 128)?;
    let password = credential.trim_ascii();
    if password.len() != 64
        || !password
            .iter()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(Error::Credentials);
    }
    header.extend_from_slice(password);
    // Manual exports use a stored ZIP and uncompressed OpenPGP payload. Its
    // plaintext cannot exceed this particular ciphertext's measured length.
    header.extend_from_slice(&before.length.to_be_bytes());
    header.push(u8::from(signing_key.is_some()));
    if let Some(key) = signing_key {
        header.extend_from_slice(key);
    }
    header.extend(read_selected(keys.kit, 256 * 1024)?);
    let plain = codec::decrypt_manual(
        keys.codec,
        &header,
        file.try_clone().map_err(|_| Error::Archive)?,
    )?;
    if source_snapshot(&file).map_err(|_| Error::Archive)? != before {
        return Err(Error::Archive);
    }
    let mut input = plain.reopen().map_err(|_| Error::Archive)?;
    if signing_key.is_some() {
        let end = input.seek(SeekFrom::End(-1)).map_err(|_| Error::Archive)?;
        let mut mode = [0];
        input.read_exact(&mut mode).map_err(|_| Error::Archive)?;
        input.set_len(end).map_err(|_| Error::Archive)?;
        input.rewind().map_err(|_| Error::Archive)?;
        match mode[0] {
            1 => {
                return crate::backup_inventory::reconstruct_signed_legacy(
                    plain.path(),
                    &expected,
                    scratch,
                );
            }
            0 => (),
            _ => return Err(Error::Archive),
        }
    }
    let mut envelope = zip::ZipArchive::new(input).map_err(|_| Error::Archive)?;
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
    reconstruct_v2(payload.path(), &signed, &expected, scratch)
}
