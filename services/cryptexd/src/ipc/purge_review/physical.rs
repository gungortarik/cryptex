//! Opt-in for the native disposable-profile harness. Production has no entry point.
use super::{ReviewError, SearchContext};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static PROFILE: OnceLock<PathBuf> = OnceLock::new();

pub(crate) fn register(root: &Path) {
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("cryptex-ipc-search-physical-")
    );
    PROFILE.set(root.to_owned()).unwrap();
}

pub(crate) fn enabled(root: &Path) -> bool {
    PROFILE.get().is_some_and(|p| p == root)
}

pub(in crate::ipc) fn finish_cleanup(ctx: &SearchContext) -> Result<(), ReviewError> {
    if !enabled(&ctx.runtime_root) {
        return Err(ReviewError::Disabled);
    }
    crate::ipc::synchronize_local_derived_state(ctx).map_err(|_| ReviewError::CleanupPending)?;
    let db = cryptex_projection::ProjectionDb::open_or_create(&ctx.runtime_root)
        .map_err(|_| ReviewError::CleanupPending)?;
    let conn = db.connection();
    conn.execute_batch(
        "PRAGMA secure_delete=ON;
        INSERT INTO events_fts(events_fts) VALUES ('rebuild');
        VACUUM;",
    )
    .map_err(|_| ReviewError::CleanupPending)?;
    let busy: i64 = conn
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get(0))
        .map_err(|_| ReviewError::CleanupPending)?;
    if busy != 0 {
        return Err(ReviewError::CleanupPending);
    }
    Ok(())
}
