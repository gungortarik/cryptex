//! Quarantine for untrusted canonical-event bytes (ADR-0016 §8).

#[cfg(unix)]
use std::fs::File;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use cryptex_core::ids::DeviceId;
use serde::{Deserialize, Serialize};

use crate::layout::quarantine_events_root;
use crate::{StorageError, StorageResult};

static ENTRY_SUFFIX: AtomicU64 = AtomicU64::new(1);

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarantineEntry {
    pub directory: PathBuf,
    pub metadata: QuarantineMetadata,
}

/// Quarantine exact rejected bytes with exclusive no-replace directory create.
pub fn quarantine_bytes(
    app_data_root: &Path,
    device_id: Option<&DeviceId>,
    reason_code: &str,
    source_kind: QuarantineSourceKind,
    source_relative: &str,
    byte_offset: Option<u64>,
    bytes: &[u8],
) -> StorageResult<QuarantineEntry> {
    if bytes.is_empty() {
        return Err(StorageError::Quarantine(
            "refusing to quarantine empty suffix".into(),
        ));
    }
    let entry_id = lowercase_hex(blake3::hash(bytes).as_bytes());
    let attribution_dir = match device_id {
        Some(id) => format!("device-{}", id.as_filename_stem()),
        None => "unattributed".to_string(),
    };
    let utc_compact = utc_compact_now();
    let base = quarantine_events_root(app_data_root)
        .join(&attribution_dir)
        .join(format!("{utc_compact}_{reason_code}_{entry_id}"));

    let dir = exclusive_create_dir(&base)?;
    let object_path = dir.join("object.bin");
    let meta_path = dir.join("quarantine.json");

    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&object_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }

    let metadata = QuarantineMetadata {
        schema_version: 1,
        reason_code: reason_code.to_string(),
        detected_at: format!("{utc_compact}Z"),
        attribution: device_id.map(|d| d.as_str().to_string()),
        entry_id: entry_id.clone(),
        byte_length: bytes.len() as u64,
        source_kind,
        source_relative: source_relative.to_string(),
        byte_offset,
    };
    let mut json = serde_json::to_string_pretty(&metadata)
        .map_err(|e| StorageError::Quarantine(e.to_string()))?;
    json.push('\n');
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&meta_path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
    }
    #[cfg(unix)]
    {
        if let Some(parent) = dir.parent() {
            let _ = File::open(parent).and_then(|d| d.sync_all());
        }
    }

    Ok(QuarantineEntry {
        directory: dir,
        metadata,
    })
}

fn exclusive_create_dir(base: &Path) -> StorageResult<PathBuf> {
    if let Some(parent) = base.parent() {
        fs::create_dir_all(parent)?;
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
            fs::create_dir(&alt).map_err(StorageError::from)?;
            Ok(alt)
        }
        Err(err) => Err(StorageError::Io(err)),
    }
}

fn utc_compact_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
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
            &root,
            Some(&device),
            "truncated_tail",
            QuarantineSourceKind::ActiveOpen,
            "canonical/events/x/00000000000000000000.open",
            Some(10),
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
    fn quarantine_collision_uses_suffix_without_overwrite() {
        let root = temp_root("collide");
        let payload = b"same-bytes";
        let a = quarantine_bytes(
            &root,
            None,
            "corrupt_frame",
            QuarantineSourceKind::Other,
            "unresolved",
            None,
            payload,
        )
        .unwrap();
        // Force same base name by calling again quickly with same hash/reason —
        // exclusive_create_dir suffixes on AlreadyExists.
        let b = quarantine_bytes(
            &root,
            None,
            "corrupt_frame",
            QuarantineSourceKind::Other,
            "unresolved",
            None,
            payload,
        )
        .unwrap();
        assert_ne!(a.directory, b.directory);
        assert!(a.directory.join("object.bin").is_file());
        assert!(b.directory.join("object.bin").is_file());
        let _ = fs::remove_dir_all(&root);
    }
}
