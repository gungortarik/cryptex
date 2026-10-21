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
    acc: A,
    mut visit: F,
) -> StorageResult<(A, u64)>
where
    V: FrameBodyValidator,
    F: FnMut(&mut A, &[u8]) -> StorageResult<()>,
{
    fold_verified_frames_with_offsets(path, validator, acc, |acc, _, body| visit(acc, body))
}

/// Fold complete verified frames while reporting each frame's byte offset.
///
/// The offset is the position of the frame header in the original pack. This
/// is useful for derived materialization watermarks; it does not rewrite or
/// otherwise change the pack scan/recovery behavior.
pub fn fold_verified_frames_with_offsets<V, A, F>(
    path: &Path,
    validator: &V,
    mut acc: A,
    mut visit: F,
) -> StorageResult<(A, u64)>
where
    V: FrameBodyValidator,
    F: FnMut(&mut A, u64, &[u8]) -> StorageResult<()>,
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
        if let Err(err) = validator.validate_frame_body(&body) {
            if matches!(&err, StorageError::CompatibilityBoundary(_)) {
                return Err(err);
            }
            return Ok((acc, offset));
        }
        visit(&mut acc, offset, &body)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recovery::AcceptAllBodies;
    use cryptex_events::framing::encode_frame_header;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static ROOT_SEQ: AtomicU64 = AtomicU64::new(1);

    fn temp_file() -> (std::path::PathBuf, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "cryptex-frame-offset-{}-{}",
            std::process::id(),
            ROOT_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        (root.clone(), root.join("frames.open"))
    }

    #[test]
    fn reports_original_frame_header_offsets() {
        let (root, path) = temp_file();
        let mut bytes = Vec::new();
        for body in [b"first".as_slice(), b"second".as_slice()] {
            bytes.extend_from_slice(&encode_frame_header(body.len() as u32).unwrap());
            bytes.extend_from_slice(body);
        }
        fs::write(&path, &bytes).unwrap();
        let (offsets, verified_len) = fold_verified_frames_with_offsets(
            &path,
            &AcceptAllBodies,
            Vec::new(),
            |offsets, offset, body| {
                offsets.push((offset, body.to_vec()));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(verified_len, bytes.len() as u64);
        assert_eq!(
            offsets,
            vec![
                (0, b"first".to_vec()),
                (
                    (cryptex_events::framing::FRAME_HEADER_BYTES + 5) as u64,
                    b"second".to_vec(),
                ),
            ]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn offsets_helper_keeps_existing_truncated_prefix_behavior() {
        let (root, path) = temp_file();
        let mut bytes = encode_frame_header(5).unwrap().to_vec();
        bytes.extend_from_slice(b"good!");
        let second_offset = bytes.len() as u64;
        bytes.extend_from_slice(&encode_frame_header(8).unwrap());
        bytes.extend_from_slice(b"short");
        fs::write(&path, &bytes).unwrap();
        let (offsets, verified_len) = fold_verified_frames_with_offsets(
            &path,
            &AcceptAllBodies,
            Vec::new(),
            |offsets, offset, _| {
                offsets.push(offset);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(offsets, vec![0]);
        assert_eq!(verified_len, second_offset);
        fs::remove_dir_all(root).unwrap();
    }
}
