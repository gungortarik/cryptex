//! Disposable local projection foundations (M3).
//!
//! Authority: accepted M3 contract under `docs/milestones/M3_LOCAL_PROJECTION_SEARCH.md`
//! and `evidence/m3/m3-phase0-implementation-plan-20261008.md`.
//!
//! SQLite/FTS state under these paths is derived and rebuildable. Canonical event
//! packs remain authoritative. This crate must not write `.open`/`.seg` files.
//!
//! Slice 3 provides empty schema v1 open/create and schema-version validation.
//! Event ingestion, rebuild/checkpoint advancement, and search APIs are deferred.

pub mod db;
pub mod error;
pub mod fts;
pub mod layout;
pub mod schema;

pub use db::ProjectionDb;
pub use error::{ProjectionError, ProjectionResult};
pub use fts::{ensure_fts5_available, probe_bundled_fts5};
pub use layout::{PROJECTION_DB_FILE_NAME, derived_projection_dir, projection_sqlite_path};
pub use schema::{
    PROJECTION_BUILD_ID_V1, PROJECTION_SCHEMA_VERSION, apply_schema_v1, read_schema_version,
    validate_schema_v1,
};
