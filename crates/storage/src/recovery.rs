//! Active `.open` recovery: quarantine-before-truncate (ADR-0016 §7.2).

use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

use cryptex_core::ids::DeviceId;
use cryptex_events::framing::{FRAME_HEADER_BYTES, decode_frame_header};

use crate::layout::active_segment_path;
use crate::quarantine::{QuarantineEntry, QuarantineSourceKind, quarantine_bytes};
use crate::segment_io::scan_complete_frames;
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
///
/// Fail closed: on any quarantine/truncate/sync failure, leave the file unchanged
/// and refuse to report success.
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

    let bytes = fs::read(&path)?;
    let verified_len = verified_prefix_length(&bytes, validator)?;
    if verified_len == bytes.len() {
        return Ok(RecoveryReport {
            verified_prefix_len: verified_len as u64,
            quarantined: None,
            truncated: false,
        });
    }

    let suffix = &bytes[verified_len..];
    let reason = classify_suffix_reason(&bytes, verified_len);
    let source_relative = format!(
        "canonical/events/{}/{:020}.open",
        device_id.as_filename_stem(),
        segment_index
    );
    let entry = quarantine_bytes(
        app_data_root,
        Some(device_id),
        reason,
        QuarantineSourceKind::ActiveOpen,
        &source_relative,
        Some(verified_len as u64),
        suffix,
    )?;

    // Only after durable quarantine: truncate and sync.
    {
        let mut file = OpenOptions::new().write(true).open(&path)?;
        file.set_len(verified_len as u64)?;
        file.sync_all()?;
        let _ = file.seek(SeekFrom::Start(0));
        let _ = file.flush();
    }
    #[cfg(unix)]
    {
        if let Some(parent) = path.parent() {
            let _ = std::fs::File::open(parent).and_then(|d| d.sync_all());
        }
    }

    // Confirm truncate succeeded.
    let after = fs::metadata(&path)?.len();
    if after != verified_len as u64 {
        return Err(StorageError::Recovery(format!(
            "truncate confirmation failed: expected {verified_len} got {after}"
        )));
    }

    Ok(RecoveryReport {
        verified_prefix_len: verified_len as u64,
        quarantined: Some(entry),
        truncated: true,
    })
}

fn verified_prefix_length<V: FrameBodyValidator>(
    bytes: &[u8],
    validator: &V,
) -> StorageResult<usize> {
    let mut offset = 0usize;
    while offset + FRAME_HEADER_BYTES <= bytes.len() {
        let header = &bytes[offset..offset + FRAME_HEADER_BYTES];
        let (_ver, body_len) = match decode_frame_header(header) {
            Ok(v) => v,
            Err(_) => return Ok(offset),
        };
        let body_end = offset + FRAME_HEADER_BYTES + body_len as usize;
        if body_end > bytes.len() {
            return Ok(offset);
        }
        let body = &bytes[offset + FRAME_HEADER_BYTES..body_end];
        if let Err(err) = validator.validate_frame_body(body) {
            let _ = err;
            return Ok(offset);
        }
        offset = body_end;
    }
    let _ = scan_complete_frames; // keep helper linked for callers
    Ok(offset)
}

fn classify_suffix_reason(bytes: &[u8], verified_len: usize) -> &'static str {
    let rem = &bytes[verified_len..];
    if rem.len() < FRAME_HEADER_BYTES {
        return "truncated_tail";
    }
    match decode_frame_header(&rem[..FRAME_HEADER_BYTES]) {
        Ok((_, body_len)) => {
            let need = FRAME_HEADER_BYTES + body_len as usize;
            if rem.len() < need {
                "truncated_tail"
            } else {
                "corrupt_frame"
            }
        }
        Err(_) => "truncated_tail",
    }
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
        // Suffix starts at the corrupt frame through EOF.
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

        // Poison quarantine root as a file so create_dir_all fails.
        let qroot = crate::layout::quarantine_events_root(&root);
        fs::create_dir_all(qroot.parent().unwrap()).unwrap();
        fs::write(&qroot, b"not-a-dir").unwrap();

        let err = recover_active_open(&root, &device, 0, &AcceptAllBodies).unwrap_err();
        assert!(matches!(
            err,
            StorageError::Io(_) | StorageError::Quarantine(_)
        ));
        // Open file must remain unchanged.
        let after = fs::read(&path).unwrap();
        assert_eq!(after.len(), original_len + 4);
        let _ = fs::remove_dir_all(&root);
    }
}
