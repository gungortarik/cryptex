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

/// Allow-listed quarantine reason codes used in directory names and metadata (ADR-0016 §8.1).
///
/// Values are fixed snake_case tokens safe as a single path component: ASCII
/// `[a-z0-9_]`, no separators, no host/path text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuarantineReasonCode {
    TruncatedTail,
    CorruptFrame,
    CorruptFrameHeader,
    CorruptSealedPack,
    UnsupportedSchema,
    DuplicateJsonKey,
    UnknownField,
    LimitExceeded,
    IntegrityFailure,
    InvalidRecord,
}

impl QuarantineReasonCode {
    /// Stable snake_case token for directory names and `quarantine.json`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TruncatedTail => "truncated_tail",
            Self::CorruptFrame => "corrupt_frame",
            Self::CorruptFrameHeader => "corrupt_frame_header",
            Self::CorruptSealedPack => "corrupt_sealed_pack",
            Self::UnsupportedSchema => "unsupported_schema",
            Self::DuplicateJsonKey => "duplicate_json_key",
            Self::UnknownField => "unknown_field",
            Self::LimitExceeded => "limit_exceeded",
            Self::IntegrityFailure => "integrity_failure",
            Self::InvalidRecord => "invalid_record",
        }
    }

    /// Parse only allow-listed tokens. Rejects separators and arbitrary path text.
    pub fn parse(token: &str) -> StorageResult<Self> {
        match token {
            "truncated_tail" => Ok(Self::TruncatedTail),
            "corrupt_frame" => Ok(Self::CorruptFrame),
            "corrupt_frame_header" => Ok(Self::CorruptFrameHeader),
            "corrupt_sealed_pack" => Ok(Self::CorruptSealedPack),
            "unsupported_schema" => Ok(Self::UnsupportedSchema),
            "duplicate_json_key" => Ok(Self::DuplicateJsonKey),
            "unknown_field" => Ok(Self::UnknownField),
            "limit_exceeded" => Ok(Self::LimitExceeded),
            "integrity_failure" => Ok(Self::IntegrityFailure),
            "invalid_record" => Ok(Self::InvalidRecord),
            _ => Err(StorageError::Quarantine(format!(
                "unsupported quarantine reason_code path token: {token}"
            ))),
        }
    }
}

impl From<cryptex_events::ValidationReasonCode> for QuarantineReasonCode {
    fn from(value: cryptex_events::ValidationReasonCode) -> Self {
        use cryptex_events::ValidationReasonCode as V;
        match value {
            V::UnsupportedSchema => Self::UnsupportedSchema,
            V::DuplicateJsonKey => Self::DuplicateJsonKey,
            V::UnknownField => Self::UnknownField,
            V::LimitExceeded => Self::LimitExceeded,
            V::IntegrityFailure => Self::IntegrityFailure,
            V::InvalidRecord => Self::InvalidRecord,
        }
    }
}

/// Common quarantine attribution/metadata inputs (keeps public helpers under Clippy arity limits).
#[derive(Debug, Clone)]
pub struct QuarantineRequest<'a> {
    pub app_data_root: &'a Path,
    pub device_id: Option<&'a DeviceId>,
    pub reason_code: QuarantineReasonCode,
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
    let base = q_parent.join(entry_directory_name(
        &utc_compact,
        request.reason_code,
        &entry_id,
    ));
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
        reason_code: request.reason_code.as_str().to_string(),
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
    let base = q_parent.join(entry_directory_name(
        &utc_compact,
        request.reason_code,
        &entry_id,
    ));
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
        reason_code: request.reason_code.as_str().to_string(),
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

fn entry_directory_name(
    utc_compact: &str,
    reason: QuarantineReasonCode,
    entry_id_hex: &str,
) -> String {
    // ADR-0016 §8.1: {utc_compact}_{reason_code}_{entry_id_hex}
    format!("{utc_compact}_{}_{entry_id_hex}", reason.as_str())
}

fn exclusive_create_dir(base: &Path) -> StorageResult<PathBuf> {
    let parent = base
        .parent()
        .ok_or_else(|| StorageError::Quarantine("quarantine entry path missing parent".into()))?;
    fs::create_dir_all(parent)?;
    sync_directory(parent)?;

    match fs::create_dir(base) {
        Ok(()) => return Ok(base.to_path_buf()),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(err) => return Err(StorageError::Io(err)),
    }

    let leaf = base
        .file_name()
        .ok_or_else(|| StorageError::Quarantine("quarantine entry path missing name".into()))?
        .to_string_lossy()
        .into_owned();
    for _ in 0..64 {
        let suffix = ENTRY_SUFFIX.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|t| t.as_nanos())
            .unwrap_or(0);
        let alt = parent.join(format!("{leaf}-{suffix}-{nanos}"));
        match fs::create_dir(&alt) {
            Ok(()) => return Ok(alt),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(StorageError::Io(err)),
        }
    }
    Err(StorageError::Quarantine(
        "exhausted exclusive quarantine directory create attempts".into(),
    ))
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

    static TEST_ROOT_SEQ: AtomicU64 = AtomicU64::new(1);

    /// Production-like app-data root for tests (ADR runtime placement).
    ///
    /// Uses a short unique suffix so ADR §8.1 directory names remain under classic
    /// Windows MAX_PATH for typical DeviceIds. Avoids deep TEMP/cargo paths that
    /// previously motivated the out-of-contract `{utc}_{entry_id}` shortening.
    fn temp_root(label: &str) -> PathBuf {
        let seq = TEST_ROOT_SEQ.fetch_add(1, Ordering::Relaxed);
        let base = production_like_app_data_root();
        base.join(format!("m2-t-q-{label}-{}-{seq}", std::process::id()))
    }

    fn production_like_app_data_root() -> PathBuf {
        #[cfg(windows)]
        {
            let local = std::env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA");
            PathBuf::from(local)
                .join("Cryptex-Dev")
                .join("p0-2")
                .join("m2-tests")
        }
        #[cfg(target_os = "macos")]
        {
            let home = std::env::var_os("HOME").expect("HOME");
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("Cryptex-Dev")
                .join("p0-2")
                .join("m2-tests")
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            std::env::temp_dir()
                .join("Cryptex-Dev")
                .join("p0-2")
                .join("m2-tests")
        }
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
                reason_code: QuarantineReasonCode::TruncatedTail,
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
                reason_code: QuarantineReasonCode::TruncatedTail,
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
                reason_code: QuarantineReasonCode::CorruptFrame,
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
                reason_code: QuarantineReasonCode::CorruptFrame,
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
                reason_code: QuarantineReasonCode::UnsupportedSchema,
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
                reason_code: QuarantineReasonCode::TruncatedTail,
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

    #[test]
    fn directory_name_uses_adr_reason_code_format() {
        let root = temp_root("adr-name");
        let entry = quarantine_bytes(
            QuarantineRequest {
                app_data_root: &root,
                device_id: None,
                reason_code: QuarantineReasonCode::CorruptFrameHeader,
                source_kind: QuarantineSourceKind::Other,
                source_relative: "unresolved",
                byte_offset: None,
                extras: QuarantineExtras::default(),
            },
            b"adr-format-bytes",
        )
        .unwrap();
        let name = entry
            .directory
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let expected = format!(
            "_{}_{}",
            QuarantineReasonCode::CorruptFrameHeader.as_str(),
            entry.metadata.entry_id
        );
        assert!(
            name.contains(&expected),
            "directory {name} missing ADR reason/entry segment {expected}"
        );
        assert!(
            name.chars().next().unwrap().is_ascii_digit(),
            "directory {name} should start with utc_compact digits"
        );
        assert!(!name.contains('/') && !name.contains('\\'));
        assert_eq!(
            entry.metadata.reason_code,
            QuarantineReasonCode::CorruptFrameHeader.as_str()
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn reason_code_parse_rejects_separators_and_unknown_tokens() {
        for token in [
            "truncated_tail/../x",
            "a\\b",
            "a/b",
            "..",
            "truncated_tail ",
            "TRUNCATED_TAIL",
            "not_a_reason",
            "",
        ] {
            let err = QuarantineReasonCode::parse(token).unwrap_err();
            assert!(
                matches!(err, StorageError::Quarantine(_)),
                "token {token:?} => {err}"
            );
        }
        assert_eq!(
            QuarantineReasonCode::parse("truncated_tail").unwrap(),
            QuarantineReasonCode::TruncatedTail
        );
    }

    #[cfg(windows)]
    #[test]
    fn adr_layout_succeeds_under_production_like_app_data_root() {
        let root = temp_root("pathlen");
        let device = DeviceId::new("device-safe-1").unwrap();
        let longest = QuarantineReasonCode::CorruptFrameHeader;
        let entry = quarantine_bytes(
            QuarantineRequest {
                app_data_root: &root,
                device_id: Some(&device),
                reason_code: longest,
                source_kind: QuarantineSourceKind::ActiveOpen,
                source_relative: "canonical/events/x/00000000000000000000.open",
                byte_offset: Some(0),
                extras: QuarantineExtras::default(),
            },
            b"path-length-regression-payload",
        )
        .unwrap();

        let object = entry.directory.join("object.bin");
        let root_len = root.to_string_lossy().chars().count();
        let dir_len = entry.directory.to_string_lossy().chars().count();
        let object_len = object.to_string_lossy().chars().count();
        let leaf = entry.directory.file_name().unwrap().to_string_lossy();
        let leaf_len = leaf.chars().count();

        assert!(
            object.is_file(),
            "object missing under production-like root {}",
            root.display()
        );
        assert!(
            leaf.contains(&format!("_{}_", longest.as_str())),
            "leaf {leaf} missing reason_code"
        );
        // Classic Win32 MAX_PATH is 260. Document measured lengths for the ADR layout.
        assert!(
            object_len < 260,
            "object path length {object_len} (root {root_len}, dir {dir_len}, leaf {leaf_len}) must stay under classic MAX_PATH=260 for production-like LocalAppData root {}",
            root.display()
        );
        eprintln!(
            "m2 quarantine path lengths: root={root_len} dir={dir_len} leaf={leaf_len} object={object_len} path={}",
            object.display()
        );

        // Measure worst-case ADR-0015 device id (64 chars -> 128 hex stem). Do not
        // silently shorten §8.1; if this exceeds classic MAX_PATH, record for ADR.
        let max_device = DeviceId::new("a".repeat(64)).unwrap();
        let attribution = format!("device-{}", max_device.as_filename_stem());
        let worst_leaf = entry_directory_name("1728312345", longest, &"b".repeat(64));
        let worst_object = root
            .join("quarantine")
            .join("canonical-events")
            .join(&attribution)
            .join(&worst_leaf)
            .join("object.bin");
        let worst_len = worst_object.to_string_lossy().chars().count();
        eprintln!(
            "m2 quarantine worst-case max DeviceId object path length={worst_len} (MAX_PATH=260) exceeds={} path={}",
            worst_len >= 260,
            worst_object.display()
        );
        // Realistic device ids must fit. Max-length DeviceId budget is reported, not
        // papered over by changing the accepted directory format.
        if worst_len >= 260 {
            eprintln!(
                "ADR-0016 §8.1 + max-length DeviceId exceeds classic Windows MAX_PATH under LocalAppData; separate ADR decision required (layout not altered)."
            );
        }

        let _ = fs::remove_dir_all(&root);
    }
}
