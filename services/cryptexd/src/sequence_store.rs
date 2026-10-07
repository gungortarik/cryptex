//! Per-device sequence allocation — **cryptexd only** (ADR-0015 section 2.3 / section 4).
//!
//! Sequence numbers are allocated only by local cryptexd. This module is intentionally
//! part of the daemon crate so Desktop/CLI/MCP cannot open a durable counter writer
//! through cryptex-core.
//!
//! Rules enforced here:
//! - explicit `initialize` creates `last_issued = 0`; it does not issue a sequence
//! - `allocate` requires existing durable state and fails closed if missing
//! - first allocate after init returns `1` only after durable persist
//! - same-`device_id` initialize/allocate are mutually exclusive (incl. first-create race)
//! - durability failures return typed errors and never hand out a sequence
//! - wall-clock is never consulted

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use fs2::FileExt;
use serde::{Deserialize, Serialize};

use cryptex_core::{CoreError, CoreResult, DeviceId};

pub const COUNTER_SCHEMA_VERSION: u32 = 1;

static INIT_MARKER_TMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CounterState {
    schema_version: u32,
    device_id: String,
    last_issued: u64,
}

/// Durable per-device sequence store.
///
/// Process-wide mutex plus per-device exclusive file lock serialize same-`device_id`
/// initialize/allocate paths, including first-counter creation.
pub struct SequenceStore {
    root: PathBuf,
    gate: Mutex<()>,
}

impl SequenceStore {
    pub fn open(root: impl Into<PathBuf>) -> CoreResult<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self {
            root,
            gate: Mutex::new(()),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Explicit first-time initialization: persist `last_issued = 0`.
    ///
    /// Does not return a sequence number. Fails if durable state already exists.
    pub fn initialize(&self, device_id: &DeviceId) -> CoreResult<()> {
        self.with_device_lock(device_id, |state| {
            if state.is_some() {
                return Err(CoreError::SequenceStateAlreadyExists {
                    device_id: device_id.as_str().to_string(),
                });
            }
            // Record that this device's sequence state has existed before
            // creating its counter. The marker is committed first so a crash
            // between these two writes fails closed instead of allowing a
            // later initialize to restart at zero.
            self.create_initialization_marker(device_id)?;
            let initial = CounterState {
                schema_version: COUNTER_SCHEMA_VERSION,
                device_id: device_id.as_str().to_string(),
                last_issued: 0,
            };
            self.persist_synced(device_id, &initial)
        })
    }

    /// Allocate the next sequence for an **existing** device counter.
    ///
    /// Missing durable state fails closed with `SequenceStateMissing` (does not
    /// auto-create or restart at `1`).
    pub fn allocate(&self, device_id: &DeviceId) -> CoreResult<u64> {
        self.with_device_lock(device_id, |state| self.allocate_locked(device_id, state))
    }

    /// Read `last_issued` without allocating. Missing → `SequenceStateMissing`.
    pub fn last_issued(&self, device_id: &DeviceId) -> CoreResult<u64> {
        self.with_device_lock(device_id, |state| match state {
            None => Err(CoreError::SequenceStateMissing {
                device_id: device_id.as_str().to_string(),
            }),
            Some(current) => Ok(current.last_issued),
        })
    }

    fn allocate_locked(
        &self,
        device_id: &DeviceId,
        state: Option<CounterState>,
    ) -> CoreResult<u64> {
        let mut current = state.ok_or_else(|| CoreError::SequenceStateMissing {
            device_id: device_id.as_str().to_string(),
        })?;

        if current.device_id != device_id.as_str() {
            return Err(CoreError::SequenceStateCorrupt {
                device_id: device_id.as_str().to_string(),
                detail: "device_id mismatch in durable counter".to_string(),
            });
        }

        let next =
            current
                .last_issued
                .checked_add(1)
                .ok_or_else(|| CoreError::SequenceOverflow {
                    device_id: device_id.as_str().to_string(),
                })?;

        if next <= current.last_issued {
            return Err(CoreError::SequenceNonMonotonic {
                device_id: device_id.as_str().to_string(),
            });
        }

        current.last_issued = next;
        self.persist_synced(device_id, &current)?;
        Ok(next)
    }

    fn with_device_lock<T>(
        &self,
        device_id: &DeviceId,
        body: impl FnOnce(Option<CounterState>) -> CoreResult<T>,
    ) -> CoreResult<T> {
        let _process_guard = self
            .gate
            .lock()
            .map_err(|_| CoreError::Durability("sequence process gate poisoned".to_string()))?;

        let lock_path = self.lock_path(device_id);
        let counter_path = self.counter_path(device_id);

        let lock_file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)?;
        lock_file
            .lock_exclusive()
            .map_err(|e| CoreError::Durability(format!("lock sequence counter: {e}")))?;

        let result = (|| {
            let state = self.read_state(device_id, &counter_path)?;
            match (state.as_ref(), self.read_initialization_marker(device_id)?) {
                (Some(_), None) => {
                    // Upgrade an existing counter from before marker support.
                    // Persist the marker before any subsequent operation can
                    // mutate or issue from this state.
                    self.create_initialization_marker(device_id)?;
                }
                (None, Some(())) => {
                    return Err(CoreError::SequenceStateMissing {
                        device_id: device_id.as_str().to_string(),
                    });
                }
                _ => {}
            }
            body(state)
        })();

        let _ = lock_file.unlock();
        result
    }

    fn read_state(
        &self,
        device_id: &DeviceId,
        counter_path: &Path,
    ) -> CoreResult<Option<CounterState>> {
        if !counter_path.exists() {
            return Ok(None);
        }

        let mut file = File::open(counter_path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        if bytes.is_empty() {
            return Err(CoreError::SequenceStateCorrupt {
                device_id: device_id.as_str().to_string(),
                detail: "empty counter file".to_string(),
            });
        }

        let parsed: CounterState =
            serde_json::from_slice(&bytes).map_err(|e| CoreError::SequenceStateCorrupt {
                device_id: device_id.as_str().to_string(),
                detail: e.to_string(),
            })?;

        if parsed.schema_version != COUNTER_SCHEMA_VERSION {
            return Err(CoreError::SequenceStateCorrupt {
                device_id: device_id.as_str().to_string(),
                detail: format!(
                    "unsupported counter schema_version {} (expected {})",
                    parsed.schema_version, COUNTER_SCHEMA_VERSION
                ),
            });
        }

        if parsed.device_id != device_id.as_str() {
            return Err(CoreError::SequenceStateCorrupt {
                device_id: device_id.as_str().to_string(),
                detail: "device_id mismatch in durable counter".to_string(),
            });
        }
        Ok(Some(parsed))
    }

    fn initialization_marker_path(&self, device_id: &DeviceId) -> PathBuf {
        self.root
            .join(format!("{}.initialized", device_id.as_filename_stem()))
    }

    fn read_initialization_marker(&self, device_id: &DeviceId) -> CoreResult<Option<()>> {
        let path = self.initialization_marker_path(device_id);
        let mut file = match File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let marker: CounterState =
            serde_json::from_slice(&bytes).map_err(|error| CoreError::SequenceStateCorrupt {
                device_id: device_id.as_str().to_string(),
                detail: format!("invalid initialization marker: {error}"),
            })?;
        if marker.schema_version != COUNTER_SCHEMA_VERSION
            || marker.device_id != device_id.as_str()
            || marker.last_issued != 0
        {
            return Err(CoreError::SequenceStateCorrupt {
                device_id: device_id.as_str().to_string(),
                detail: "initialization marker does not match device/schema".to_string(),
            });
        }
        Ok(Some(()))
    }

    fn create_initialization_marker(&self, device_id: &DeviceId) -> CoreResult<()> {
        let path = self.initialization_marker_path(device_id);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|time| time.as_nanos())
            .unwrap_or(0);
        let sequence = INIT_MARKER_TMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let tmp = path.with_extension(format!(
            "initialized.tmp.{}-{sequence}-{nanos}",
            std::process::id()
        ));
        let marker = CounterState {
            schema_version: COUNTER_SCHEMA_VERSION,
            device_id: device_id.as_str().to_string(),
            last_issued: 0,
        };
        let bytes = serde_json::to_vec(&marker)
            .map_err(|error| CoreError::Durability(format!("serialize init marker: {error}")))?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|error| {
                CoreError::Durability(format!(
                    "create sequence initialization marker temp: {error}"
                ))
            })?;
        let staged = file.write_all(&bytes).and_then(|()| file.sync_all());
        drop(file);
        if let Err(error) = staged {
            return Err(cleanup_marker_temp(&tmp, error));
        }
        if let Err(error) = place_initialization_marker(&tmp, &path) {
            return Err(cleanup_marker_temp(&tmp, error));
        }
        Ok(())
    }

    fn persist_synced(&self, device_id: &DeviceId, state: &CounterState) -> CoreResult<()> {
        let path = self.counter_path(device_id);
        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_vec_pretty(state)
            .map_err(|e| CoreError::Durability(format!("serialize counter: {e}")))?;

        {
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&tmp)?;
            file.write_all(&json)?;
            file.sync_all()
                .map_err(|e| CoreError::Durability(format!("sync counter temp: {e}")))?;
        }

        replace_file_atomic(&tmp, &path)?;
        confirm_durability(&path)?;

        // Re-read to confirm durable committed value before caller observes success.
        let confirmed = self.read_state(device_id, &path)?.ok_or_else(|| {
            CoreError::Durability("counter missing after durable write".to_string())
        })?;
        if confirmed.last_issued != state.last_issued {
            return Err(CoreError::Durability(format!(
                "counter confirm mismatch: wrote {} read {}",
                state.last_issued, confirmed.last_issued
            )));
        }

        Ok(())
    }

    fn counter_path(&self, device_id: &DeviceId) -> PathBuf {
        self.root
            .join(format!("{}.json", device_id.as_filename_stem()))
    }

    fn lock_path(&self, device_id: &DeviceId) -> PathBuf {
        self.root
            .join(format!("{}.lock", device_id.as_filename_stem()))
    }
}

fn cleanup_marker_temp(tmp: &Path, primary: std::io::Error) -> CoreError {
    match fs::remove_file(tmp) {
        Ok(()) => CoreError::Durability(primary.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            CoreError::Durability(primary.to_string())
        }
        Err(cleanup) => CoreError::Durability(format!(
            "{primary}; initialization marker temp cleanup failed: {cleanup}"
        )),
    }
}

fn place_initialization_marker(tmp: &Path, path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        fs::hard_link(tmp, path)?;
        fs::remove_file(tmp)?;
        let parent = path.parent().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "sequence initialization marker has no parent",
            )
        })?;
        File::open(parent)?.sync_all()
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW};

        let from: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: both pointers refer to valid null-terminated UTF-16 paths.
        let result = unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_WRITE_THROUGH) };
        if result == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = (tmp, path);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "durable sequence initialization markers are unsupported on this platform",
        ))
    }
}

/// Crash-safe replace: never delete-then-rename.
///
/// - Unix: `rename` replaces atomically on the same filesystem.
/// - Windows: `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`.
fn replace_file_atomic(tmp: &Path, dest: &Path) -> CoreResult<()> {
    #[cfg(unix)]
    {
        fs::rename(tmp, dest)
            .map_err(|e| CoreError::Durability(format!("atomic rename counter into place: {e}")))?;
        Ok(())
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;

        const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
        const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

        let from: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = dest.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: pointers are null-terminated wide strings from OsStr encoding.
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::MoveFileExW(
                from.as_ptr(),
                to.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        if ok == 0 {
            let err = std::io::Error::last_os_error();
            return Err(CoreError::Durability(format!(
                "atomic MoveFileEx replace counter: {err}"
            )));
        }
        Ok(())
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = (tmp, dest);
        Err(CoreError::Durability(
            "atomic counter replace is unsupported on this platform".to_string(),
        ))
    }
}

fn confirm_durability(path: &Path) -> CoreResult<()> {
    // On Windows, FlushFileBuffers requires a writable handle.
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| {
            CoreError::Durability(format!("reopen counter for durability confirm: {e}"))
        })?;
    file.sync_all()
        .map_err(|e| CoreError::Durability(format!("sync replaced counter: {e}")))?;

    #[cfg(unix)]
    {
        let parent = path.parent().ok_or_else(|| {
            CoreError::Durability("counter path has no parent directory".to_string())
        })?;
        let dir = File::open(parent)
            .map_err(|e| CoreError::Durability(format!("open parent directory for sync: {e}")))?;
        dir.sync_all()
            .map_err(|e| CoreError::Durability(format!("sync parent directory: {e}")))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn mismatched_device_counter_is_rejected_by_every_operation() {
        let store = temp_store("device-mismatch");
        let device = DeviceId::new("requested-device").unwrap();
        let path = store.counter_path(&device);
        let bytes = br#"{"schema_version":1,"device_id":"other-device","last_issued":42}"#;
        fs::write(&path, bytes).unwrap();
        assert!(matches!(
            store.last_issued(&device),
            Err(CoreError::SequenceStateCorrupt { .. })
        ));
        assert!(matches!(
            store.allocate(&device),
            Err(CoreError::SequenceStateCorrupt { .. })
        ));
        assert!(matches!(
            store.initialize(&device),
            Err(CoreError::SequenceStateCorrupt { .. })
        ));
        assert_eq!(
            fs::read(&path).unwrap(),
            bytes,
            "corrupt state must remain unchanged"
        );
        fs::remove_dir_all(store.root()).unwrap();
    }

    fn temp_store(label: &str) -> SequenceStore {
        let root =
            std::env::temp_dir().join(format!("cryptex-m1-seq-{}-{}", label, std::process::id()));
        let _ = fs::remove_dir_all(&root);
        SequenceStore::open(root).expect("open store")
    }

    #[test]
    fn initialize_then_first_allocate_returns_one() {
        let store = temp_store("first");
        let device = DeviceId::new("device-a").unwrap();
        store.initialize(&device).unwrap();
        assert_eq!(store.last_issued(&device).unwrap(), 0);
        assert_eq!(store.allocate(&device).unwrap(), 1);
        assert_eq!(store.last_issued(&device).unwrap(), 1);
        assert_eq!(store.allocate(&device).unwrap(), 2);
    }

    #[test]
    fn allocate_without_initialize_fails_closed() {
        let store = temp_store("missing");
        let device = DeviceId::new("device-c").unwrap();
        let err = store.allocate(&device).unwrap_err();
        assert!(matches!(err, CoreError::SequenceStateMissing { .. }));
    }

    #[test]
    fn deleted_counter_cannot_be_reinitialized_from_zero() {
        let store = temp_store("deleted-counter");
        let device = DeviceId::new("device-counter-deleted").unwrap();
        store.initialize(&device).unwrap();
        assert_eq!(store.allocate(&device).unwrap(), 1);
        fs::remove_file(store.counter_path(&device)).unwrap();

        assert!(matches!(
            store.allocate(&device),
            Err(CoreError::SequenceStateMissing { .. })
        ));
        assert!(matches!(
            store.initialize(&device),
            Err(CoreError::SequenceStateMissing { .. })
        ));
        assert!(!store.counter_path(&device).exists());
        assert!(store.initialization_marker_path(&device).exists());
        fs::remove_dir_all(store.root()).unwrap();
    }

    #[test]
    fn existing_counter_without_marker_is_protected_before_use() {
        let store = temp_store("legacy-counter");
        let device = DeviceId::new("device-counter-legacy").unwrap();
        let legacy_state = CounterState {
            schema_version: COUNTER_SCHEMA_VERSION,
            device_id: device.as_str().to_string(),
            last_issued: 7,
        };
        fs::write(
            store.counter_path(&device),
            serde_json::to_vec(&legacy_state).unwrap(),
        )
        .unwrap();

        assert_eq!(store.last_issued(&device).unwrap(), 7);
        assert!(store.initialization_marker_path(&device).exists());
        fs::remove_file(store.counter_path(&device)).unwrap();
        assert!(matches!(
            store.initialize(&device),
            Err(CoreError::SequenceStateMissing { .. })
        ));
        assert!(!store.counter_path(&device).exists());
        fs::remove_dir_all(store.root()).unwrap();
    }

    #[test]
    fn reinitialize_existing_fails_closed() {
        let store = temp_store("exists");
        let device = DeviceId::new("device-x").unwrap();
        store.initialize(&device).unwrap();
        let err = store.initialize(&device).unwrap_err();
        assert!(matches!(err, CoreError::SequenceStateAlreadyExists { .. }));
    }

    #[test]
    fn durable_before_return_survives_reopen() {
        let root =
            std::env::temp_dir().join(format!("cryptex-m1-seq-reopen-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let device = DeviceId::new("device-b").unwrap();

        {
            let store = SequenceStore::open(&root).unwrap();
            store.initialize(&device).unwrap();
            assert_eq!(store.allocate(&device).unwrap(), 1);
            assert_eq!(store.allocate(&device).unwrap(), 2);
        }

        let store = SequenceStore::open(&root).unwrap();
        assert_eq!(store.last_issued(&device).unwrap(), 2);
        assert_eq!(store.allocate(&device).unwrap(), 3);
    }

    #[test]
    fn corrupt_state_fails_closed() {
        let store = temp_store("corrupt");
        let device = DeviceId::new("device-d").unwrap();
        store.initialize(&device).unwrap();
        let path = store
            .root()
            .join(format!("{}.json", device.as_filename_stem()));
        fs::write(&path, b"{not-json").unwrap();
        let err = store.allocate(&device).unwrap_err();
        assert!(matches!(err, CoreError::SequenceStateCorrupt { .. }));
    }

    #[test]
    fn unsupported_schema_version_fails_closed() {
        let store = temp_store("schema");
        let device = DeviceId::new("device-s").unwrap();
        store.initialize(&device).unwrap();
        let path = store
            .root()
            .join(format!("{}.json", device.as_filename_stem()));
        fs::write(
            &path,
            br#"{
  "schema_version": 99,
  "device_id": "device-s",
  "last_issued": 3
}"#,
        )
        .unwrap();
        let err = store.allocate(&device).unwrap_err();
        match err {
            CoreError::SequenceStateCorrupt { detail, .. } => {
                assert!(detail.contains("unsupported counter schema_version"));
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn concurrent_allocates_are_unique_and_increasing() {
        let root = std::env::temp_dir().join(format!("cryptex-m1-seq-conc-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let store = Arc::new(SequenceStore::open(&root).unwrap());
        let device = DeviceId::new("device-e").unwrap();
        store.initialize(&device).unwrap();

        let mut handles = Vec::new();
        for _ in 0..32 {
            let store = Arc::clone(&store);
            let device = device.clone();
            handles.push(thread::spawn(move || store.allocate(&device)));
        }

        let mut values = BTreeSet::new();
        for handle in handles {
            let value = handle.join().expect("thread").expect("allocate");
            assert!(values.insert(value), "duplicate sequence {value}");
        }

        assert_eq!(values.len(), 32);
        assert_eq!(*values.first().unwrap(), 1);
        assert_eq!(*values.last().unwrap(), 32);
        assert_eq!(store.last_issued(&device).unwrap(), 32);
    }

    #[test]
    fn concurrent_initialize_has_single_winner() {
        let root =
            std::env::temp_dir().join(format!("cryptex-m1-seq-initrace-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let store = Arc::new(SequenceStore::open(&root).unwrap());
        let device = DeviceId::new("device-f").unwrap();

        let mut handles = Vec::new();
        for _ in 0..16 {
            let store = Arc::clone(&store);
            let device = device.clone();
            handles.push(thread::spawn(move || store.initialize(&device)));
        }

        let mut wins = 0usize;
        let mut already = 0usize;
        for handle in handles {
            match handle.join().expect("thread") {
                Ok(()) => wins += 1,
                Err(CoreError::SequenceStateAlreadyExists { .. }) => already += 1,
                Err(other) => panic!("unexpected initialize error: {other}"),
            }
        }
        assert_eq!(wins, 1);
        assert_eq!(already, 15);
        assert_eq!(store.last_issued(&device).unwrap(), 0);
        assert_eq!(store.allocate(&device).unwrap(), 1);
    }

    #[test]
    fn concurrent_allocates_across_separate_store_instances() {
        let root = std::env::temp_dir().join(format!(
            "cryptex-m1-seq-multistore-alloc-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        {
            let bootstrap = SequenceStore::open(&root).unwrap();
            let device = DeviceId::new("device-ms-a").unwrap();
            bootstrap.initialize(&device).unwrap();
        }

        let device = DeviceId::new("device-ms-a").unwrap();
        let mut handles = Vec::new();
        for _ in 0..24 {
            let root = root.clone();
            let device = device.clone();
            handles.push(thread::spawn(move || {
                // Distinct SequenceStore (distinct in-memory mutex) — file lock must coordinate.
                let store = SequenceStore::open(&root).expect("open");
                store.allocate(&device)
            }));
        }

        let mut values = BTreeSet::new();
        for handle in handles {
            let value = handle.join().expect("thread").expect("allocate");
            assert!(values.insert(value), "duplicate sequence {value}");
        }
        assert_eq!(values.len(), 24);
        assert_eq!(*values.first().unwrap(), 1);
        assert_eq!(*values.last().unwrap(), 24);

        let verify = SequenceStore::open(&root).unwrap();
        assert_eq!(verify.last_issued(&device).unwrap(), 24);
    }

    #[test]
    fn concurrent_initialize_across_separate_store_instances() {
        let root = std::env::temp_dir().join(format!(
            "cryptex-m1-seq-multistore-init-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let device = DeviceId::new("device-ms-b").unwrap();

        let mut handles = Vec::new();
        for _ in 0..16 {
            let root = root.clone();
            let device = device.clone();
            handles.push(thread::spawn(move || {
                let store = SequenceStore::open(&root).expect("open");
                store.initialize(&device)
            }));
        }

        let mut wins = 0usize;
        let mut already = 0usize;
        for handle in handles {
            match handle.join().expect("thread") {
                Ok(()) => wins += 1,
                Err(CoreError::SequenceStateAlreadyExists { .. }) => already += 1,
                Err(other) => panic!("unexpected initialize error: {other}"),
            }
        }
        assert_eq!(wins, 1);
        assert_eq!(already, 15);
        let verify = SequenceStore::open(&root).unwrap();
        assert_eq!(verify.last_issued(&device).unwrap(), 0);
    }

    #[test]
    fn case_distinct_device_ids_use_separate_counter_files() {
        let store = temp_store("casefold");
        let upper = DeviceId::new("Device-A").unwrap();
        let lower = DeviceId::new("device-a").unwrap();
        store.initialize(&upper).unwrap();
        store.initialize(&lower).unwrap();
        assert_eq!(store.allocate(&upper).unwrap(), 1);
        assert_eq!(store.allocate(&lower).unwrap(), 1);
        assert_eq!(store.allocate(&upper).unwrap(), 2);
        assert_eq!(store.last_issued(&upper).unwrap(), 2);
        assert_eq!(store.last_issued(&lower).unwrap(), 1);
        assert_ne!(
            store
                .root()
                .join(format!("{}.json", upper.as_filename_stem())),
            store
                .root()
                .join(format!("{}.json", lower.as_filename_stem()))
        );
    }

    #[test]
    fn cross_process_file_lock_initialize_and_allocate() {
        use std::process::Command;

        const WORKER_ENV: &str = "CRYPTEX_CORE_SEQ_XPROC_WORKER";
        const ROOT_ENV: &str = "CRYPTEX_CORE_SEQ_XPROC_ROOT";

        if let Ok(role) = std::env::var(WORKER_ENV) {
            let root = PathBuf::from(std::env::var(ROOT_ENV).expect("root env"));
            let device = DeviceId::new("device-xproc").expect("device");
            let store = SequenceStore::open(&root).expect("open");
            match role.as_str() {
                "init" => match store.initialize(&device) {
                    Ok(()) => std::process::exit(10),
                    Err(CoreError::SequenceStateAlreadyExists { .. }) => std::process::exit(11),
                    Err(_) => std::process::exit(12),
                },
                "alloc" => match store.allocate(&device) {
                    Ok(n) => {
                        // Avoid stdout (test harness noise + exit without flush).
                        let out = root.join(format!("alloc-result-{}.txt", std::process::id()));
                        fs::write(&out, n.to_string()).expect("write alloc result");
                        std::process::exit(0);
                    }
                    Err(_) => std::process::exit(13),
                },
                _ => std::process::exit(14),
            }
        }

        let root =
            std::env::temp_dir().join(format!("cryptex-m1-seq-xproc-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let exe = std::env::current_exe().expect("current_exe");
        let test_args = [
            "--exact",
            "sequence_store::tests::cross_process_file_lock_initialize_and_allocate",
            "--nocapture",
        ];

        let mut init_children = Vec::new();
        for _ in 0..8 {
            init_children.push(
                Command::new(&exe)
                    .env(WORKER_ENV, "init")
                    .env(ROOT_ENV, &root)
                    .env("RUST_TEST_THREADS", "1")
                    .args(test_args)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .expect("spawn init worker"),
            );
        }

        let mut wins = 0usize;
        let mut already = 0usize;
        for mut child in init_children {
            let status = child.wait().expect("wait init");
            match status.code() {
                Some(10) => wins += 1,
                Some(11) => already += 1,
                other => panic!("unexpected init worker exit: {other:?}"),
            }
        }
        assert_eq!(wins, 1, "exactly one cross-process initialize must win");
        assert_eq!(already, 7);

        let mut alloc_children = Vec::new();
        for _ in 0..12 {
            alloc_children.push(
                Command::new(&exe)
                    .env(WORKER_ENV, "alloc")
                    .env(ROOT_ENV, &root)
                    .env("RUST_TEST_THREADS", "1")
                    .args(test_args)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .expect("spawn alloc worker"),
            );
        }

        for mut child in alloc_children {
            let status = child.wait().expect("wait alloc");
            assert!(status.success(), "alloc worker failed: {status:?}");
        }

        let mut values = BTreeSet::new();
        for entry in fs::read_dir(&root).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("alloc-result-") {
                continue;
            }
            let text = fs::read_to_string(entry.path()).unwrap();
            let value: u64 = text
                .trim()
                .parse()
                .unwrap_or_else(|_| panic!("bad alloc result file {name}: {text:?}"));
            assert!(
                values.insert(value),
                "duplicate cross-process sequence {value}"
            );
        }
        assert_eq!(values.len(), 12);
        assert_eq!(*values.first().unwrap(), 1);
        assert_eq!(*values.last().unwrap(), 12);

        let verify = SequenceStore::open(&root).unwrap();
        let device = DeviceId::new("device-xproc").unwrap();
        assert_eq!(verify.last_issued(&device).unwrap(), 12);
    }
    /// Plan §6.1: after a successful allocate return, forcibly stop the process,
    /// reopen, and ensure the next allocate is strictly greater with matching durable state.
    #[test]
    fn crash_restart_after_successful_allocate_does_not_reuse_sequence() {
        use std::process::Command;
        use std::time::{Duration, Instant};

        const WORKER_ENV: &str = "CRYPTEX_CORE_SEQ_CRASH_WORKER";
        const ROOT_ENV: &str = "CRYPTEX_CORE_SEQ_CRASH_ROOT";
        const RESULT_NAME: &str = "crash-allocate-result.txt";
        const READY_NAME: &str = "crash-allocate-ready.txt";

        if std::env::var_os(WORKER_ENV).is_some() {
            let root = PathBuf::from(std::env::var(ROOT_ENV).expect("root env"));
            let device = DeviceId::new("device-crash-1").expect("device");
            let store = SequenceStore::open(&root).expect("open");
            store.initialize(&device).expect("initialize");
            let first = store.allocate(&device).expect("allocate 1");
            let second = store.allocate(&device).expect("allocate 2");
            assert_eq!(first, 1);
            assert_eq!(second, 2);
            // Durable state must be visible before we report success to the parent.
            assert_eq!(store.last_issued(&device).expect("last_issued"), second);
            fs::write(root.join(RESULT_NAME), second.to_string()).expect("write result");
            fs::write(root.join(READY_NAME), "ready").expect("write ready");
            // Hold until the parent forcibly stops this process.
            loop {
                std::thread::sleep(Duration::from_secs(60));
            }
        }

        let root =
            std::env::temp_dir().join(format!("cryptex-m1-seq-crash-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let exe = std::env::current_exe().expect("current_exe");
        let mut child = Command::new(&exe)
            .env(WORKER_ENV, "1")
            .env(ROOT_ENV, &root)
            .env("RUST_TEST_THREADS", "1")
            .args([
                "--exact",
                "sequence_store::tests::crash_restart_after_successful_allocate_does_not_reuse_sequence",
                "--nocapture",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn crash worker");

        let ready_path = root.join(READY_NAME);
        let result_path = root.join(RESULT_NAME);
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if ready_path.exists() && result_path.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            ready_path.exists() && result_path.exists(),
            "worker did not report a successful allocate before timeout"
        );

        let n: u64 = fs::read_to_string(&result_path)
            .unwrap()
            .trim()
            .parse()
            .expect("parse n");
        assert_eq!(
            n, 2,
            "worker should have returned last allocated sequence 2"
        );

        // Forcible stop after successful return (portable kill).
        child.kill().expect("forcibly stop worker");
        let _ = child.wait();

        let store = SequenceStore::open(&root).expect("reopen after crash");
        let device = DeviceId::new("device-crash-1").unwrap();
        assert_eq!(
            store.last_issued(&device).expect("durable last_issued"),
            n,
            "durable state must match the last successfully returned sequence"
        );
        let next = store.allocate(&device).expect("allocate after restart");
        assert!(
            next > n,
            "next allocate must be strictly greater than {n}, got {next}"
        );
        assert_eq!(next, n + 1);
        assert_eq!(store.last_issued(&device).unwrap(), next);
    }
}
