//! Active `.open` append and seal-to-`.seg` (ADR-0016 §§9/11).

#[cfg(unix)]
use std::fs::File;
use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

use cryptex_core::ids::DeviceId;
use cryptex_events::framing::{FRAME_HEADER_BYTES, decode_frame_header, encode_frame_header};

use crate::layout::{active_segment_path, canonical_events_device_dir, sealed_segment_path};
use crate::{StorageError, StorageResult};

/// Append one complete frame (header + body) to the device active `.open` pack.
///
/// Creates the pack file if missing. Syncs the file before returning success.
/// Callers must run recovery first when an untrusted suffix may exist.
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
    file.sync_all()?;
    let len = file.seek(SeekFrom::End(0))?;
    Ok(len)
}

/// Seal the active `.open` pack into an immutable `.seg` via exclusive rename.
///
/// Refuses if `.seg` already exists. Syncs `.open` before rename.
pub fn seal_active_segment(
    app_data_root: &Path,
    device_id: &DeviceId,
    segment_index: u64,
) -> StorageResult<()> {
    let open = active_segment_path(app_data_root, device_id, segment_index);
    let sealed = sealed_segment_path(app_data_root, device_id, segment_index);
    if !open.exists() {
        return Err(StorageError::InvalidLayout(format!(
            "active pack missing: {}",
            open.display()
        )));
    }
    if sealed.exists() {
        return Err(StorageError::InvalidLayout(format!(
            "sealed pack already exists: {}",
            sealed.display()
        )));
    }

    let lock_path = open.with_extension("open.lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)?;
    fs2::FileExt::lock_exclusive(&lock)?;

    {
        let file = OpenOptions::new().read(true).write(true).open(&open)?;
        file.sync_all()?;
    }

    rename_exclusive(&open, &sealed)?;
    #[cfg(unix)]
    {
        if let Some(parent) = sealed.parent() {
            let _ = File::open(parent).and_then(|d| d.sync_all());
        }
    }
    Ok(())
}

fn rename_exclusive(from: &Path, to: &Path) -> StorageResult<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
        let src: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
        let dst: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
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
                    to.display()
                )));
            }
            return Err(StorageError::Io(err));
        }
        Ok(())
    }
    #[cfg(unix)]
    {
        fs::hard_link(from, to).map_err(|err| {
            if err.kind() == std::io::ErrorKind::AlreadyExists {
                StorageError::InvalidLayout(format!("sealed pack already exists: {}", to.display()))
            } else {
                StorageError::Io(err)
            }
        })?;
        fs::remove_file(from)?;
        Ok(())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (from, to);
        Err(StorageError::InvalidLayout(
            "segment seal unsupported on this platform".into(),
        ))
    }
}

/// Read the entire active pack (tests / recovery helpers).
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
///
/// Returns `(frames, verified_prefix_len)` where frames are complete headers+bodies that
/// passed framing limits. Does not run body semantic validation.
pub fn scan_complete_frames(bytes: &[u8]) -> StorageResult<(Vec<Vec<u8>>, usize)> {
    let mut frames = Vec::new();
    let mut offset = 0usize;
    while offset + FRAME_HEADER_BYTES <= bytes.len() {
        let header = &bytes[offset..offset + FRAME_HEADER_BYTES];
        let (_ver, body_len) = match decode_frame_header(header) {
            Ok(v) => v,
            Err(err) => {
                // Complete-looking header region that fails framing: stop before it.
                let _ = err;
                return Ok((frames, offset));
            }
        };
        let body_end = offset
            .checked_add(FRAME_HEADER_BYTES)
            .and_then(|h| h.checked_add(body_len as usize))
            .ok_or_else(|| StorageError::Framing("frame length overflow".into()))?;
        if body_end > bytes.len() {
            // Truncated body: stop before this incomplete frame.
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
    fn append_seal_and_refuse_reseal() {
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
        let err = seal_active_segment(&root, &device, 0).unwrap_err();
        assert!(matches!(err, StorageError::InvalidLayout(_)));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_stops_before_truncated_tail() {
        let root = temp_root("trunc");
        let device = DeviceId::new("device-b").unwrap();
        append_frame(&root, &device, 0, b"abc").unwrap();
        let mut bytes = read_active_pack_bytes(&root, &device, 0).unwrap();
        bytes.extend_from_slice(b"CXE1"); // truncated header tail
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
}
