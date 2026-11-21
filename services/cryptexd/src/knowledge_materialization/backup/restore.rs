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
    let scratch = private_scratch(None)?;
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
    reconstruct_selected(codec, backup, checkpoint, kit, unlock, scratch, None)
        .map(|(profiles, _)| profiles)
}

/// The scheduled verifier uses a private frozen copy of the numbered file.
/// Locate the companion on the publication's target and bind it to the
/// daemon's publication receipt before either decrypted history is admitted.
pub(crate) fn reconstruct_selected(
    codec: &Path,
    backup: &Path,
    checkpoint: &Path,
    kit: &Path,
    unlock: &Path,
    scratch: &Path,
    scheduled: Option<(&Path, Option<[u8; 32]>)>,
) -> Result<(Vec<tempfile::TempDir>, Option<u64>), Error> {
    independent_folders(
        backup.parent().ok_or(Error::Checkpoint)?,
        checkpoint.parent().ok_or(Error::Checkpoint)?,
    )?;
    let bytes = read_selected(checkpoint, 32768)?;
    let keys = KitFiles::load(codec, kit, unlock)?;
    let protected = |selected: &RecoverySelection| -> Result<Option<u64>, Error> {
        if scheduled.is_none() {
            return Ok(None);
        }
        let writer = selected.writer()?;
        let expected = selected.checkpoint(&writer)?;
        let header = keys.header(&expected.brain_identity_sha256, 1, 3, None);
        let Some(pin) = codec::selected_checkpoint(codec, &header)? else {
            return Ok(None);
        };
        let pinned = pin.writer()?;
        let checkpoint = pin.checkpoint(&pinned)?;
        if expected.brain_identity_sha256 != checkpoint.brain_identity_sha256
            || expected.brain_id != checkpoint.brain_id
        {
            return Err(Error::Checkpoint);
        }
        Ok((pinned == writer).then_some(checkpoint.generation))
    };
    if let Ok(selected) = serde_json::from_slice::<RecoverySelection>(&bytes) {
        if scheduled.is_some_and(|(_, digest)| digest.is_some()) {
            return Err(Error::RecoveryHistory);
        }
        let (file, _) = open_recovery_source(backup).map_err(|_| Error::Archive)?;
        return Ok((
            vec![open_file(&keys, file, &selected, None, scratch)?],
            protected(&selected)?,
        ));
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
    let (older, newer) = if pair.scheduled() {
        let digest = pair.parent.approval.request.backup_sha256;
        let target = match scheduled {
            Some((dir, Some(expected))) if digest == expected => dir,
            Some(_) => return Err(Error::RecoveryHistory),
            None => backup.parent().ok_or(Error::Checkpoint)?,
        };
        (
            target.join(super::paired::archive_name(digest)),
            backup.to_owned(),
        )
    } else if scheduled.is_some() {
        return Err(Error::RecoveryHistory);
    } else if selected_prior {
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
    Ok((vec![old, new], protected(&pair.replacement)?))
}

struct KitFiles<'a> {
    codec: &'a Path,
    kit: Vec<u8>,
    unlock: Vec<u8>,
}

impl<'a> KitFiles<'a> {
    fn load(codec: &'a Path, kit: &Path, unlock: &Path) -> Result<Self, Error> {
        let credential = read_selected(unlock, 128)?;
        let password = credential.trim_ascii();
        if password.len() != 64
            || !password
                .iter()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err(Error::Credentials);
        }
        Ok(Self {
            codec,
            kit: read_selected(kit, 256 * 1024)?,
            unlock: credential,
        })
    }

    fn header(
        &self,
        identity: &[u8; 32],
        limit: u64,
        mode: u8,
        proof: Option<&[u8; 32]>,
    ) -> Vec<u8> {
        let mut header = identity.to_vec();
        header.extend_from_slice(self.unlock.trim_ascii());
        header.extend_from_slice(&limit.to_be_bytes());
        header.push(mode);
        if let Some(bytes) = proof {
            header.extend_from_slice(bytes);
        }
        header.extend_from_slice(&self.kit);
        header
    }
}

/// Inspect the Kit-selected first-device generation without an enrolled profile.
/// Reconstruct and verify the same frozen ciphertext whose hash is returned.
pub(crate) fn inspect_first_archive(
    codec: &Path,
    files: &crate::ipc::backup_verify::Selection,
) -> Result<(String, String, String), Error> {
    use sha2::{Digest, Sha256};
    let scratch = private_scratch(None)?;
    let archive = first_archive(codec, files, scratch.path())?;
    let writer = archive.selection.writer()?;
    let brain = archive.selection.checkpoint(&writer)?.brain_id.to_owned();
    let result = (
        brain,
        format!("{:x}", Sha256::digest(archive.pin)),
        format!("{:x}", sha2::digest::Output::<Sha256>::from(archive.hash)),
    );
    archive.profile.close().map_err(|_| Error::Archive)?;
    scratch.close().map_err(|_| Error::Archive)?;
    Ok(result)
}

pub(crate) fn private_scratch(parent: Option<&Path>) -> Result<tempfile::TempDir, Error> {
    let mut private = tempfile::Builder::new();
    private.prefix("cryptex-recovery-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        private.permissions(std::fs::Permissions::from_mode(0o700));
    }
    match parent {
        Some(dir) => private.tempdir_in(dir),
        None => private.tempdir(),
    }
    .map_err(|_| Error::Archive)
}

pub(super) struct FirstArchive {
    pub profile: tempfile::TempDir,
    pub selection: RecoverySelection,
    pub hash: [u8; 32],
    pub pin: [u8; 32],
}

#[cfg(all(test, unix))]
#[test]
fn reconstructed_plaintext_staging_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let parent = tempfile::tempdir().unwrap();
    for location in [None, Some(parent.path())] {
        let scratch = private_scratch(location).unwrap();
        let permissions = scratch.path().metadata().unwrap().permissions();
        assert_eq!(permissions.mode() & 0o777, 0o700);
    }
}

pub(super) fn first_archive(
    codec: &Path,
    files: &crate::ipc::backup_verify::Selection,
    scratch: &Path,
) -> Result<FirstArchive, Error> {
    use sha2::{Digest, Sha256};
    independent_folders(
        files.backup.parent().ok_or(Error::Checkpoint)?,
        files.checkpoint.parent().ok_or(Error::Checkpoint)?,
    )?;
    let selected: RecoverySelection =
        serde_json::from_slice(&read_selected(&files.checkpoint, 32768)?)
            .map_err(|_| Error::Checkpoint)?;
    let writer = selected.writer()?;
    let expected = selected.checkpoint(&writer)?;
    if expected.generation > (1u64 << 53) - 1 {
        return Err(Error::Checkpoint);
    }
    let selection_hash =
        Sha256::digest(serde_json_canonicalizer::to_vec(&selected).map_err(|_| Error::Checkpoint)?)
            .into();
    let keys = KitFiles::load(codec, &files.kit, &files.unlock)?;
    let header = keys.header(&expected.brain_identity_sha256, 1, 2, Some(&selection_hash));
    let pin = codec::kit_public_pin(codec, &header)?;
    let (frozen, hash) = super::paired::freeze_cipher(&files.backup)?;
    let profile = open_file(&keys, frozen, &selected, Some(&pin), scratch)?;
    Ok(FirstArchive {
        profile,
        selection: selected,
        hash,
        pin,
    })
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
    // Manual exports use a stored ZIP and uncompressed OpenPGP payload. Its
    // plaintext cannot exceed this particular ciphertext's measured length.
    let header = keys.header(
        &expected.brain_identity_sha256,
        before.length,
        u8::from(signing_key.is_some()),
        signing_key,
    );
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
