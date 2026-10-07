//! File/directory durability helpers (ADR-0016).
//!
//! Required sync failures must surface as typed errors. Success is never reported
//! when a required sync did not complete.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

use crate::{StorageError, StorageResult};

#[cfg(test)]
thread_local! {
    static FAIL_DIR_SYNC_REMAINING: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Test-only: next `count` [`sync_directory`] calls fail on this thread.
#[cfg(test)]
pub fn fail_next_directory_syncs(count: u32) {
    FAIL_DIR_SYNC_REMAINING.with(|f| f.set(count));
}

/// Test-only: next [`sync_directory`] call fails once on this thread.
#[cfg(test)]
pub fn fail_next_directory_sync() {
    fail_next_directory_syncs(1);
}

/// Sync an already-open file to durable storage.
pub fn sync_file(file: &File) -> StorageResult<()> {
    file.sync_all()
        .map_err(|err| StorageError::Durability(format!("file sync_all failed: {err}")))
}

/// Sync the directory at `dir` so newly created entries become durable.
///
/// On Unix this opens the directory and calls `sync_all`. On other first-class
/// targets directory sync is not required by the platform model and returns Ok.
pub fn sync_directory(dir: &Path) -> StorageResult<()> {
    #[cfg(test)]
    {
        let inject = FAIL_DIR_SYNC_REMAINING.with(|flag| {
            let remaining = flag.get();
            if remaining > 0 {
                flag.set(remaining - 1);
                true
            } else {
                false
            }
        });
        if inject {
            return Err(StorageError::Durability(
                "injected directory sync failure".into(),
            ));
        }
    }

    #[cfg(unix)]
    {
        let file = File::open(dir).map_err(|err| {
            StorageError::Durability(format!("open directory for sync {}: {err}", dir.display()))
        })?;
        file.sync_all().map_err(|err| {
            StorageError::Durability(format!(
                "directory sync_all failed for {}: {err}",
                dir.display()
            ))
        })?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
        Ok(())
    }
}

/// Sync the parent directory of `path`.
pub fn sync_parent_directory(path: &Path) -> StorageResult<()> {
    let parent = path.parent().ok_or_else(|| {
        StorageError::Durability(format!(
            "path has no parent directory for sync: {}",
            path.display()
        ))
    })?;
    sync_directory(parent)
}

/// Stream-compare two files for exact byte identity without loading either whole file.
pub fn files_byte_identical(a: &Path, b: &Path) -> StorageResult<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let ma = fs::metadata(a)?;
        let mb = fs::metadata(b)?;
        if ma.dev() == mb.dev() && ma.ino() == mb.ino() {
            return Ok(true);
        }
        if ma.len() != mb.len() {
            return Ok(false);
        }
    }
    #[cfg(not(unix))]
    {
        let ma = fs::metadata(a)?;
        let mb = fs::metadata(b)?;
        if ma.len() != mb.len() {
            return Ok(false);
        }
    }

    let mut fa = File::open(a)?;
    let mut fb = File::open(b)?;
    let mut ba = [0u8; 65_536];
    let mut bb = [0u8; 65_536];
    loop {
        let na = fa.read(&mut ba)?;
        let nb = fb.read(&mut bb)?;
        if na != nb || ba[..na] != bb[..nb] {
            return Ok(false);
        }
        if na == 0 {
            return Ok(true);
        }
    }
}

/// Map raw IO into durability when the caller knows the operation was a required sync.
pub fn io_as_durability(context: &str, err: io::Error) -> StorageError {
    StorageError::Durability(format!("{context}: {err}"))
}
