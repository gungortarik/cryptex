//! Test-only exclusive temporary directory allocation.
//!
//! Avoids timestamp-only fixture names and pre-delete races under parallel
//! `cargo test -- --test-threads>1` (observed on macOS CI).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_ROOT_SEQ: AtomicU64 = AtomicU64::new(1);

/// Create a unique directory under [`std::env::temp_dir`].
///
/// Uses `{prefix}-{pid}-{seq}`, exclusive [`fs::create_dir`], and retries on
/// [`std::io::ErrorKind::AlreadyExists`]. Never pre-deletes a candidate path.
pub fn exclusive_temp_dir(prefix: &str) -> PathBuf {
    exclusive_temp_dir_in(&std::env::temp_dir(), prefix)
}

/// Create a unique directory under `parent` (creating `parent` if needed).
pub fn exclusive_temp_dir_in(parent: &Path, prefix: &str) -> PathBuf {
    fs::create_dir_all(parent).unwrap_or_else(|err| {
        panic!(
            "exclusive_temp_dir_in: create_dir_all failed for {}: {err}",
            parent.display()
        )
    });
    for _ in 0..10_000 {
        let seq = TEMP_ROOT_SEQ.fetch_add(1, Ordering::Relaxed);
        let root = parent.join(format!("{prefix}-{}-{seq}", std::process::id()));
        match fs::create_dir(&root) {
            Ok(()) => return root,
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => panic!(
                "exclusive_temp_dir_in: create_dir failed for {}: {err}",
                root.display()
            ),
        }
    }
    panic!("exclusive_temp_dir_in: exhausted unique candidates for prefix {prefix}");
}
