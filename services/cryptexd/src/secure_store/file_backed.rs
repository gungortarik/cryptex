//! File-backed SecureStore for cross-process unit/integration tests.
//!
//! Persists secrets under a caller-provided directory (synthetic fixtures must
//! live outside the Git repository). Not for production use.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::{SecureStore, SecureStoreError, SecureStoreResult, validate_name_pair};

/// Durable test SecureStore shared across child processes via the filesystem.
pub struct FileBackedSecureStore {
    root: PathBuf,
    put_count: AtomicU64,
}

impl FileBackedSecureStore {
    pub fn open(root: impl Into<PathBuf>) -> SecureStoreResult<Self> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(|e| SecureStoreError::Platform {
            op: "file_backed_mkdir",
            code: e.raw_os_error().unwrap_or(1) as i64,
        })?;
        Ok(Self {
            root,
            put_count: AtomicU64::new(0),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn put_count(&self) -> u64 {
        self.put_count.load(Ordering::SeqCst)
    }

    fn entry_path(&self, service: &str, account: &str) -> PathBuf {
        // Encode labels: account names contain ':' which is not path-safe on Windows.
        let mut key = Vec::with_capacity(service.len() + 1 + account.len());
        key.extend_from_slice(service.as_bytes());
        key.push(0);
        key.extend_from_slice(account.as_bytes());
        let mut hex = String::with_capacity(key.len() * 2);
        for b in key {
            hex.push_str(&format!("{b:02x}"));
        }
        self.root.join(format!("{hex}.bin"))
    }
}

impl SecureStore for FileBackedSecureStore {
    fn put(&self, service: &str, account: &str, secret: &[u8]) -> SecureStoreResult<()> {
        validate_name_pair(service, account)?;
        let path = self.entry_path(service, account);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| SecureStoreError::Platform {
                op: "file_backed",
                code: e.raw_os_error().unwrap_or(1) as i64,
            })?;
        }
        let tmp = path.with_extension(format!("bin.tmp.{}", std::process::id()));
        {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&tmp)
                .map_err(|e| SecureStoreError::Platform {
                    op: "file_backed",
                    code: e.raw_os_error().unwrap_or(1) as i64,
                })?;
            file.write_all(secret)
                .map_err(|e| SecureStoreError::Platform {
                    op: "file_backed",
                    code: e.raw_os_error().unwrap_or(1) as i64,
                })?;
            file.sync_all().map_err(|e| SecureStoreError::Platform {
                op: "file_backed",
                code: e.raw_os_error().unwrap_or(1) as i64,
            })?;
        }
        fs::rename(&tmp, &path).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            SecureStoreError::Platform {
                op: "file_backed_rename",
                code: e.raw_os_error().unwrap_or(1) as i64,
            }
        })?;
        // Best-effort parent sync on Unix; ignore on platforms without it.
        if let Some(parent) = path.parent()
            && let Ok(dir) = File::open(parent)
        {
            let _ = dir.sync_all();
        }
        self.put_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn get(&self, service: &str, account: &str) -> SecureStoreResult<Vec<u8>> {
        validate_name_pair(service, account)?;
        let path = self.entry_path(service, account);
        match fs::read(&path) {
            Ok(bytes) => Ok(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(SecureStoreError::Missing),
            Err(e) => Err(SecureStoreError::Platform {
                op: "file_backed",
                code: e.raw_os_error().unwrap_or(1) as i64,
            }),
        }
    }

    fn delete(&self, service: &str, account: &str) -> SecureStoreResult<()> {
        validate_name_pair(service, account)?;
        let path = self.entry_path(service, account);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(SecureStoreError::Missing),
            Err(e) => Err(SecureStoreError::Platform {
                op: "file_backed",
                code: e.raw_os_error().unwrap_or(1) as i64,
            }),
        }
    }
}
