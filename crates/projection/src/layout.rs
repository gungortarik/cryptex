//! App-data path layout for disposable projection state (M3 Phase 0 plan).
//!
//! Paths are relative to an app-data / runtime root supplied by the caller.
//! Callers must keep that root outside the source repository and outside
//! visible home/Desktop/Documents by default.
//!
//! Proposed layout (implementation choice, not an ADR):
//! `<app-data-root>/derived/projection/projection.sqlite`

use std::path::{Path, PathBuf};

/// Leaf filename for the primary projection database.
pub const PROJECTION_DB_FILE_NAME: &str = "projection.sqlite";

/// `<app-data-root>/derived/projection`
///
/// Holds rebuildable SQLite/FTS artifacts. Must never be treated as canonical
/// recovery state or as a Syncthing semantic authority.
pub fn derived_projection_dir(app_data_root: &Path) -> PathBuf {
    app_data_root.join("derived").join("projection")
}

/// `<app-data-root>/derived/projection/projection.sqlite`
pub fn projection_sqlite_path(app_data_root: &Path) -> PathBuf {
    derived_projection_dir(app_data_root).join(PROJECTION_DB_FILE_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Component, Path};

    #[test]
    fn builds_derived_projection_paths_under_app_data() {
        let root = Path::new("C:/Cryptex-Dev/runtime-fixture");
        let dir = derived_projection_dir(root);
        let db = projection_sqlite_path(root);

        assert!(dir.ends_with("derived/projection") || dir.ends_with(r"derived\projection"));
        assert_eq!(
            db.file_name().and_then(|n| n.to_str()),
            Some(PROJECTION_DB_FILE_NAME)
        );
        assert_eq!(db.parent(), Some(dir.as_path()));
        assert!(db.starts_with(root));
    }

    #[test]
    fn projection_paths_stay_out_of_canonical_and_identity_trees() {
        let root = Path::new("/var/tmp/cryptex-app-data");
        let dir = derived_projection_dir(root);
        let db = projection_sqlite_path(root);

        let components: Vec<_> = dir
            .strip_prefix(root)
            .expect("under root")
            .components()
            .filter_map(|c| match c {
                Component::Normal(s) => s.to_str(),
                _ => None,
            })
            .collect();
        assert_eq!(components, ["derived", "projection"]);

        assert!(!db.components().any(|c| c.as_os_str() == "canonical"));
        assert!(!db.components().any(|c| c.as_os_str() == "identity"));
        assert!(!db.components().any(|c| c.as_os_str() == "quarantine"));
        assert!(!dir.ends_with("canonical/events"));
    }

    #[test]
    fn unix_style_root_uses_same_relative_layout() {
        let root = Path::new("/Users/test/Library/Application Support/Cryptex-Dev");
        let db = projection_sqlite_path(root);
        assert!(db.ends_with("derived/projection/projection.sqlite"));
    }
}
