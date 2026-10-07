//! Canonical event storage layout helpers (M2).
//!
//! Authority: accepted ADR-0016 \u00a78\u2013\u00a79 and ADR-0013 sealed per-device segments.
//! These helpers build paths only; they do not write runtime data and must not be
//! used to place canonical bytes inside the Git repository.
//!
//! Slice 1 does not implement append/seal/quarantine I/O (later slices).

pub mod error;
pub mod layout;

pub use error::{StorageError, StorageResult};
pub use layout::{
    SegmentKind, active_segment_path, canonical_events_device_dir, quarantine_events_root,
    sealed_segment_path,
};
