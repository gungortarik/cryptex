//! Cryptex immutable event foundations (M2).
//!
//! Authority: accepted ADR-0016. This crate holds event-domain constants and
//! typed failures used by later M2 slices. Canonical writes remain `cryptexd`-only.
//!
//! Slice 1 scaffolds the workspace surface only: no hashing, signing, schema
//! generation, or segment I/O yet.

pub mod error;
pub mod framing;
pub mod limits;

pub use error::{EventError, EventResult};
pub use framing::{FRAME_HEADER_BYTES, FRAME_MAGIC, FRAME_VERSION_V1};
pub use limits::{
    DEFAULT_SEAL_EVENT_THRESHOLD_HINT, MAX_ACCEPTED_RECORD_BYTES, MAX_JSON_CONTAINER_ELEMENTS,
    MAX_JSON_NESTING_DEPTH, MAX_JSON_STRING_BYTES_PER_RECORD, MAX_MEMORY_NOTE_TEXT_BYTES,
};
