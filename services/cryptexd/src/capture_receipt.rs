//! Derived capture receipts (M5 Slice 1).
//!
//! Receipts bind `capture_id` -> `event_id` for the bounded idempotency epoch.
//! They are not canonical and must not be required for note recovery.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use cryptex_core::ids::DeviceId;
use cryptex_storage::durability::{sync_directory, sync_file, sync_parent_directory};

use crate::paths::assert_outside_repository;

pub const CAPTURE_RECEIPT_SCHEMA_VERSION: u32 = 1;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptState {
    Pending,
    Committed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureReceipt {
    pub schema_version: u32,
    pub brain_id: String,
    pub device_id: String,
    pub capture_id: String,
    pub event_id: String,
    pub fingerprint: String,
    pub state: ReceiptState,
}

#[derive(Debug)]
pub enum ReceiptError {
    Path(String),
    Durability(String),
    Invalid(String),
    AlreadyExists,
    Io(String),
    /// Primary failure preserved together with staging cleanup failure.
    CleanupFailed {
        primary: String,
        cleanup: String,
    },
}

impl std::fmt::Display for ReceiptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Path(m) | Self::Durability(m) | Self::Invalid(m) | Self::Io(m) => {
                write!(f, "{m}")
            }
            Self::AlreadyExists => write!(f, "capture receipt already exists"),
            Self::CleanupFailed { primary, cleanup } => {
                write!(f, "{primary}; staging cleanup failed: {cleanup}")
            }
        }
    }
}

impl std::error::Error for ReceiptError {}

pub type ReceiptResult<T> = Result<T, ReceiptError>;

#[cfg(test)]
thread_local! {
    static FAIL_RECEIPT_PERSIST: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_PACK_CONFIRM: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_RECEIPT_CONFIRM: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    static FAIL_DIR_ANCESTRY: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    static FAIL_RECEIPT_PLACE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static LAST_PACK_CONFIRM: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

/// Test-only: next receipt persist fails once.
#[cfg(test)]
pub fn fail_next_receipt_persist() {
    FAIL_RECEIPT_PERSIST.with(|f| f.set(true));
}

/// Test-only: next pack durability confirm fails once.
#[cfg(test)]
pub fn fail_next_pack_confirm() {
    FAIL_PACK_CONFIRM.with(|f| f.set(true));
}

/// Test-only: next receipt-file durability confirm fails once.
#[cfg(test)]
pub fn fail_next_receipt_confirm() {
    fail_next_receipt_confirms(1);
}

#[cfg(test)]
pub fn fail_next_receipt_confirms(count: u32) {
    FAIL_RECEIPT_CONFIRM.with(|f| f.set(count));
}

/// Test-only: next `count` ancestry sync operations fail.
#[cfg(test)]
pub fn fail_next_dir_ancestry_syncs(count: u32) {
    FAIL_DIR_ANCESTRY.with(|f| f.set(count));
}

/// Test-only: fail once after staged receipt bytes are synced, before place/rename.
#[cfg(test)]
pub fn fail_next_receipt_place() {
    FAIL_RECEIPT_PLACE.with(|f| f.set(true));
}

#[cfg(test)]
pub fn take_last_confirmed_pack() -> Option<PathBuf> {
    LAST_PACK_CONFIRM.with(|c| c.borrow_mut().take())
}

pub fn capture_receipts_root(runtime_root: &Path) -> PathBuf {
    runtime_root.join("derived").join("capture_receipts")
}

pub fn receipt_path(
    runtime_root: &Path,
    brain_id: &str,
    device_id: &DeviceId,
    capture_id: &str,
) -> PathBuf {
    capture_receipts_root(runtime_root)
        .join(brain_id)
        .join(device_id.as_str())
        .join(format!("{capture_id}.json"))
}

fn inject_ancestry_fail() -> bool {
    #[cfg(test)]
    {
        FAIL_DIR_ANCESTRY.with(|f| {
            let remaining = f.get();
            if remaining > 0 {
                f.set(remaining - 1);
                true
            } else {
                false
            }
        })
    }
    #[cfg(not(test))]
    {
        false
    }
}

/// Ensure `dir` exists and that every directory entry in its ancestry has a
/// required parent-directory sync (Unix). Existing entries are not skipped:
/// a prior failed sync must not imply durability.
pub fn ensure_dir_tree_durable(dir: &Path) -> ReceiptResult<()> {
    use std::path::Component;

    if dir.as_os_str().is_empty() {
        return Err(ReceiptError::Path("empty directory path".into()));
    }
    let mut cur = PathBuf::new();
    for component in dir.components() {
        cur.push(component);
        // Drive prefixes and root dirs have no parent dirent to sync (Windows `C:`).
        if matches!(component, Component::Prefix(_) | Component::RootDir) {
            continue;
        }
        if !cur.exists() {
            fs::create_dir(&cur).map_err(|e| ReceiptError::Io(e.to_string()))?;
        }
        // Always sync, even when the directory already existed: a prior failed sync
        // must not be treated as durable merely because the path is visible.
        if inject_ancestry_fail() {
            return Err(ReceiptError::Durability(
                "injected directory ancestry sync failure".into(),
            ));
        }
        sync_parent_directory(&cur).map_err(|e| ReceiptError::Durability(e.to_string()))?;
    }
    Ok(())
}

/// Confirm durability of an existing directory entry created before the helper
/// ran (for example by device-lock acquisition).
pub fn confirm_existing_dir_entry_durable(dir: &Path) -> ReceiptResult<()> {
    if !dir.is_dir() {
        return Err(ReceiptError::Path(format!(
            "directory missing for durability confirm: {}",
            dir.display()
        )));
    }
    if inject_ancestry_fail() {
        return Err(ReceiptError::Durability(
            "injected directory ancestry sync failure".into(),
        ));
    }
    sync_parent_directory(dir).map_err(|e| ReceiptError::Durability(e.to_string()))?;
    Ok(())
}

pub fn load_receipt(
    runtime_root: &Path,
    brain_id: &str,
    device_id: &DeviceId,
    capture_id: &str,
) -> ReceiptResult<Option<CaptureReceipt>> {
    assert_outside_repository(runtime_root).map_err(|e| ReceiptError::Path(e.to_string()))?;
    let path = receipt_path(runtime_root, brain_id, device_id, capture_id);
    match fs::read_to_string(&path) {
        Ok(text) => {
            if text.contains('\r') || !text.ends_with('\n') {
                return Err(ReceiptError::Invalid(
                    "receipt must be UTF-8 LF with final newline".into(),
                ));
            }
            let receipt: CaptureReceipt = serde_json::from_str(text.trim_end())
                .map_err(|e| ReceiptError::Invalid(e.to_string()))?;
            if receipt.schema_version != CAPTURE_RECEIPT_SCHEMA_VERSION {
                return Err(ReceiptError::Invalid(format!(
                    "unsupported receipt schema_version {}",
                    receipt.schema_version
                )));
            }
            if receipt.brain_id != brain_id
                || receipt.device_id != device_id.as_str()
                || receipt.capture_id != capture_id
            {
                return Err(ReceiptError::Invalid(
                    "receipt identity fields do not match path scope".into(),
                ));
            }
            Ok(Some(receipt))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(ReceiptError::Io(e.to_string())),
    }
}

/// Exclusive-create a PENDING receipt via staged, synced, no-replace publish.
pub fn create_pending_receipt_exclusive(
    runtime_root: &Path,
    receipt: &CaptureReceipt,
) -> ReceiptResult<()> {
    if receipt.state != ReceiptState::Pending {
        return Err(ReceiptError::Invalid(
            "create_pending_receipt_exclusive requires Pending state".into(),
        ));
    }
    persist_receipt(runtime_root, receipt, PersistMode::ExclusiveNew)
}

pub fn write_committed_receipt(runtime_root: &Path, receipt: &CaptureReceipt) -> ReceiptResult<()> {
    if receipt.state != ReceiptState::Committed {
        return Err(ReceiptError::Invalid(
            "write_committed_receipt requires Committed state".into(),
        ));
    }
    persist_receipt(runtime_root, receipt, PersistMode::Replace)
}

enum PersistMode {
    ExclusiveNew,
    Replace,
}

fn persist_receipt(
    runtime_root: &Path,
    receipt: &CaptureReceipt,
    mode: PersistMode,
) -> ReceiptResult<()> {
    assert_outside_repository(runtime_root).map_err(|e| ReceiptError::Path(e.to_string()))?;
    let device = DeviceId::new(receipt.device_id.clone())
        .map_err(|e| ReceiptError::Invalid(e.to_string()))?;
    let path = receipt_path(
        runtime_root,
        &receipt.brain_id,
        &device,
        &receipt.capture_id,
    );
    let parent = path
        .parent()
        .ok_or_else(|| ReceiptError::Path("receipt path has no parent".into()))?;
    ensure_dir_tree_durable(parent)?;

    #[cfg(test)]
    {
        let fail = FAIL_RECEIPT_PERSIST.with(|f| {
            let armed = f.get();
            if armed {
                f.set(false);
            }
            armed
        });
        if fail {
            return Err(ReceiptError::Durability(
                "injected receipt persist failure".into(),
            ));
        }
    }

    let mut json =
        serde_json::to_string(receipt).map_err(|e| ReceiptError::Invalid(e.to_string()))?;
    json.push('\n');

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|t| t.as_nanos())
        .unwrap_or(0);
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_extension(format!(
        "json.tmp.{}-{sequence}-{nanos}",
        std::process::id()
    ));
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|e| ReceiptError::Io(e.to_string()))?;
        if let Err(primary) = file
            .write_all(json.as_bytes())
            .and_then(|_| file.sync_all())
        {
            return Err(cleanup_staging(&tmp, ReceiptError::Io(primary.to_string())));
        }
    }

    #[cfg(test)]
    {
        let fail = FAIL_RECEIPT_PLACE.with(|f| {
            let armed = f.get();
            if armed {
                f.set(false);
            }
            armed
        });
        if fail {
            return Err(cleanup_staging(
                &tmp,
                ReceiptError::Durability("injected receipt place failure after staged sync".into()),
            ));
        }
    }

    let place = match mode {
        PersistMode::ExclusiveNew => {
            if path.exists() {
                Err(ReceiptError::AlreadyExists)
            } else {
                place_new_file_no_replace(&tmp, &path)
            }
        }
        PersistMode::Replace => replace_atomic(&tmp, &path),
    };
    match place {
        Ok(()) => {
            sync_parent_directory(&path).map_err(|e| ReceiptError::Durability(e.to_string()))?;
            sync_directory(parent).map_err(|e| ReceiptError::Durability(e.to_string()))?;
            Ok(())
        }
        Err(primary) => Err(cleanup_staging(&tmp, primary)),
    }
}

fn cleanup_staging(tmp: &Path, primary: ReceiptError) -> ReceiptError {
    match fs::remove_file(tmp) {
        Ok(()) => primary,
        Err(error) if error.kind() == io::ErrorKind::NotFound => primary,
        Err(cleanup) => ReceiptError::CleanupFailed {
            primary: primary.to_string(),
            cleanup: cleanup.to_string(),
        },
    }
}

fn place_new_file_no_replace(tmp: &Path, dest: &Path) -> ReceiptResult<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
        let from: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = dest.as_os_str().encode_wide().chain(Some(0)).collect();
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::MoveFileExW(
                from.as_ptr(),
                to.as_ptr(),
                MOVEFILE_WRITE_THROUGH,
            )
        };
        if ok == 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::AlreadyExists || err.raw_os_error() == Some(183) {
                return Err(ReceiptError::AlreadyExists);
            }
            return Err(ReceiptError::Io(err.to_string()));
        }
        Ok(())
    }
    #[cfg(unix)]
    {
        fs::hard_link(tmp, dest).map_err(|err| {
            if err.kind() == io::ErrorKind::AlreadyExists {
                ReceiptError::AlreadyExists
            } else {
                ReceiptError::Io(err.to_string())
            }
        })?;
        fs::remove_file(tmp).map_err(|err| {
            ReceiptError::Io(format!(
                "failed to remove receipt staging file after durable placement: {err}"
            ))
        })?;
        Ok(())
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = (tmp, dest);
        Err(ReceiptError::Io(
            "exclusive receipt placement unsupported on this platform".into(),
        ))
    }
}

fn replace_atomic(tmp: &Path, path: &Path) -> ReceiptResult<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
        const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
        let from: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::MoveFileExW(
                from.as_ptr(),
                to.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        if ok == 0 {
            return Err(ReceiptError::Io(io::Error::last_os_error().to_string()));
        }
        Ok(())
    }
    #[cfg(unix)]
    {
        fs::rename(tmp, path).map_err(|e| ReceiptError::Io(e.to_string()))
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = (tmp, path);
        Err(ReceiptError::Io(
            "atomic receipt replace unsupported on this platform".into(),
        ))
    }
}

/// Open a pack file and sync file + parent (Unix parent sync required for Committed).
pub fn confirm_pack_durable(pack_path: &Path) -> ReceiptResult<()> {
    #[cfg(test)]
    {
        let fail = FAIL_PACK_CONFIRM.with(|f| {
            let armed = f.get();
            if armed {
                f.set(false);
            }
            armed
        });
        if fail {
            return Err(ReceiptError::Durability(
                "injected pack durability confirm failure".into(),
            ));
        }
    }
    if !pack_path.is_file() {
        return Err(ReceiptError::Path(format!(
            "pack missing for durability confirm: {}",
            pack_path.display()
        )));
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(pack_path)
        .map_err(|e| ReceiptError::Io(e.to_string()))?;
    sync_file(&file).map_err(|e| ReceiptError::Durability(e.to_string()))?;
    sync_parent_directory(pack_path).map_err(|e| ReceiptError::Durability(e.to_string()))?;
    #[cfg(test)]
    {
        LAST_PACK_CONFIRM.with(|c| {
            *c.borrow_mut() = Some(pack_path.to_path_buf());
        });
    }
    Ok(())
}

/// Confirm durability of an on-disk receipt file (required even when state is COMMITTED).
pub fn confirm_receipt_durable(path: &Path) -> ReceiptResult<()> {
    #[cfg(test)]
    {
        let fail = FAIL_RECEIPT_CONFIRM.with(|f| {
            let left = f.get();
            if left > 0 {
                f.set(left - 1);
                true
            } else {
                false
            }
        });
        if fail {
            return Err(ReceiptError::Durability(
                "injected receipt durability confirm failure".into(),
            ));
        }
    }
    if !path.is_file() {
        return Err(ReceiptError::Path(format!(
            "receipt missing for durability confirm: {}",
            path.display()
        )));
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| ReceiptError::Io(e.to_string()))?;
    sync_file(&file).map_err(|e| ReceiptError::Durability(e.to_string()))?;
    sync_parent_directory(path).map_err(|e| ReceiptError::Durability(e.to_string()))?;
    Ok(())
}
