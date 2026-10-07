//! Atomic replacement of mutable M1 public metadata, owned by cryptexd.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::identity_runtime::IdentityRuntimeError;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(crate) fn write_public_metadata(path: &Path, bytes: &[u8]) -> Result<(), IdentityRuntimeError> {
    // Serialize writers across processes as well as threads. Windows replacement
    // operations can otherwise contend for the destination's delete handle.
    let lock_path = path.with_extension("json.lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)?;
    fs2::FileExt::lock_exclusive(&lock)?;
    // Closing this handle releases the OS lock on every return path.
    write_with_hooks(
        path,
        bytes,
        |file, bytes| {
            file.write_all(bytes)?;
            file.sync_all()
        },
        replace_atomic,
        remove_temp,
    )?;
    #[cfg(unix)]
    {
        let parent = path.parent().ok_or_else(|| {
            cryptex_core::CoreError::Durability("metadata path has no parent".into())
        })?;
        fs::File::open(parent)
            .and_then(|dir| dir.sync_all())
            .map_err(|error| {
                cryptex_core::CoreError::Durability(format!(
                    "public metadata replaced, but directory durability is unconfirmed: {error}"
                ))
            })?;
    }
    Ok(())
}

fn remove_temp(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

fn write_with_hooks<W, R, C>(
    path: &Path,
    bytes: &[u8],
    write_and_sync: W,
    replace: R,
    cleanup: C,
) -> Result<(), IdentityRuntimeError>
where
    W: FnOnce(&mut fs::File, &[u8]) -> io::Result<()>,
    R: FnOnce(&Path, &Path) -> io::Result<()>,
    C: FnOnce(&Path) -> io::Result<()>,
{
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_nanos())
        .unwrap_or(0);
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_extension(format!(
        "json.tmp.{}-{sequence}-{nanos}",
        std::process::id()
    ));
    let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    let written = write_and_sync(&mut file, bytes);
    drop(file);
    let result = written.and_then(|()| replace(&tmp, path));
    if let Err(primary) = result {
        let primary = primary.into();
        return Err(match cleanup(&tmp) {
            Ok(()) => primary,
            Err(error) => IdentityRuntimeError::PersistenceCleanupFailed {
                primary: Box::new(primary),
                cleanup_error: error.to_string(),
            },
        });
    }
    Ok(())
}

fn replace_atomic(tmp: &Path, path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        };
        let from: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: both pointers refer to valid null-terminated UTF-16 strings.
        if unsafe {
            MoveFileExW(
                from.as_ptr(),
                to.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    #[cfg(unix)]
    {
        fs::rename(tmp, path)
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = (tmp, path);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "atomic metadata replacement is unsupported",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    fn fixture() -> std::path::PathBuf {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "cryptex-m1-metadata-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("fixture root");
        root
    }

    #[test]
    fn concurrent_replacements_publish_complete_payloads_without_temp_collisions() {
        let root = fixture();
        let path = Arc::new(root.join("record.json"));
        let barrier = Arc::new(Barrier::new(16));
        let mut threads = Vec::new();
        for writer in 0..16 {
            let path = Arc::clone(&path);
            let barrier = Arc::clone(&barrier);
            threads.push(std::thread::spawn(move || {
                let payload = format!(
                    "{{\"writer\":{writer},\"padding\":\"{}\"}}\n",
                    "x".repeat(8192)
                );
                barrier.wait();
                write_public_metadata(&path, payload.as_bytes()).expect("atomic write");
                payload
            }));
        }
        let payloads: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect();
        let final_bytes = fs::read_to_string(&*path).unwrap();
        assert!(
            payloads.contains(&final_bytes),
            "final file must be one complete submitted payload"
        );
        assert_eq!(
            fs::read_dir(&root).unwrap().count(),
            2,
            "only metadata and its persistent lock remain"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn partial_write_and_replace_failures_preserve_previous_metadata() {
        let root = fixture();
        let path = root.join("record.json");
        fs::write(&path, b"previous\n").unwrap();
        let err = write_with_hooks(
            &path,
            b"candidate\n",
            |file, bytes| {
                file.write_all(&bytes[..3])?;
                Err(io::Error::other("injected write failure"))
            },
            |_, _| panic!("replace must not run after failed write"),
            remove_temp,
        )
        .unwrap_err();
        assert!(err.to_string().contains("injected write failure"));
        let err = write_with_hooks(
            &path,
            b"candidate\n",
            |file, bytes| {
                file.write_all(bytes)?;
                file.sync_all()
            },
            |_, _| Err(io::Error::other("injected replace failure")),
            remove_temp,
        )
        .unwrap_err();
        assert!(err.to_string().contains("injected replace failure"));
        assert_eq!(fs::read(&path).unwrap(), b"previous\n");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cleanup_failure_preserves_primary_and_is_non_retry_safe() {
        let root = fixture();
        let path = root.join("record.json");
        let err = write_with_hooks(
            &path,
            b"candidate",
            |_, _| Err(io::Error::other("injected sync failure")),
            |_, _| panic!("must not replace"),
            |_| Err(io::Error::other("injected cleanup failure")),
        )
        .unwrap_err();
        assert!(err.to_string().contains("injected sync failure"));
        assert!(err.to_string().contains("injected cleanup failure"));
        assert_eq!(err.whole_operation_retry_safe(), Some(false));
        assert!(!path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_replacement_releases_writer_lock_and_cleans_staging() {
        let root = fixture();
        let path = root.join("record.json");
        fs::create_dir(&path).unwrap();
        assert!(write_public_metadata(&path, b"candidate\n").is_err());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 2);
        fs::remove_dir(&path).unwrap();
        write_public_metadata(&path, b"complete\n").expect("lock released after error");
        assert_eq!(fs::read(&path).unwrap(), b"complete\n");
        fs::remove_dir_all(root).unwrap();
    }
}
