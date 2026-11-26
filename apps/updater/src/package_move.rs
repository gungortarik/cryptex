//! A recently closed Windows image may briefly retain a handle without delete sharing.
use std::{fs, io, path::Path};

pub(super) fn rename(source: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::time::{Duration, Instant};
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match fs::rename(source, destination) {
                Err(err)
                    if matches!(err.raw_os_error(), Some(5 | 32)) && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(50));
                }
                result => return result,
            }
        }
    }
    #[cfg(not(windows))]
    fs::rename(source, destination)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::{os::windows::fs::OpenOptionsExt, thread, time::Duration};

    fn hold_directory(path: &Path) -> fs::File {
        fs::OpenOptions::new()
            .read(true)
            .share_mode(1) // FILE_SHARE_READ
            .custom_flags(0x0200_0000) // FILE_FLAG_BACKUP_SEMANTICS opens a directory.
            .open(path)
            .unwrap()
    }

    #[test]
    fn selection_waits_for_a_closing_image_handle() {
        let root = tempfile::tempdir().unwrap();
        let active = root.path().join("active");
        let retained = root.path().join("previous");
        fs::create_dir(&active).unwrap();
        fs::write(active.join("image"), b"working package").unwrap();
        let handle = hold_directory(&active);
        let error = fs::rename(&active, &retained).unwrap_err();
        assert!(matches!(error.raw_os_error(), Some(5 | 32)));
        thread::scope(|scope| {
            scope.spawn(move || {
                thread::sleep(Duration::from_millis(200));
                drop(handle);
            });
            rename(&active, &retained).unwrap();
        });
        assert!(!active.exists());
        assert_eq!(
            fs::read(retained.join("image")).unwrap(),
            b"working package"
        );
    }

    #[test]
    fn persistent_lock_preserves_the_original_package() {
        let root = tempfile::tempdir().unwrap();
        let active = root.path().join("active");
        let retained = root.path().join("previous");
        fs::create_dir(&active).unwrap();
        fs::write(active.join("image"), b"working package").unwrap();
        let _handle = hold_directory(&active);
        let error = rename(&active, &retained).unwrap_err();
        assert!(matches!(error.raw_os_error(), Some(5 | 32)));
        assert!(!retained.exists());
        assert_eq!(fs::read(active.join("image")).unwrap(), b"working package");
    }
}
