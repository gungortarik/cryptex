//! Quarantine for untrusted canonical-event bytes (ADR-0016 §8).
//!
//! Streaming entry creation hashes and copies rejected bytes without requiring
//! the full suffix to be held in memory by the caller.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use cryptex_core::ids::DeviceId;
use serde::{Deserialize, Serialize};

use crate::durability::{sync_directory, sync_file, sync_parent_directory};
use crate::layout::quarantine_events_root;
use crate::{StorageError, StorageResult};

/// Optional diagnostic fields for quarantine.json (ADR-0016 §8).
#[derive(Debug, Clone, Default)]
pub struct QuarantineExtras {
    pub schema_version_if_parsed: Option<u32>,
    pub content_hash_if_computed: Option<String>,
}

/// Common quarantine attribution/metadata inputs (keeps public helpers under Clippy arity limits).
#[derive(Debug, Clone)]
pub struct QuarantineRequest<'a> {
    pub app_data_root: &'a Path,
    pub device_id: Option<&'a DeviceId>,
    pub reason_code: &'a str,
    pub source_kind: QuarantineSourceKind,
    pub source_relative: &'a str,
    pub byte_offset: Option<u64>,
    pub extras: QuarantineExtras,
}

static ENTRY_SUFFIX: AtomicU64 = AtomicU64::new(1);
const COPY_BUF: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuarantineSourceKind {
    ActiveOpen,
    SealedSeg,
    Import,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuarantineMetadata {
    pub schema_version: u32,
    pub reason_code: String,
    pub detected_at: String,
    pub attribution: Option<String>,
    pub entry_id: String,
    pub byte_length: u64,
    pub source_kind: QuarantineSourceKind,
    pub source_relative: String,
    pub byte_offset: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_version_if_parsed: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash_if_computed: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarantineEntry {
    pub directory: PathBuf,
    pub metadata: QuarantineMetadata,
}

/// Quarantine a byte slice (small helpers/tests). Prefer [`quarantine_file_range`] for packs.
pub fn quarantine_bytes(
    request: QuarantineRequest<'_>,
    bytes: &[u8],
) -> StorageResult<QuarantineEntry> {
    if bytes.is_empty() {
        return Err(StorageError::Quarantine(
            "refusing to quarantine empty suffix".into(),
        ));
    }
    let entry_id = lowercase_hex(blake3::hash(bytes).as_bytes());
    let byte_length = bytes.len() as u64;
    finish_quarantine_entry(request, entry_id, byte_length, |object| {
        object.write_all(bytes)?;
        Ok(())
    })
}

/// Stream-copy `[start, end)` from `source` into quarantine while hashing.
///
/// Does not require the caller to hold the rejected range in memory.
pub fn quarantine_file_range(
    request: QuarantineRequest<'_>,
    start: u64,
    end: u64,
    source: &mut File,
) -> StorageResult<QuarantineEntry> {
    if end <= start {
        return Err(StorageError::Quarantine(
            "refusing to quarantine empty suffix".into(),
        ));
    }
    let byte_length = end - start;
    let q_parent = attribution_parent(request.app_data_root, request.device_id)?;
    fs::create_dir_all(&q_parent)?;
    sync_directory(&q_parent)?;

    let staging = q_parent.join(format!(
        ".staging-{}-{}-{}",
        std::process::id(),
        ENTRY_SUFFIX.fetch_add(1, Ordering::Relaxed),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|t| t.as_nanos())
            .unwrap_or(0)
    ));

    let mut hasher = blake3::Hasher::new();
    {
        let mut out = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)?;
        source.seek(SeekFrom::Start(start))?;
        let mut remaining = byte_length;
        let mut buf = vec![0u8; COPY_BUF];
        while remaining > 0 {
            let chunk = core::cmp::min(COPY_BUF as u64, remaining) as usize;
            read_exact_some(source, &mut buf[..chunk])?;
            hasher.update(&buf[..chunk]);
            out.write_all(&buf[..chunk])?;
            remaining -= chunk as u64;
        }
        sync_file(&out)?;
    }

    let entry_id = lowercase_hex(hasher.finalize().as_bytes());
    let utc_compact = utc_compact_now();
    let base = q_parent.join(format!("{utc_compact}_{entry_id}"));
    let dir = exclusive_create_dir(&base)?;
    sync_parent_directory(&dir.join("object.bin"))?;

    let object_path = dir.join("object.bin");
    place_staging_as_object(&staging, &object_path)?;
    // Re-open and sync destination object for platforms where rename does not imply durability.
    {
        let object = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&object_path)?;
        sync_file(&object)?;
    }

    let metadata = QuarantineMetadata {
        schema_version: 1,
        reason_code: request.reason_code.to_string(),
        detected_at: format_detected_at(&utc_compact),
        attribution: request.device_id.map(|d| d.as_str().to_string()),
        entry_id: entry_id.clone(),
        byte_length,
        source_kind: request.source_kind,
        source_relative: request.source_relative.to_string(),
        byte_offset: Some(start),
        schema_version_if_parsed: request.extras.schema_version_if_parsed,
        content_hash_if_computed: request.extras.content_hash_if_computed,
    };
    write_metadata_file(&dir, &metadata)?;
    sync_directory(&dir)?;
    sync_parent_directory(&dir.join("object.bin"))?;

    Ok(QuarantineEntry {
        directory: dir,
        metadata,
    })
}

fn finish_quarantine_entry<F>(
    request: QuarantineRequest<'_>,
    entry_id: String,
    byte_length: u64,
    write_object: F,
) -> StorageResult<QuarantineEntry>
where
    F: FnOnce(&mut File) -> std::io::Result<()>,
{
    let q_parent = attribution_parent(request.app_data_root, request.device_id)?;
    fs::create_dir_all(&q_parent)?;
    sync_directory(&q_parent)?;

    let utc_compact = utc_compact_now();
    let base = q_parent.join(format!("{utc_compact}_{entry_id}"));
    let dir = exclusive_create_dir(&base)?;
    sync_parent_directory(&dir.join("object.bin"))?;

    let object_path = dir.join("object.bin");
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&object_path)?;
        write_object(&mut file)?;
        sync_file(&file)?;
    }

    let metadata = QuarantineMetadata {
        schema_version: 1,
        reason_code: request.reason_code.to_string(),
        detected_at: format_detected_at(&utc_compact),
        attribution: request.device_id.map(|d| d.as_str().to_string()),
        entry_id,
        byte_length,
        source_kind: request.source_kind,
        source_relative: request.source_relative.to_string(),
        byte_offset: request.byte_offset,
        schema_version_if_parsed: request.extras.schema_version_if_parsed,
        content_hash_if_computed: request.extras.content_hash_if_computed,
    };
    write_metadata_file(&dir, &metadata)?;
    sync_directory(&dir)?;
    sync_parent_directory(&dir.join("object.bin"))?;

    Ok(QuarantineEntry {
        directory: dir,
        metadata,
    })
}

fn write_metadata_file(dir: &Path, metadata: &QuarantineMetadata) -> StorageResult<()> {
    let meta_path = dir.join("quarantine.json");
    let mut json = serde_json::to_string_pretty(metadata)
        .map_err(|e| StorageError::Quarantine(e.to_string()))?;
    json.push('\n');
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&meta_path)?;
    file.write_all(json.as_bytes())?;
    sync_file(&file)?;
    Ok(())
}

fn attribution_parent(
    app_data_root: &Path,
    device_id: Option<&DeviceId>,
) -> StorageResult<PathBuf> {
    let attribution_dir = match device_id {
        Some(id) => format!("device-{}", id.as_filename_stem()),
        None => "unattributed".to_string(),
    };
    Ok(quarantine_events_root(app_data_root).join(attribution_dir))
}

fn place_staging_as_object(staging: &Path, object_path: &Path) -> StorageResult<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
        let from: Vec<u16> = staging.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = object_path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::MoveFileExW(
                from.as_ptr(),
                to.as_ptr(),
                MOVEFILE_WRITE_THROUGH,
            )
        };
        if ok == 0 {
            let err = std::io::Error::last_os_error();
            let _ = fs::remove_file(staging);
            return Err(StorageError::Io(err));
        }
        Ok(())
    }
    #[cfg(unix)]
    {
        fs::rename(staging, object_path).map_err(|err| {
            let _ = fs::remove_file(staging);
            StorageError::Io(err)
        })
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (staging, object_path);
        Err(StorageError::Quarantine(
            "quarantine object placement unsupported on this platform".into(),
        ))
    }
}

fn exclusive_create_dir(base: &Path) -> StorageResult<PathBuf> {
    if let Some(parent) = base.parent() {
        fs::create_dir_all(parent)?;
        sync_directory(parent)?;
    }
    match fs::create_dir(base) {
        Ok(()) => Ok(base.to_path_buf()),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            let suffix = ENTRY_SUFFIX.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|t| t.as_nanos())
                .unwrap_or(0);
            let alt = PathBuf::from(format!("{}-{}-{}", base.display(), suffix, nanos));
            fs::create_dir(&alt)?;
            Ok(alt)
        }
        Err(err) => Err(StorageError::Io(err)),
    }
}

fn read_exact_some(file: &mut File, buf: &mut [u8]) -> StorageResult<()> {
    let mut filled = 0usize;
    while filled < buf.len() {
        match file.read(&mut buf[filled..])? {
            0 => {
                return Err(StorageError::Quarantine(
                    "unexpected EOF while streaming quarantine range".into(),
                ));
            }
            n => filled += n,
        }
    }
    Ok(())
}

fn utc_compact_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

fn format_detected_at(utc_compact: &str) -> String {
    // Compact seconds since epoch rendered as RFC 3339-ish diagnostic timestamp.
    // Operators use this for ordering; protocol acceptance never trusts wall-clock.
    match utc_compact.parse::<u64>() {
        Ok(secs) => crate::timefmt::format_unix_secs_rfc3339_utc(secs),
        Err(_) => format!("{utc_compact}Z"),
    }
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|t| t.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "cryptex-m2-q-{}-{}-{}",
            label,
            std::process::id(),
            nanos
        ))
    }

    #[test]
    fn quarantine_retains_exact_bytes_and_metadata() {
        let root = temp_root("basic");
        let device = DeviceId::new("device-a").unwrap();
        let payload = b"rejected-bytes-exact";
        let entry = quarantine_bytes(
            QuarantineRequest {
                app_data_root: &root,
                device_id: Some(&device),
                reason_code: "truncated_tail",
                source_kind: QuarantineSourceKind::ActiveOpen,
                source_relative: "canonical/events/x/00000000000000000000.open",
                byte_offset: Some(10),
                extras: QuarantineExtras::default(),
            },
            payload,
        )
        .unwrap();
        let object = fs::read(entry.directory.join("object.bin")).unwrap();
        assert_eq!(object, payload);
        let meta_text = fs::read_to_string(entry.directory.join("quarantine.json")).unwrap();
        assert!(meta_text.contains("truncated_tail"));
        assert!(!meta_text.contains(root.to_string_lossy().as_ref()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn quarantine_file_range_streams_without_caller_buffer() {
        let root = temp_root("range");
        let device = DeviceId::new("device-b").unwrap();
        let pack = root.join("pack.bin");
        fs::create_dir_all(&root).unwrap();
        let mut data = Vec::new();
        data.extend_from_slice(b"PREFIX");
        data.extend_from_slice(b"REJECTED-SUFFIX-BYTES");
        fs::write(&pack, &data).unwrap();
        let mut file = File::open(&pack).unwrap();
        let entry = quarantine_file_range(
            QuarantineRequest {
                app_data_root: &root,
                device_id: Some(&device),
                reason_code: "truncated_tail",
                source_kind: QuarantineSourceKind::ActiveOpen,
                source_relative: "canonical/events/x/0.open",
                byte_offset: Some(6),
                extras: QuarantineExtras::default(),
            },
            6,
            data.len() as u64,
            &mut file,
        )
        .unwrap();
        let object = fs::read(entry.directory.join("object.bin")).unwrap();
        assert_eq!(object, b"REJECTED-SUFFIX-BYTES");
        assert_eq!(entry.metadata.byte_offset, Some(6));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn quarantine_collision_uses_suffix_without_overwrite() {
        let root = temp_root("collide");
        let payload = b"same-bytes";
        let a = quarantine_bytes(
            QuarantineRequest {
                app_data_root: &root,
                device_id: None,
                reason_code: "corrupt_frame",
                source_kind: QuarantineSourceKind::Other,
                source_relative: "unresolved",
                byte_offset: None,
                extras: QuarantineExtras::default(),
            },
            payload,
        )
        .unwrap();
        let b = quarantine_bytes(
            QuarantineRequest {
                app_data_root: &root,
                device_id: None,
                reason_code: "corrupt_frame",
                source_kind: QuarantineSourceKind::Other,
                source_relative: "unresolved",
                byte_offset: None,
                extras: QuarantineExtras::default(),
            },
            payload,
        )
        .unwrap();
        assert_ne!(a.directory, b.directory);
        assert!(a.directory.join("object.bin").is_file());
        assert!(b.directory.join("object.bin").is_file());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn extras_schema_version_round_trips() {
        let root = temp_root("extras");
        let entry = quarantine_bytes(
            QuarantineRequest {
                app_data_root: &root,
                device_id: None,
                reason_code: "unsupported_schema",
                source_kind: QuarantineSourceKind::ActiveOpen,
                source_relative: "canonical/events/x/0.open",
                byte_offset: Some(0),
                extras: QuarantineExtras {
                    schema_version_if_parsed: Some(99),
                    content_hash_if_computed: None,
                },
            },
            b"bad-schema-bytes",
        )
        .unwrap();
        assert_eq!(entry.metadata.schema_version_if_parsed, Some(99));
        let meta_text = fs::read_to_string(entry.directory.join("quarantine.json")).unwrap();
        assert!(meta_text.contains("\"schema_version_if_parsed\": 99"));
        assert!(!meta_text.contains(root.to_string_lossy().as_ref()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn directory_sync_failure_is_typed_and_not_success() {
        let root = temp_root("syncfail");
        crate::durability::fail_next_directory_sync();
        let err = quarantine_bytes(
            QuarantineRequest {
                app_data_root: &root,
                device_id: None,
                reason_code: "truncated_tail",
                source_kind: QuarantineSourceKind::Other,
                source_relative: "unresolved",
                byte_offset: None,
                extras: QuarantineExtras::default(),
            },
            b"x",
        )
        .unwrap_err();
        assert!(matches!(err, StorageError::Durability(_)), "{err}");
        let _ = fs::remove_dir_all(&root);
    }
}
