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

// Deliberately test-only until the ADR-0017 rollout gate permits v2 writes.
pub(crate) fn sign_v2_fixture(
    mut body: serde_json::Value,
    key: &ed25519_dalek::SigningKey,
) -> Vec<u8> {
    use cryptex_events::{
        ContentHashHex, SignatureHex, V2_CONTENT_HASH_DOMAIN, V2_SIGNATURE_DOMAIN,
    };
    use ed25519_dalek::Signer;
    use serde_json::json;
    let canonical = serde_json_canonicalizer::to_vec(&body).unwrap();
    let mut hasher = blake3::Hasher::new();
    hasher.update(V2_CONTENT_HASH_DOMAIN);
    hasher.update(&[0]);
    hasher.update(&canonical);
    let hash = hasher.finalize();
    let signature = key.sign(&[V2_SIGNATURE_DOMAIN, &[0], hash.as_bytes()].concat());
    body["content_hash"] = json!(ContentHashHex::from_bytes(hash.as_bytes()).unwrap());
    body["signature"] = json!(SignatureHex::from_bytes(&signature.to_bytes()).unwrap());
    serde_json_canonicalizer::to_vec(&body).unwrap()
}

#[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
pub(crate) fn stage_stopped_m9_profile(source: &Path, parent: &Path, name: &str) -> PathBuf {
    use std::fs::File;
    // Only used on a stopped synthetic profile. Copy regular files exclusively
    // into an empty protected destination, including stale control and receipt files.
    fn copy_entry(source: &Path, dest: &Path) {
        let metadata = fs::symlink_metadata(source).unwrap();
        if metadata.is_dir() {
            fs::create_dir(dest).unwrap();
            for entry in fs::read_dir(source).unwrap() {
                let entry = entry.unwrap();
                copy_entry(&entry.path(), &dest.join(entry.file_name()));
            }
        } else {
            assert!(metadata.is_file());
            let mut output = File::create_new(dest).unwrap();
            std::io::copy(&mut File::open(source).unwrap(), &mut output).unwrap();
            fs::set_permissions(dest, metadata.permissions()).unwrap();
        }
    }

    let restored = parent.join(name);
    if let Some(daemon) = std::env::var_os("CRYPTEX_M9_CANDIDATE_DAEMON") {
        let prepare = |path: &Path| {
            std::process::Command::new(&daemon)
                .args(["lifecycle", "prepare-reader-rejoin-m9", "--runtime-root"])
                .arg(path)
                .output()
                .unwrap()
        };
        let result = prepare(&restored);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            String::from_utf8_lossy(&result.stdout)
                .contains("reader_rejoin=destination_prepared production_v2_activation=blocked")
        );
        assert_eq!(prepare(source).status.code(), Some(2));
        assert_eq!(prepare(&restored).status.code(), Some(2));
    } else {
        crate::instance::create_m9_profile_root(parent, name).unwrap();
    }
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() != "daemon.lock" {
            copy_entry(&entry.path(), &restored.join(entry.file_name()));
        }
    }
    restored
}
