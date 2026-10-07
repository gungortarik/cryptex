//! Active `.open` recovery: quarantine-before-truncate (ADR-0016 §7.2).
//!
//! Streams verified frames with per-frame body allocation capped by the ADR
//! maximum; copies the untrusted suffix into quarantine without loading the
//! whole pack into memory.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use cryptex_core::ids::DeviceId;
use cryptex_events::framing::{FRAME_HEADER_BYTES, decode_frame_header};
use cryptex_events::limits::MAX_ACCEPTED_RECORD_BYTES;

use crate::durability::{sync_file, sync_parent_directory};
use crate::layout::active_segment_path;
use crate::quarantine::{QuarantineEntry, QuarantineSourceKind, quarantine_file_range};
use crate::{StorageError, StorageResult};

/// Validates a complete frame body after framing/limit checks succeed.
pub trait FrameBodyValidator {
    fn validate_frame_body(&self, body: &[u8]) -> StorageResult<()>;
}

/// Accept every framed body (framing-only recovery tests).
#[derive(Debug, Default, Clone, Copy)]
pub struct AcceptAllBodies;

impl FrameBodyValidator for AcceptAllBodies {
    fn validate_frame_body(&self, _body: &[u8]) -> StorageResult<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryReport {
    pub verified_prefix_len: u64,
    pub quarantined: Option<QuarantineEntry>,
    pub truncated: bool,
}

/// Recover an active `.open` pack: quarantine untrusted suffix, then truncate.
pub fn recover_active_open<V: FrameBodyValidator>(
    app_data_root: &Path,
    device_id: &DeviceId,
    segment_index: u64,
    validator: &V,
) -> StorageResult<RecoveryReport> {
    let path = active_segment_path(app_data_root, device_id, segment_index);
    if !path.exists() {
        return Ok(RecoveryReport {
            verified_prefix_len: 0,
            quarantined: None,
            truncated: false,
        });
    }

    let lock_path = path.with_extension("open.lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)?;
    fs2::FileExt::lock_exclusive(&lock)?;

    let mut file = OpenOptions::new().read(true).write(true).open(&path)?;
    let file_len = file.metadata()?.len();
    let (verified_len, reason) = stream_verified_prefix_length(&mut file, file_len, validator)?;
    if verified_len == file_len {
        return Ok(RecoveryReport {
            verified_prefix_len: verified_len,
            quarantined: None,
            truncated: false,
        });
    }

    let source_relative = format!(
        "canonical/events/{}/{:020}.open",
        device_id.as_filename_stem(),
        segment_index
    );
    let entry = quarantine_file_range(
        app_data_root,
        Some(device_id),
        reason,
        QuarantineSourceKind::ActiveOpen,
        &source_relative,
        verified_len,
        file_len,
        &mut file,
    )?;

    file.set_len(verified_len)?;
    sync_file(&file)?;
    sync_parent_directory(&path)?;

    let after = file.metadata()?.len();
    if after != verified_len {
        return Err(StorageError::Recovery(format!(
            "truncate confirmation failed: expected {verified_len} got {after}"
        )));
    }

    Ok(RecoveryReport {
        verified_prefix_len: verified_len,
        quarantined: Some(entry),
        truncated: true,
    })
}

fn stream_verified_prefix_length<V: FrameBodyValidator>(
    file: &mut File,
    file_len: u64,
    validator: &V,
) -> StorageResult<(u64, &'static str)> {
    let mut offset = 0u64;
    let mut header = [0u8; FRAME_HEADER_BYTES];
    while offset + FRAME_HEADER_BYTES as u64 <= file_len {
        file.seek(SeekFrom::Start(offset))?;
        read_exact(file, &mut header)?;
        let (_ver, body_len) = match decode_frame_header(&header) {
            Ok(v) => v,
            Err(_) => return Ok((offset, "truncated_tail")),
        };
        // decode_frame_header already enforces MAX_ACCEPTED_RECORD_BYTES.
        debug_assert!(body_len <= MAX_ACCEPTED_RECORD_BYTES);
        let body_end = offset
            .checked_add(FRAME_HEADER_BYTES as u64)
            .and_then(|h| h.checked_add(u64::from(body_len)))
            .ok_or_else(|| StorageError::Framing("frame length overflow".into()))?;
        if body_end > file_len {
            return Ok((offset, "truncated_tail"));
        }
        let mut body = vec![0u8; body_len as usize];
        read_exact(file, &mut body)?;
        if let Err(_err) = validator.validate_frame_body(&body) {
            let reason = if body_end <= file_len {
                "corrupt_frame"
            } else {
                "truncated_tail"
            };
            return Ok((offset, reason));
        }
        offset = body_end;
    }
    if offset < file_len {
        return Ok((offset, "truncated_tail"));
    }
    Ok((offset, "truncated_tail"))
}

fn read_exact(file: &mut File, buf: &mut [u8]) -> StorageResult<()> {
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

/// Test helper: validate bodies equal to an allowed set.
#[derive(Debug, Clone)]
pub struct AllowListedBodies {
    pub allowed: Vec<Vec<u8>>,
}

impl FrameBodyValidator for AllowListedBodies {
    fn validate_frame_body(&self, body: &[u8]) -> StorageResult<()> {
        if self.allowed.iter().any(|b| b.as_slice() == body) {
            Ok(())
        } else {
            Err(StorageError::Integrity("body not allow-listed".into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment_io::{append_frame, read_active_pack_bytes};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|t| t.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "cryptex-m2-rec-{}-{}-{}",
            label,
            std::process::id(),
            nanos
        ))
    }

    #[test]
    fn quarantine_before_truncate_on_truncated_tail() {
        let root = temp_root("trunc-rec");
        let device = DeviceId::new("device-r").unwrap();
        append_frame(&root, &device, 0, b"good").unwrap();
        let mut bytes = read_active_pack_bytes(&root, &device, 0).unwrap();
        let good_len = bytes.len();
        bytes.extend_from_slice(b"CXE1TRUNC");
        let path = active_segment_path(&root, &device, 0);
        fs::write(&path, &bytes).unwrap();

        let report = recover_active_open(&root, &device, 0, &AcceptAllBodies).unwrap();
        assert!(report.truncated);
        assert_eq!(report.verified_prefix_len, good_len as u64);
        let entry = report.quarantined.unwrap();
        let object = fs::read(entry.directory.join("object.bin")).unwrap();
        assert_eq!(object, b"CXE1TRUNC");
        let after = fs::read(&path).unwrap();
        assert_eq!(after.len(), good_len);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn corrupt_complete_frame_quarantines_frame_and_rest() {
        let root = temp_root("corrupt");
        let device = DeviceId::new("device-s").unwrap();
        append_frame(&root, &device, 0, b"keep").unwrap();
        append_frame(&root, &device, 0, b"bad").unwrap();
        append_frame(&root, &device, 0, b"also").unwrap();
        let bytes = read_active_pack_bytes(&root, &device, 0).unwrap();
        let validator = AllowListedBodies {
            allowed: vec![b"keep".to_vec()],
        };
        let report = recover_active_open(&root, &device, 0, &validator).unwrap();
        assert!(report.truncated);
        let entry = report.quarantined.unwrap();
        assert_eq!(entry.metadata.reason_code, "corrupt_frame");
        let object = fs::read(entry.directory.join("object.bin")).unwrap();
        assert!(object.len() < bytes.len());
        assert!(!object.is_empty());
        let after = read_active_pack_bytes(&root, &device, 0).unwrap();
        assert_eq!(after.len() as u64, report.verified_prefix_len);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn fails_closed_if_quarantine_cannot_be_created() {
        let root = temp_root("fail-q");
        let device = DeviceId::new("device-t").unwrap();
        append_frame(&root, &device, 0, b"good").unwrap();
        let mut bytes = read_active_pack_bytes(&root, &device, 0).unwrap();
        let original_len = bytes.len();
        bytes.extend_from_slice(b"TAIL");
        let path = active_segment_path(&root, &device, 0);
        fs::write(&path, &bytes).unwrap();

        let qroot = crate::layout::quarantine_events_root(&root);
        fs::create_dir_all(qroot.parent().unwrap()).unwrap();
        fs::write(&qroot, b"not-a-dir").unwrap();

        let err = recover_active_open(&root, &device, 0, &AcceptAllBodies).unwrap_err();
        assert!(matches!(
            err,
            StorageError::Io(_) | StorageError::Quarantine(_) | StorageError::Durability(_)
        ));
        let after = fs::read(&path).unwrap();
        assert_eq!(after.len(), original_len + 4);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn streaming_recovery_handles_multi_megabyte_suffix() {
        let root = temp_root("big-suffix");
        let device = DeviceId::new("device-u").unwrap();
        append_frame(&root, &device, 0, b"keep").unwrap();
        let path = active_segment_path(&root, &device, 0);
        let good_len = fs::metadata(&path).unwrap().len();
        {
            use std::io::Write;
            let mut file = OpenOptions::new().append(true).open(&path).unwrap();
            let chunk = vec![b'X'; 1024 * 256];
            for _ in 0..8 {
                file.write_all(&chunk).unwrap();
            }
            // ~2 MiB suffix without forming a valid frame header.
        }
        let report = recover_active_open(&root, &device, 0, &AcceptAllBodies).unwrap();
        assert!(report.truncated);
        assert_eq!(report.verified_prefix_len, good_len);
        assert_eq!(fs::metadata(&path).unwrap().len(), good_len);
        let entry = report.quarantined.unwrap();
        assert_eq!(entry.metadata.byte_length, 1024 * 256 * 8);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn recovery_propagates_directory_sync_failure_without_truncate() {
        let root = temp_root("rec-sync");
        let device = DeviceId::new("device-v").unwrap();
        append_frame(&root, &device, 0, b"good").unwrap();
        let path = active_segment_path(&root, &device, 0);
        let mut bytes = fs::read(&path).unwrap();
        let original = bytes.len();
        bytes.extend_from_slice(b"TAIL");
        fs::write(&path, &bytes).unwrap();

        // Fail the first directory sync inside quarantine so recovery aborts before truncate.
        crate::durability::fail_next_directory_sync();
        let err = recover_active_open(&root, &device, 0, &AcceptAllBodies).unwrap_err();
        assert!(matches!(err, StorageError::Durability(_)), "{err}");
        let after = fs::read(&path).unwrap();
        assert_eq!(after.len(), original + 4);
        let _ = fs::remove_dir_all(&root);
    }
}
