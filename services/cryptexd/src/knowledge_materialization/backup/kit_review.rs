use super::*;
use crate::ipc::backup_verify::Selection;

/// Verify the two sealed Kit selections before the owner can publish a successor.
/// Each archive is opened through the same path used to admit a first device.
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
        let new = restore::first_archive(codec, files, scratch.path())?;
        let writer = old.selection.writer()?;
        let a = old.selection.checkpoint(&writer)?;
        let b = new.selection.checkpoint(&writer)?;
        if old.pin != new.pin
            || a.brain_id != b.brain_id
            || a.brain_identity_sha256 != b.brain_identity_sha256
            || b.generation <= a.generation
        {
            return Err(Error::Checkpoint);
        }
        {
            let (before, _) = retention_state(old.profile.path(), &writer)?;
            let (after, _) = retention_state(new.profile.path(), &writer)?;
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
                if prior.content_hash() != next.content_hash()
                    || matches!(prior, AuthenticatedSecurityRecord::PurgeBridge(..))
                        && !matches!((prior, *next), (AuthenticatedSecurityRecord::PurgeBridge(a, ..), AuthenticatedSecurityRecord::PurgeBridge(b, ..)) if a.content_hash == b.content_hash)
                {
                    return Err(Error::RecoveryHistory);
                }
            }
        }
        hashes.push(hex(new.hash));
        new.profile.close().map_err(|_| Error::Archive)?;
    }
    old.profile.close().map_err(|_| Error::Archive)?;
    scratch.close().map_err(|_| Error::Archive)?;
    Ok(hashes)
}
