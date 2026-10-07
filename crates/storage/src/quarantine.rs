//! Quarantine for untrusted canonical-event bytes (ADR-0016 §8).
//!
//! Streaming entry creation hashes and copies rejected bytes without requiring
//! the full suffix to be held in memory by the caller.
//!
//! On Windows, quarantine filesystem I/O uses extended-length paths (`\\?\`)
//! per ADR-0016 Windows quarantine extended-path addendum. Logical §8.1 layout
//! names are unchanged.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
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
    validate_source_relative(request.source_relative)?;
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
struct StagingCleanup(Option<PathBuf>);

impl Drop for StagingCleanup {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = q_remove_file(&path);
        }
    }
}

impl StagingCleanup {
    fn disarm(mut self) {
        self.0 = None;
    }
}

pub fn quarantine_file_range(
    request: QuarantineRequest<'_>,
    start: u64,
    end: u64,
    source: &mut File,
) -> StorageResult<QuarantineEntry> {
    validate_source_relative(request.source_relative)?;
    if end <= start {
        return Err(StorageError::Quarantine(
            "refusing to quarantine empty suffix".into(),
        ));
    }
    let byte_length = end - start;
    let q_parent = attribution_parent(request.app_data_root, request.device_id)?;
    q_create_dir_all(&q_parent)?;
    q_sync_directory(&q_parent)?;

    let staging = q_parent.join(format!(
        ".staging-{}-{}-{}",
        std::process::id(),
        ENTRY_SUFFIX.fetch_add(1, Ordering::Relaxed),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|t| t.as_nanos())
            .unwrap_or(0)
    ));
    // Remove incomplete staging on any failure before successful placement.
    let staging_guard = StagingCleanup(Some(staging.clone()));

    let mut hasher = blake3::Hasher::new();
    {
        let mut out = q_open_create_new_write(&staging)?;
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
    q_sync_parent_directory(&dir.join("object.bin"))?;

    let object_path = dir.join("object.bin");
    place_staging_as_object(&staging, &object_path)?;
    staging_guard.disarm();
    // Re-open and sync destination object for platforms where rename does not imply durability.
    {
        let object = q_open_read_write(&object_path)?;
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
    q_sync_directory(&dir)?;
    q_sync_parent_directory(&dir.join("object.bin"))?;

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
    q_create_dir_all(&q_parent)?;
    q_sync_directory(&q_parent)?;

    let utc_compact = utc_compact_now();
    let base = q_parent.join(entry_directory_name(
        &utc_compact,
        request.reason_code,
        &entry_id,
    ));
    let dir = exclusive_create_dir(&base)?;
    q_sync_parent_directory(&dir.join("object.bin"))?;

    let object_path = dir.join("object.bin");
    {
        let mut file = q_open_create_new_write(&object_path)?;
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
    q_sync_directory(&dir)?;
    q_sync_parent_directory(&dir.join("object.bin"))?;

    Ok(QuarantineEntry {
        directory: dir,
        metadata,
    })
}

fn validate_source_relative(source_relative: &str) -> StorageResult<()> {
    // ADR-0016 §8.2: pack-relative / unresolved only; never absolute host paths.
    if source_relative.is_empty() {
        return Err(StorageError::Quarantine(
            "quarantine source_relative must not be empty".into(),
        ));
    }
    if source_relative.contains('\0') {
        return Err(StorageError::Quarantine(
            "quarantine source_relative must not contain NUL".into(),
        ));
    }
    if source_relative == "unresolved" {
        return Ok(());
    }
    let normalized = source_relative.replace('\\', "/");
    if normalized.starts_with('/')
        || normalized.starts_with("//")
        || (normalized.len() >= 2 && normalized.as_bytes()[1] == b':')
    {
        return Err(StorageError::Quarantine(
            "quarantine source_relative must not be an absolute host path".into(),
        ));
    }
    let parts: Vec<&str> = normalized.split('/').collect();
    if parts.len() != 4
        || parts[0] != "canonical"
        || parts[1] != "events"
        || parts[2].is_empty()
        || parts[3].is_empty()
    {
        return Err(StorageError::Quarantine(
            "quarantine source_relative must be 'unresolved' or 'canonical/events/<stem>/<pack>'"
                .into(),
        ));
    }
    for part in &parts {
        if *part == "." || *part == ".." || part.is_empty() {
            return Err(StorageError::Quarantine(
                "quarantine source_relative rejects '.' / '..' / empty segments".into(),
            ));
        }
        if part.contains('\\') {
            return Err(StorageError::Quarantine(
                "quarantine source_relative rejects mixed separators inside a segment".into(),
            ));
        }
    }
    let stem = parts[2];
    let pack = parts[3];
    if !stem
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(StorageError::Quarantine(
            "quarantine source_relative stem contains unsupported characters".into(),
        ));
    }
    let (index, ext) = pack.rsplit_once('.').ok_or_else(|| {
        StorageError::Quarantine(
            "quarantine source_relative pack name must end in .open or .seg".into(),
        )
    })?;
    if ext != "open" && ext != "seg" {
        return Err(StorageError::Quarantine(
            "quarantine source_relative pack extension must be .open or .seg".into(),
        ));
    }
    if index.is_empty() || !index.chars().all(|c| c.is_ascii_digit()) {
        return Err(StorageError::Quarantine(
            "quarantine source_relative pack index must be decimal digits".into(),
        ));
    }
    Ok(())
}

/// Reject path escape / separator smuggling before any quarantine FS I/O.
fn reject_unsafe_path_components(path: &Path) -> StorageResult<()> {
    for component in path.components() {
        match component {
            Component::ParentDir | Component::CurDir => {
                return Err(StorageError::Quarantine(
                    "refusing '.' or '..' path component in quarantine filesystem path".into(),
                ));
            }
            Component::Normal(name) => {
                let name = name.to_string_lossy();
                // Windows may surface "." / ".." as Normal rather than CurDir/ParentDir
                // (especially under verbatim / extended-length prefixes).
                if name == "." || name == ".." {
                    return Err(StorageError::Quarantine(
                        "refusing '.' or '..' path component in quarantine filesystem path".into(),
                    ));
                }
                if name.contains('/') || name.contains('\\') || name.contains('\0') {
                    return Err(StorageError::Quarantine(
                        "refusing separator or NUL inside quarantine path component".into(),
                    ));
                }
            }
            Component::Prefix(_) | Component::RootDir => {}
        }
    }
    // String-level guard: some Windows path forms keep "\." / "\.." in the
    // display text even when components() omits CurDir (e.g. `C:\\safe\.\x`).
    let normalized = path.to_string_lossy().replace('/', "\\");
    let padded = format!("\\{normalized}\\");
    if padded.contains("\\.\\") || padded.contains("\\..\\") {
        return Err(StorageError::Quarantine(
            "refusing '.' or '..' path segment in quarantine filesystem path".into(),
        ));
    }
    Ok(())
}

/// Logical quarantine path → OS path used for I/O (Windows: `\\?\` extended-length).
fn quarantine_fs_path(path: &Path) -> StorageResult<PathBuf> {
    reject_unsafe_path_components(path)?;
    #[cfg(windows)]
    {
        windows_extended_length_path(path)
    }
    #[cfg(not(windows))]
    {
        Ok(path.to_path_buf())
    }
}

#[cfg(windows)]
fn windows_extended_length_path(path: &Path) -> StorageResult<PathBuf> {
    let as_text = path.to_string_lossy();
    if as_text.starts_with(r"\\?\") {
        reject_unsafe_path_components(path)?;
        return Ok(path.to_path_buf());
    }

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::path::absolute(path).map_err(StorageError::Io)?
    };
    reject_unsafe_path_components(&absolute)?;

    let normalized = absolute.to_string_lossy().replace('/', "\\");
    if normalized.starts_with(r"\\?\") {
        return Ok(PathBuf::from(normalized));
    }
    if normalized.starts_with(r"\\") {
        // UNC \\server\share\... → \\?\UNC\server\share\...
        let rest = normalized.trim_start_matches('\\');
        Ok(PathBuf::from(format!(r"\\?\UNC\{rest}")))
    } else {
        Ok(PathBuf::from(format!(r"\\?\{normalized}")))
    }
}

fn q_create_dir_all(path: &Path) -> StorageResult<()> {
    fs::create_dir_all(quarantine_fs_path(path)?)?;
    Ok(())
}

fn q_create_dir(path: &Path) -> StorageResult<()> {
    fs::create_dir(quarantine_fs_path(path)?)?;
    Ok(())
}

fn q_open_create_new_write(path: &Path) -> StorageResult<File> {
    Ok(OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(quarantine_fs_path(path)?)?)
}

fn q_open_read_write(path: &Path) -> StorageResult<File> {
    Ok(OpenOptions::new()
        .read(true)
        .write(true)
        .open(quarantine_fs_path(path)?)?)
}

fn q_remove_file(path: &Path) -> StorageResult<()> {
    fs::remove_file(quarantine_fs_path(path)?)?;
    Ok(())
}

fn q_sync_directory(path: &Path) -> StorageResult<()> {
    sync_directory(&quarantine_fs_path(path)?)
}

fn q_sync_parent_directory(path: &Path) -> StorageResult<()> {
    sync_parent_directory(&quarantine_fs_path(path)?)
}

fn write_metadata_file(dir: &Path, metadata: &QuarantineMetadata) -> StorageResult<()> {
    let meta_path = dir.join("quarantine.json");
    let mut json = serde_json::to_string_pretty(metadata)
        .map_err(|e| StorageError::Quarantine(e.to_string()))?;
    json.push('\n');
    let mut file = q_open_create_new_write(&meta_path)?;
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
    let staging_fs = quarantine_fs_path(staging)?;
    let object_fs = quarantine_fs_path(object_path)?;
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
        let from: Vec<u16> = staging_fs
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        let to: Vec<u16> = object_fs.as_os_str().encode_wide().chain(Some(0)).collect();
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::MoveFileExW(
                from.as_ptr(),
                to.as_ptr(),
                MOVEFILE_WRITE_THROUGH,
            )
        };
        if ok == 0 {
            let err = std::io::Error::last_os_error();
            let _ = q_remove_file(staging);
            return Err(StorageError::Io(err));
        }
        Ok(())
    }
    #[cfg(unix)]
    {
        fs::rename(&staging_fs, &object_fs).map_err(|err| {
            let _ = q_remove_file(staging);
            StorageError::Io(err)
        })
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (staging_fs, object_fs);
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
    q_create_dir_all(parent)?;
    q_sync_directory(parent)?;

    match q_create_dir(base) {
        Ok(()) => return Ok(base.to_path_buf()),
        Err(StorageError::Io(err)) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(err) => return Err(err),
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
        match q_create_dir(&alt) {
            Ok(()) => return Ok(alt),
            Err(StorageError::Io(err)) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                continue;
            }
            Err(err) => return Err(err),
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

    fn walk_paths(root: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            let Ok(entries) = fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, out);
                }
                out.push(path);
            }
        }
        if root.exists() {
            walk(root, &mut out);
        }
        out
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

    #[test]
    fn source_relative_rejects_absolute_and_parent_paths() {
        let root = temp_root("src-rel");
        let device = DeviceId::new("device-src").unwrap();
        for bad in [
            r"C:\Users\secret\file.open",
            "/var/evil",
            "canonical/events/../escape/0.open",
            "canonical/events/x/0.txt",
            "",
        ] {
            let err = quarantine_bytes(
                QuarantineRequest {
                    app_data_root: &root,
                    device_id: Some(&device),
                    reason_code: QuarantineReasonCode::TruncatedTail,
                    source_kind: QuarantineSourceKind::Other,
                    source_relative: bad,
                    byte_offset: None,
                    extras: QuarantineExtras::default(),
                },
                b"x",
            )
            .unwrap_err();
            assert!(
                matches!(err, StorageError::Quarantine(_)),
                "expected Quarantine for {bad:?}, got {err}"
            );
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn staging_removed_when_source_range_read_fails() {
        let root = temp_root("staging-fail");
        let device = DeviceId::new("device-staging-fail").unwrap();
        fs::create_dir_all(&root).unwrap();
        let pack = root.join("pack.bin");
        fs::write(&pack, b"short").unwrap();
        let mut file = File::open(&pack).unwrap();
        // Request more bytes than the file contains so the staging copy fails mid-read.
        let err = quarantine_file_range(
            QuarantineRequest {
                app_data_root: &root,
                device_id: Some(&device),
                reason_code: QuarantineReasonCode::TruncatedTail,
                source_kind: QuarantineSourceKind::ActiveOpen,
                source_relative: "unresolved",
                byte_offset: Some(0),
                extras: QuarantineExtras::default(),
            },
            0,
            1024,
            &mut file,
        )
        .unwrap_err();
        assert!(
            matches!(err, StorageError::Io(_) | StorageError::Quarantine(_)),
            "expected read/quarantine failure, got {err}"
        );
        let qroot = quarantine_events_root(&root);
        if qroot.exists() {
            for path in walk_paths(&qroot) {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                assert!(
                    !name.starts_with(".staging-"),
                    "staging orphan must not remain after failed quarantine: {}",
                    path.display()
                );
            }
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn quarantine_fs_path_rejects_parent_and_curdir_components() {
        let with_parent = PathBuf::from(r"C:\\safe").join("..").join("escape");
        let err = quarantine_fs_path(&with_parent).unwrap_err();
        assert!(matches!(err, StorageError::Quarantine(_)), "{err}");
        let with_cur = PathBuf::from(r"C:\\safe").join(".").join("nested");
        let err = quarantine_fs_path(&with_cur).unwrap_err();
        assert!(matches!(err, StorageError::Quarantine(_)), "{err}");
    }

    #[cfg(windows)]
    #[test]
    fn quarantine_fs_path_prefixes_absolute_windows_paths() {
        let logical =
            PathBuf::from(r"C:\Users\example\AppData\Local\Cryptex-Dev\p0-2\m2-tests\example");
        let io_path = quarantine_fs_path(&logical).unwrap();
        let text = io_path.to_string_lossy();
        assert!(
            text.starts_with(r"\\?\"),
            "expected extended-length prefix, got {text}"
        );
        assert!(
            !text.starts_with("\\\\?\\\\?\\"),
            "must not double-prefix: {text}"
        );
        let again = quarantine_fs_path(&io_path).unwrap();
        assert_eq!(again, io_path);
    }

    #[cfg(windows)]
    #[test]
    fn adr_layout_windows_path_ops_under_production_like_app_data_root() {
        let longest = QuarantineReasonCode::CorruptFrameHeader;
        let max64 = "a".repeat(64);
        let cases: [(&str, String); 3] = [
            ("short", "device-safe-1".to_string()),
            ("uuid36", "550e8400-e29b-41d4-a716-446655440000".to_string()),
            ("max64", max64),
        ];

        // Observed for diagnostics only. Adapter uses \\?\ and must not rely on this.
        let long_paths_enabled = windows_long_paths_enabled_policy();
        eprintln!(
            "windows LongPathsEnabled policy (HKLM FileSystem) = {long_paths_enabled:?} (observed; not required by adapter)"
        );

        for (label, device_raw) in &cases {
            let root = temp_root(&format!("pathlen-{label}"));
            let device = DeviceId::new(device_raw.clone()).unwrap();
            let projected = projected_object_path_len(&root, &device, longest);
            let result = quarantine_bytes(
                QuarantineRequest {
                    app_data_root: &root,
                    device_id: Some(&device),
                    reason_code: longest,
                    source_kind: QuarantineSourceKind::ActiveOpen,
                    source_relative: "canonical/events/x/00000000000000000000.open",
                    byte_offset: Some(0),
                    extras: QuarantineExtras::default(),
                },
                format!("path-length-regression-{label}").as_bytes(),
            );

            match result {
                Ok(entry) => {
                    let object = entry.directory.join("object.bin");
                    let root_len = root.to_string_lossy().chars().count();
                    let dir_len = entry.directory.to_string_lossy().chars().count();
                    let object_len = object.to_string_lossy().chars().count();
                    let leaf = entry.directory.file_name().unwrap().to_string_lossy();

                    // Public API path remains logical (no \\?\ leak).
                    assert!(
                        !entry.directory.to_string_lossy().starts_with(r"\\?\"),
                        "{label}: public QuarantineEntry.directory must remain logical"
                    );
                    assert!(
                        leaf.contains(&format!("_{}_", longest.as_str())),
                        "{label}: leaf {leaf} missing ADR reason_code"
                    );

                    // All existence / read / cleanup on >MAX_PATH fixtures must use
                    // the extended-path adapter. Use it for every case so required
                    // assertions never depend on unprefixed Win32 MAX_PATH behavior.
                    let io_object = quarantine_fs_path(&object).expect("prefix object");
                    let io_root = quarantine_fs_path(&root).expect("prefix fixture root");
                    let io_object_text = io_object.to_string_lossy();
                    assert!(
                        io_object_text.starts_with(r"\\?\"),
                        "{label}: I/O path must use extended-length prefix: {io_object_text}"
                    );
                    assert!(
                        io_object.is_file(),
                        "{label}: object missing via extended-path adapter ({io_object_text})"
                    );

                    let expected = format!("path-length-regression-{label}");
                    let via_adapter = {
                        use std::io::Read;
                        let mut f = q_open_read_write(&object).expect("adapter read open");
                        let mut buf = Vec::new();
                        f.read_to_end(&mut buf).expect("adapter read");
                        buf
                    };
                    assert_eq!(
                        via_adapter,
                        expected.as_bytes(),
                        "{label}: adapter read mismatch"
                    );
                    // Prefixed fs::read also exercises extended-path open without
                    // going through the unprefixed logical object path.
                    let via_prefixed_fs =
                        fs::read(&io_object).expect("prefixed fs::read of object");
                    assert_eq!(
                        via_prefixed_fs,
                        expected.as_bytes(),
                        "{label}: prefixed fs::read mismatch"
                    );

                    eprintln!(
                        "m2 quarantine FS OK label={label} id_len={} projected={projected} root={root_len} dir={dir_len} leaf={} object={object_len} path={}",
                        device_raw.len(),
                        leaf.chars().count(),
                        object.display()
                    );
                    if *label == "short" {
                        assert!(
                            object_len < 260,
                            "short DeviceId object path {object_len} exceeded MAX_PATH under {}",
                            root.display()
                        );
                    } else {
                        assert!(
                            object_len > 260,
                            "{label}: expected projected/object path > MAX_PATH, got {object_len}"
                        );
                    }

                    let removed = fs::remove_dir_all(&io_root);
                    let root_gone = !io_root.exists();
                    eprintln!(
                        "m2 quarantine cleanup label={label} prefixed_remove_dir_all={removed:?} prefixed_root_exists={}",
                        io_root.exists()
                    );
                    assert!(
                        removed.is_ok(),
                        "{label}: prefixed cleanup failed: {removed:?}"
                    );
                    assert!(root_gone, "{label}: prefixed root remained after cleanup");
                }
                Err(err) => {
                    let io_root = quarantine_fs_path(&root).expect("prefix fixture root on err");
                    let _ = fs::remove_dir_all(&io_root);
                    panic!(
                        "{label}: quarantine must succeed with extended-length adapter \
                         (id_len={} projected_object_len={projected}): {err}",
                        device_raw.len()
                    );
                }
            }
        }
    }

    #[cfg(windows)]
    fn projected_object_path_len(
        root: &Path,
        device: &DeviceId,
        reason: QuarantineReasonCode,
    ) -> usize {
        let leaf = entry_directory_name("1728312345", reason, &"b".repeat(64));
        root.join("quarantine")
            .join("canonical-events")
            .join(format!("device-{}", device.as_filename_stem()))
            .join(leaf)
            .join("object.bin")
            .to_string_lossy()
            .chars()
            .count()
    }

    #[cfg(windows)]
    fn windows_long_paths_enabled_policy() -> Option<u32> {
        // Read-only query; do not modify registry.
        let output = std::process::Command::new("reg")
            .args([
                "query",
                r"HKLM\SYSTEM\CurrentControlSet\Control\FileSystem",
                "/v",
                "LongPathsEnabled",
            ])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        for token in text.split_whitespace() {
            if let Some(rest) = token.strip_prefix("0x")
                && let Ok(v) = u32::from_str_radix(rest, 16)
            {
                return Some(v);
            }
        }
        Some(0)
    }
}
