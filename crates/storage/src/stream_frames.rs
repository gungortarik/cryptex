//! Streaming complete-frame iteration over segment packs.

use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use cryptex_events::framing::{FRAME_HEADER_BYTES, decode_frame_header};
use cryptex_events::limits::MAX_ACCEPTED_RECORD_BYTES;

use crate::recovery::FrameBodyValidator;
use crate::{StorageError, StorageResult};

/// Fold over complete verified frames in `path` without loading the whole pack.
///
/// Stops before a truncated or framing-invalid region. Returns `(accumulator, verified_prefix_len)`.
/// Body allocation is capped by the ADR per-frame maximum via header decode.
pub fn fold_verified_frames<V, A, F>(
    path: &Path,
    validator: &V,
    mut acc: A,
    mut visit: F,
) -> StorageResult<(A, u64)>
where
    V: FrameBodyValidator,
    F: FnMut(&mut A, &[u8]) -> StorageResult<()>,
{
    if !path.exists() {
        return Ok((acc, 0));
    }
    let mut file = OpenOptions::new().read(true).open(path)?;
    let file_len = file.metadata()?.len();
    let mut offset = 0u64;
    let mut header = [0u8; FRAME_HEADER_BYTES];
    while offset + FRAME_HEADER_BYTES as u64 <= file_len {
        file.seek(SeekFrom::Start(offset))?;
        read_exact(&mut file, &mut header)?;
        let (_ver, body_len) = match decode_frame_header(&header) {
            Ok(v) => v,
            Err(_) => return Ok((acc, offset)),
        };
        debug_assert!(body_len <= MAX_ACCEPTED_RECORD_BYTES);
        let body_end = offset
            .checked_add(FRAME_HEADER_BYTES as u64)
            .and_then(|h| h.checked_add(u64::from(body_len)))
            .ok_or_else(|| StorageError::Framing("frame length overflow".into()))?;
        if body_end > file_len {
            return Ok((acc, offset));
        }
        let mut body = vec![0u8; body_len as usize];
        read_exact(&mut file, &mut body)?;
        if let Err(_err) = validator.validate_frame_body(&body) {
            return Ok((acc, offset));
        }
        visit(&mut acc, &body)?;
        offset = body_end;
    }
    Ok((acc, offset))
}

fn read_exact(file: &mut std::fs::File, buf: &mut [u8]) -> StorageResult<()> {
    let mut filled = 0usize;
    while filled < buf.len() {
        match file.read(&mut buf[filled..])? {
            0 => {
                return Err(StorageError::Framing(
                    "unexpected EOF while reading frame".into(),
                ));
            }
            n => filled += n,
        }
    }
    Ok(())
}
