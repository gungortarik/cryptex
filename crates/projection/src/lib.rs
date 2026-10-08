//! Disposable local projection foundations (M3).
//!
//! Authority: accepted M3 contract under `docs/milestones/M3_LOCAL_PROJECTION_SEARCH.md`
//! and `evidence/m3/m3-phase0-implementation-plan-20261008.md`.
//!
//! SQLite/FTS state under these paths is derived and rebuildable. Canonical event
//! packs remain authoritative. This crate must not write `.open`/`.seg` files.
//!
//! Slice 4/5 insert verified `memory.note` rows and rebuild/catch-up checkpoints via
//! cryptexd. Search APIs remain deferred.

pub mod checkpoint;
pub mod db;
pub mod error;
pub mod fts;
pub mod ingest;
pub mod layout;
pub mod schema;

pub use checkpoint::{
    DeviceCheckpoint, clear_all_projection_rows, clear_device_projection, read_device_checkpoint,
    upsert_device_checkpoint,
};
pub use db::{
    ProjectionDb, ProjectionOpenKind, ProjectionReplaceReason, projection_sidecar_paths,
    remove_projection_files,
};
pub use error::{ProjectionError, ProjectionResult};
pub use fts::{ensure_fts5_available, probe_bundled_fts5};
pub use ingest::{project_memory_note, project_memory_notes};
pub use layout::{PROJECTION_DB_FILE_NAME, derived_projection_dir, projection_sqlite_path};
pub use schema::{
    PROJECTION_BUILD_ID_V1, PROJECTION_SCHEMA_VERSION, apply_schema_v1, read_schema_version,
    validate_schema_v1,
};
