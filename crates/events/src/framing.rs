//! Versioned segment record framing constants (ADR-0016 §4.4).

/// On-disk magic for production event frames.
pub const FRAME_MAGIC: &[u8; 4] = b"CXE1";

/// Little-endian frame version for schema_version / frame_ver pairings in M2.
pub const FRAME_VERSION_V1: u16 = 1;

/// Bytes in `magic (4) + frame_ver (2) + body_len (4)` before the body.
pub const FRAME_HEADER_BYTES: usize = 10;

/// Encode a frame header for `body_len` without allocating the body.
pub fn encode_frame_header(body_len: u32) -> [u8; FRAME_HEADER_BYTES] {
    let mut header = [0_u8; FRAME_HEADER_BYTES];
    header[0..4].copy_from_slice(FRAME_MAGIC);
    header[4..6].copy_from_slice(&FRAME_VERSION_V1.to_le_bytes());
    header[6..10].copy_from_slice(&body_len.to_le_bytes());
    header
}

/// Decode and validate a frame header. Does not allocate a body buffer.
pub fn decode_frame_header(bytes: &[u8]) -> crate::EventResult<(u16, u32)> {
    use crate::EventError;

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
    Ok((version, body_len))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::MAX_ACCEPTED_RECORD_BYTES;

    #[test]
    fn round_trips_header_and_rejects_bad_magic() {
        let header = encode_frame_header(42);
        let (version, body_len) = decode_frame_header(&header).expect("header");
        assert_eq!(version, FRAME_VERSION_V1);
        assert_eq!(body_len, 42);

        let mut bad = header;
        bad[0] = b'X';
        assert!(matches!(
            decode_frame_header(&bad),
            Err(crate::EventError::Framing(_))
        ));
    }

    #[test]
    fn truncated_header_fails_closed() {
        let header = encode_frame_header(1);
        let err = decode_frame_header(&header[..FRAME_HEADER_BYTES - 1]).unwrap_err();
        assert!(matches!(err, crate::EventError::Framing(_)));
    }

    #[test]
    fn body_len_may_equal_accepted_record_limit() {
        let header = encode_frame_header(MAX_ACCEPTED_RECORD_BYTES);
        let (_, body_len) = decode_frame_header(&header).unwrap();
        assert_eq!(body_len, MAX_ACCEPTED_RECORD_BYTES);
    }
}
