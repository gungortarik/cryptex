use super::*;
use crate::ipc::backup_verify::Selection;

/// Authenticate the current first-device selection and reconstruct a proposed
/// daily backup with the original Kit before requesting an owner decision.
pub fn review_kit_backups(
    codec: &Path,
    current: &Selection,
    candidate: Option<&Selection>,
) -> Result<Vec<String>, Error> {
    let scratch = private_scratch(None)?;
    let old = restore::first_archive(codec, current, scratch.path())?;
    let hex = |hash: [u8; 32]| format!("{:x}", sha2::digest::Output::<Sha256>::from(hash));
    let mut hashes = vec![hex(old.hash)];
    if let Some(files) = candidate {
        let writer = old.selection.writer()?;
        let selected: RecoverySelection =
            serde_json::from_slice(&restore::read_selected(&files.checkpoint, 32768)?)
                .map_err(|_| Error::Checkpoint)?;
        let a = old.selection.checkpoint(&writer)?;
        let b = selected.checkpoint(&writer)?;
        if a.brain_id != b.brain_id
            || a.brain_identity_sha256 != b.brain_identity_sha256
            || b.generation <= a.generation
            || b.generation > (1u64 << 53) - 1
        {
            return Err(Error::Checkpoint);
        }
        let (new, hash) =
            restore::open_proposed_daily(codec, current, files, &selected, scratch.path())?;
        {
            let (before, _) = retention_state(old.profile.path(), &writer)?;
            let (after, _) = retention_state(new.path(), &writer)?;
            let records: BTreeMap<_, _> = after
                .security_history
                .records()
                .iter()
                .map(|r| (r.event_id(), r))
                .collect();
            for prior in before.security_history.records() {
                let next = records
                    .get(prior.event_id())
                    .ok_or(Error::RecoveryHistory)?;
                let preserves_purge = match (prior, *next) {
                    (
                        AuthenticatedSecurityRecord::PurgeBridge(a, ..),
                        AuthenticatedSecurityRecord::PurgeBridge(b, ..),
                    ) => a.content_hash == b.content_hash,
                    (AuthenticatedSecurityRecord::PurgeBridge(..), _) => false,
                    _ => true,
                };
                if prior.content_hash() != next.content_hash() || !preserves_purge {
                    return Err(Error::RecoveryHistory);
                }
            }
        }
        hashes.push(hex(hash));
        new.close().map_err(|_| Error::Archive)?;
    }
    old.profile.close().map_err(|_| Error::Archive)?;
    scratch.close().map_err(|_| Error::Archive)?;
    Ok(hashes)
}
