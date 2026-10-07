//! Cryptex immutable event foundations (M2).
//!
//! Authority: accepted ADR-0016. Canonical writes remain `cryptexd`-only.

pub mod device_seq;
pub mod envelope;
pub mod error;
pub mod framing;
pub mod hex;
pub mod limits;
pub mod schema;

pub use device_seq::{DEVICE_SEQ_PATTERN, DeviceSeq};
pub use envelope::{
    AcceptedEvent, Classification, EVENT_SCHEMA_VERSION, EventPayload, EventType,
    MemoryNotePayload, SignableEvent,
};
pub use error::{EventError, EventResult};
pub use framing::{
    FRAME_HEADER_BYTES, FRAME_MAGIC, FRAME_VERSION_V1, decode_frame_header, encode_frame_header,
};
pub use hex::{ContentHashHex, SignatureHex, SigningKeyId, ZERO_CONTENT_HASH_HEX};
pub use limits::{
    DEFAULT_SEAL_EVENT_THRESHOLD_HINT, MAX_ACCEPTED_RECORD_BYTES, MAX_JSON_CONTAINER_ELEMENTS,
    MAX_JSON_NESTING_DEPTH, MAX_JSON_STRING_BYTES_PER_RECORD, MAX_MEMORY_NOTE_TEXT_BYTES,
    check_body_len,
};
