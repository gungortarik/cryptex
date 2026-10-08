//! Disposable local projection foundations (M3).
//!
//! Authority: accepted M3 contract under `docs/milestones/M3_LOCAL_PROJECTION_SEARCH.md`
//! and `evidence/m3/m3-phase0-implementation-plan-20261008.md`.
//!
//! SQLite/FTS state under these paths is derived and rebuildable. Canonical event
//! packs remain authoritative. This crate must not write `.open`/`.seg` files.
//! Product SQLite pins arrive in a later slice; Slice 1 is path scaffolding only.

pub mod layout;

pub use layout::{PROJECTION_DB_FILE_NAME, derived_projection_dir, projection_sqlite_path};
