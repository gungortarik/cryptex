use super::*;
use crate::backup_scheduler::retention::{VerifiedBackups, generation_number};
use crate::knowledge_materialization::backup::{
    independent_folders, private_scratch, reconstruct_files,
};
use cryptex_events::LocalTrustAnchor;

pub(super) fn prepare(
    ctx: &SearchContext,
    anchor: &LocalTrustAnchor,
    codec: &Path,
    older: &Path,
    older_checkpoint: &Path,
    files: &super::super::backup_verify::Selection,
) -> Result<Outcome, Error> {
    let target = files.backup.parent().ok_or(Error::UnsafeTarget)?;
    if older.parent() != Some(target) {
        return Err(Error::UnsafeTarget);
    }
    let number = |path: &Path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .and_then(generation_number)
            .ok_or(Error::InvalidRequest)
    };
    let old = number(older)?;
    let new = number(&files.backup)?;
    for file in [
        older_checkpoint,
        &files.checkpoint,
        &files.kit,
        &files.unlock,
    ] {
        if !file.is_absolute() {
            return Err(Error::UnsafeTarget);
        }
        crate::paths::assert_outside_repository(file).map_err(|_| Error::UnsafeTarget)?;
        independent_folders(&ctx.runtime_root, file).map_err(|_| Error::UnsafeTarget)?;
    }
    for checkpoint in [older_checkpoint, &files.checkpoint] {
        independent_folders(target, checkpoint.parent().ok_or(Error::UnsafeTarget)?)
            .map_err(|_| Error::UnsafeTarget)?;
    }
    let catalog = VerifiedBackups {
        target,
        designated: anchor,
        store: ctx.store.as_ref(),
        environment: ctx.environment,
        writer_profile: &ctx.runtime_root,
    };
    let scratch = private_scratch(None).map_err(|_| Error::Unavailable)?;
    let review = catalog
        .review_removal(anchor, old, new, |generation, cipher| {
            let checkpoint = if generation == old {
                older_checkpoint
            } else {
                &files.checkpoint
            };
            let mut profiles = reconstruct_files(
                codec,
                cipher,
                checkpoint,
                &files.kit,
                &files.unlock,
                scratch.path(),
            )
            .map_err(|_| io::Error::other("backup removal reconstruction held"))?;
            if profiles.len() != 1 {
                return Err(io::Error::other("unexpected backup history count"));
            }
            Ok(profiles.remove(0))
        })
        .map_err(|_| Error::Verification)?;
    scratch.close().map_err(|_| Error::Unavailable)?;
    let pending = super::super::backup_removal::Pending::from_review(ctx, anchor.clone(), review)
        .map_err(|_| Error::Unavailable)?;
    *ctx.backup_removal.lock().map_err(|_| Error::Unavailable)? = Some(pending);
    Ok(Outcome::RemovalPrepared)
}
