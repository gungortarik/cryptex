//! Versioned segment record framing constants (ADR-0016 §4.4).

use crate::limits::{MAX_ACCEPTED_RECORD_BYTES, check_body_len};
use crate::{EventError, EventResult};

/// On-disk magic for production event frames.
pub const FRAME_MAGIC: &[u8; 4] = b"CXE1";

/// Little-endian frame version for schema_version / frame_ver pairings in M2.
pub const FRAME_VERSION_V1: u16 = 1;

/// Bytes in `magic (4) + frame_ver (2) + body_len (4)` before the body.
pub const FRAME_HEADER_BYTES: usize = 10;

/// Encode a frame header for `body_len` without allocating the body.
///
/// Rejects `body_len` above [`MAX_ACCEPTED_RECORD_BYTES`] before writing any header bytes.
pub fn encode_frame_header(body_len: u32) -> EventResult<[u8; FRAME_HEADER_BYTES]> {
    check_body_len(body_len)?;
    let mut header = [0_u8; FRAME_HEADER_BYTES];
    header[0..4].copy_from_slice(FRAME_MAGIC);
    header[4..6].copy_from_slice(&FRAME_VERSION_V1.to_le_bytes());
    header[6..10].copy_from_slice(&body_len.to_le_bytes());
    Ok(header)
}

/// Decode and validate a frame header. Does not allocate a body buffer.
///
/// Rejects `body_len` above [`MAX_ACCEPTED_RECORD_BYTES`] so callers never allocate
/// from an oversized length discovered in the header.
pub fn decode_frame_header(bytes: &[u8]) -> EventResult<(u16, u32)> {
    if bytes.len() < FRAME_HEADER_BYTES {
        return Err(EventError::Framing("frame header is truncated".to_string()));
    }
    if &bytes[0..4] != FRAME_MAGIC.as_slice() {
        return Err(EventError::Framing("frame magic mismatch".to_string()));
    }
    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if version != FRAME_VERSION_V1 {
        return Err(EventError::Framing(format!(
            "unsupported frame_ver {version} (expected {FRAME_VERSION_V1})"
        )));
    }
    let body_len = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]);
    check_body_len(body_len)?;
    Ok((version, body_len))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_header_and_rejects_bad_magic() {
        let header = encode_frame_header(42).expect("header");
        let (version, body_len) = decode_frame_header(&header).expect("header");
        assert_eq!(version, FRAME_VERSION_V1);
        assert_eq!(body_len, 42);

        let mut bad = header;
        bad[0] = b'X';
        assert!(matches!(
            decode_frame_header(&bad),
            Err(EventError::Framing(_))
        ));
    }

    #[test]
    fn truncated_header_fails_closed() {
        let header = encode_frame_header(1).expect("header");
        let err = decode_frame_header(&header[..FRAME_HEADER_BYTES - 1]).unwrap_err();
        assert!(matches!(err, EventError::Framing(_)));
    }

    #[test]
    fn encode_accepts_body_len_at_limit_and_rejects_above() {
        let ok = encode_frame_header(MAX_ACCEPTED_RECORD_BYTES).expect("at limit");
        let (_, body_len) = decode_frame_header(&ok).expect("decode at limit");
        assert_eq!(body_len, MAX_ACCEPTED_RECORD_BYTES);

        let err = encode_frame_header(MAX_ACCEPTED_RECORD_BYTES + 1).unwrap_err();
        assert!(matches!(err, EventError::LimitExceeded(_)));
    }

    #[test]
    fn decode_accepts_body_len_at_limit_and_rejects_above() {
        let at_limit = encode_frame_header(MAX_ACCEPTED_RECORD_BYTES).expect("encode");
        let (_, body_len) = decode_frame_header(&at_limit).expect("decode at limit");
        assert_eq!(body_len, MAX_ACCEPTED_RECORD_BYTES);

        // Craft an otherwise-valid header with an oversized body_len that encode refuses.
        let mut oversized = at_limit;
        oversized[6..10].copy_from_slice(&(MAX_ACCEPTED_RECORD_BYTES + 1).to_le_bytes());
        let err = decode_frame_header(&oversized).unwrap_err();
        assert!(matches!(err, EventError::LimitExceeded(_)));
    }
}
