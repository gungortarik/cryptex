//! Opt-in for the native disposable-profile harness. Production has no entry point.
use super::ReviewError;
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

/// Rebuild from the approved, authenticated survivors before reclaiming SQLite pages.
/// May be repeated after a crash between pack replacement and derived cleanup.
pub(crate) fn finish_cleanup(
    root: &Path,
    brain: &str,
    anchor: &cryptex_events::LocalTrustAnchor,
) -> Result<(), ReviewError> {
    if !enabled(root) {
        return Err(ReviewError::Disabled);
    }
    let mut db = cryptex_projection::ProjectionDb::open_or_create(root)
        .map_err(|_| ReviewError::CleanupPending)?;
    db.connection()
        .execute_batch("PRAGMA secure_delete=ON")
        .map_err(|_| ReviewError::CleanupPending)?;
    crate::knowledge_materialization::rebuild_knowledge_from_local_approval_history(
        root, brain, anchor, &mut db,
    )
    .map_err(|_| ReviewError::CleanupPending)?;
    // The projection rebuild also reconstructs the external-content FTS table.
    let conn = db.connection();
    conn.execute_batch("VACUUM")
        .map_err(|_| ReviewError::CleanupPending)?;
    let busy: i64 = conn
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get(0))
        .map_err(|_| ReviewError::CleanupPending)?;
    if busy != 0 {
        return Err(ReviewError::CleanupPending);
    }
    Ok(())
}
