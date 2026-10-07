//! Canonical event storage layout and segment I/O (M2).
//!
//! Authority: accepted ADR-0016. Runtime fixtures must stay outside the Git repository.
//! Append/seal/quarantine helpers are intended for cryptexd-only callers.

pub mod durability;
pub mod error;
pub mod layout;
pub mod quarantine;
pub mod recovery;
pub mod segment_io;
pub mod stream_frames;
pub mod timefmt;

pub use error::{StorageError, StorageResult};
pub use layout::{
    SegmentKind, active_segment_path, canonical_events_device_dir, event_signing_key_path,
    quarantine_events_root, sealed_segment_path,
};
pub use quarantine::{
    QuarantineEntry, QuarantineMetadata, QuarantineSourceKind, quarantine_bytes,
    quarantine_file_range,
};
pub use recovery::{
    AcceptAllBodies, AllowListedBodies, FrameBodyValidator, RecoveryReport, recover_active_open,
};
pub use segment_io::{
    append_frame, ensure_device_events_dir, read_active_pack_bytes, scan_complete_frames,
    seal_active_segment,
};
pub use stream_frames::fold_verified_frames;
