//! Active `.open` append and seal-to-`.seg` (ADR-0016 §§9/11).

use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

use cryptex_core::ids::DeviceId;
use cryptex_events::framing::{FRAME_HEADER_BYTES, decode_frame_header, encode_frame_header};

use crate::durability::{files_byte_identical, sync_file, sync_parent_directory};
use crate::layout::{active_segment_path, canonical_events_device_dir, sealed_segment_path};
use crate::{StorageError, StorageResult};

/// Append one complete frame (header + body) to the device active `.open` pack.
pub fn append_frame(
    app_data_root: &Path,
    device_id: &DeviceId,
    segment_index: u64,
    body: &[u8],
) -> StorageResult<u64> {
    if body.len() > u32::MAX as usize {
        return Err(StorageError::Framing("body exceeds u32 length".into()));
    }
    let body_len = body.len() as u32;
    let header = encode_frame_header(body_len)?;
    let path = active_segment_path(app_data_root, device_id, segment_index);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let lock_path = path.with_extension("open.lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)?;
    fs2::FileExt::lock_exclusive(&lock)?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .read(true)
        .open(&path)?;
    file.write_all(&header)?;
    file.write_all(body)?;
    sync_file(&file)?;
    let len = file.seek(SeekFrom::End(0))?;
    Ok(len)
}

/// Seal the active `.open` pack into an immutable `.seg`.
///
/// Crash-safe and retryable: if both `.open` and `.seg` exist, contents must be
/// proven identical before removing `.open`; divergent pairs fail closed.
/// Required directory sync failures abort without reporting success.
pub fn seal_active_segment(
    app_data_root: &Path,
    device_id: &DeviceId,
    segment_index: u64,
) -> StorageResult<()> {
    let open = active_segment_path(app_data_root, device_id, segment_index);
    let sealed = sealed_segment_path(app_data_root, device_id, segment_index);

    let lock_path = open.with_extension("open.lock");
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)?;
    fs2::FileExt::lock_exclusive(&lock)?;

    let open_exists = open.exists();
    let sealed_exists = sealed.exists();
    match (open_exists, sealed_exists) {
        (false, false) => {
            return Err(StorageError::InvalidLayout(format!(
                "active pack missing: {}",
                open.display()
            )));
        }
        (false, true) => {
            // Prior seal completed; treat as idempotent success after parent sync.
            sync_parent_directory(&sealed)?;
            return Ok(());
        }
        (true, true) => {
            if !files_byte_identical(&open, &sealed)? {
                return Err(StorageError::InvalidLayout(format!(
                    "both .open and .seg exist with divergent contents for segment {segment_index}; refusing silent overwrite"
                )));
            }
            fs::remove_file(&open)?;
            sync_parent_directory(&sealed)?;
            return Ok(());
        }
        (true, false) => {}
    }

    {
        let file = OpenOptions::new().read(true).write(true).open(&open)?;
        sync_file(&file)?;
    }

    place_sealed_from_open(&open, &sealed)?;
    sync_parent_directory(&sealed)?;

    if open.exists() {
        // Unix hard-link path leaves `.open` until explicitly removed.
        if !files_byte_identical(&open, &sealed)? {
            return Err(StorageError::InvalidLayout(format!(
                "post-seal divergence between {} and {}",
                open.display(),
                sealed.display()
            )));
        }
        fs::remove_file(&open)?;
        sync_parent_directory(&sealed)?;
    }

    Ok(())
}

fn place_sealed_from_open(open: &Path, sealed: &Path) -> StorageResult<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
        let src: Vec<u16> = open.as_os_str().encode_wide().chain(Some(0)).collect();
        let dst: Vec<u16> = sealed.as_os_str().encode_wide().chain(Some(0)).collect();
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::MoveFileExW(
                src.as_ptr(),
                dst.as_ptr(),
                MOVEFILE_WRITE_THROUGH,
            )
        };
        if ok == 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(183) {
                return Err(StorageError::InvalidLayout(format!(
                    "sealed pack already exists: {}",
                    sealed.display()
                )));
            }
            return Err(StorageError::Io(err));
        }
        Ok(())
    }
    #[cfg(unix)]
    {
        match fs::hard_link(open, sealed) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                if files_byte_identical(open, sealed)? {
                    Ok(())
                } else {
                    Err(StorageError::InvalidLayout(format!(
                        "sealed pack already exists with divergent contents: {}",
                        sealed.display()
                    )))
                }
            }
            Err(err) => Err(StorageError::Io(err)),
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (open, sealed);
        Err(StorageError::InvalidLayout(
            "segment seal unsupported on this platform".into(),
        ))
    }
}

/// Read the entire active pack (small-pack tests only).
pub fn read_active_pack_bytes(
    app_data_root: &Path,
    device_id: &DeviceId,
    segment_index: u64,
) -> StorageResult<Vec<u8>> {
    let path = active_segment_path(app_data_root, device_id, segment_index);
    Ok(fs::read(path)?)
}

/// Ensure device canonical events directory exists.
pub fn ensure_device_events_dir(app_data_root: &Path, device_id: &DeviceId) -> StorageResult<()> {
    fs::create_dir_all(canonical_events_device_dir(app_data_root, device_id))?;
    Ok(())
}

/// Decode successive frames from `bytes` until EOF or incomplete trailing header/body.
pub fn scan_complete_frames(bytes: &[u8]) -> StorageResult<(Vec<Vec<u8>>, usize)> {
    let mut frames = Vec::new();
    let mut offset = 0usize;
    while offset + FRAME_HEADER_BYTES <= bytes.len() {
        let header = &bytes[offset..offset + FRAME_HEADER_BYTES];
        let (_ver, body_len) = match decode_frame_header(header) {
            Ok(v) => v,
            Err(_err) => return Ok((frames, offset)),
        };
        let body_end = offset
            .checked_add(FRAME_HEADER_BYTES)
            .and_then(|h| h.checked_add(body_len as usize))
            .ok_or_else(|| StorageError::Framing("frame length overflow".into()))?;
        if body_end > bytes.len() {
            return Ok((frames, offset));
        }
        frames.push(bytes[offset..body_end].to_vec());
        offset = body_end;
    }
    Ok((frames, offset))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|t| t.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "cryptex-m2-seg-{}-{}-{}",
            label,
            std::process::id(),
            nanos
        ))
    }

    #[test]
    fn append_seal_and_refuse_reseal_when_open_gone() {
        let root = temp_root("append-seal");
        let device = DeviceId::new("device-a").unwrap();
        append_frame(&root, &device, 0, b"{\"ok\":true}").unwrap();
        append_frame(&root, &device, 0, b"{\"ok\":2}").unwrap();
        let bytes = read_active_pack_bytes(&root, &device, 0).unwrap();
        let (frames, prefix) = scan_complete_frames(&bytes).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(prefix, bytes.len());
        seal_active_segment(&root, &device, 0).unwrap();
        assert!(!active_segment_path(&root, &device, 0).exists());
        assert!(sealed_segment_path(&root, &device, 0).is_file());
        // Idempotent when only .seg remains.
        seal_active_segment(&root, &device, 0).unwrap();
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_stops_before_truncated_tail() {
        let root = temp_root("trunc");
        let device = DeviceId::new("device-b").unwrap();
        append_frame(&root, &device, 0, b"abc").unwrap();
        let mut bytes = read_active_pack_bytes(&root, &device, 0).unwrap();
        bytes.extend_from_slice(b"CXE1");
        let path = active_segment_path(&root, &device, 0);
        fs::write(&path, &bytes).unwrap();
        let (frames, prefix) = scan_complete_frames(&bytes).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(prefix, frames[0].len());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn concurrent_appends_preserve_complete_frames() {
        use std::sync::{Arc, Barrier};
        use std::thread;
        let root = temp_root("conc");
        let device = DeviceId::new("device-c").unwrap();
        let barrier = Arc::new(Barrier::new(6));
        let mut handles = Vec::new();
        for i in 0..6 {
            let root = root.clone();
            let device = device.clone();
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier.wait();
                let body = format!(r#"{{"n":{i}}}"#);
                append_frame(&root, &device, 0, body.as_bytes())
            }));
        }
        for h in handles {
            h.join().unwrap().unwrap();
        }
        let bytes = read_active_pack_bytes(&root, &device, 0).unwrap();
        let (frames, prefix) = scan_complete_frames(&bytes).unwrap();
        assert_eq!(frames.len(), 6);
        assert_eq!(prefix, bytes.len());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn seal_reconciles_identical_open_and_seg_pair() {
        let root = temp_root("reconcile");
        let device = DeviceId::new("device-d").unwrap();
        append_frame(&root, &device, 0, b"payload").unwrap();
        let open = active_segment_path(&root, &device, 0);
        let sealed = sealed_segment_path(&root, &device, 0);
        fs::copy(&open, &sealed).unwrap();
        assert!(open.exists() && sealed.exists());
        seal_active_segment(&root, &device, 0).unwrap();
        assert!(!open.exists());
        assert!(sealed.is_file());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn seal_fails_closed_on_divergent_open_and_seg() {
        let root = temp_root("diverge");
        let device = DeviceId::new("device-e").unwrap();
        append_frame(&root, &device, 0, b"payload-a").unwrap();
        let open = active_segment_path(&root, &device, 0);
        let sealed = sealed_segment_path(&root, &device, 0);
        fs::write(&sealed, b"different-bytes").unwrap();
        let err = seal_active_segment(&root, &device, 0).unwrap_err();
        assert!(matches!(err, StorageError::InvalidLayout(_)), "{err}");
        assert!(open.exists());
        assert!(sealed.exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn seal_propagates_directory_sync_failure() {
        let root = temp_root("seal-sync");
        let device = DeviceId::new("device-f").unwrap();
        append_frame(&root, &device, 0, b"payload").unwrap();
        crate::durability::fail_next_directory_sync();
        let err = seal_active_segment(&root, &device, 0).unwrap_err();
        assert!(matches!(err, StorageError::Durability(_)), "{err}");
        let _ = fs::remove_dir_all(&root);
    }
}
