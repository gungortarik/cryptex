//! Default-off Mac private profile adapter. No ACL repair or symlink adoption.
use super::ProfileError;
use std::ffi::CString;
use std::fs::File;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

unsafe extern "C" {
    fn cx_transport_private_acl(fd: libc::c_int) -> libc::c_int;
}
pub(super) struct PrivateDirectory {
    path: PathBuf,
    directory: File,
    _lock: File,
    _pins: Vec<File>,
}
fn name(value: &str) -> Result<CString, ProfileError> {
    if value.is_empty()
        || value.len() > 128
        || matches!(value, "." | "..")
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
    {
        return Err(ProfileError::InvalidPath);
    }
    CString::new(value).map_err(|_| ProfileError::InvalidPath)
}
fn opened(fd: i32) -> Result<File, ProfileError> {
    if fd < 0 {
        return Err(ProfileError::Io);
    }
    // SAFETY: a successful open/openat transfers sole descriptor ownership.
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn validate(file: &File, directory: bool) -> Result<(), ProfileError> {
    let m = file.metadata().map_err(|_| ProfileError::Io)?;
    if (directory && !m.is_dir()) || (!directory && (!m.is_file() || m.nlink() != 1)) {
        return Err(ProfileError::UnsafeObject);
    }
    if m.uid() != unsafe { libc::geteuid() }
        || m.mode() & if directory { 0o077 } else { 0o022 } != 0
        || unsafe { cx_transport_private_acl(file.as_raw_fd()) } != 0
    {
        return Err(ProfileError::UnsafePermissions);
    }
    Ok(())
}
impl PrivateDirectory {
    pub(super) fn path(&self) -> &Path {
        &self.path
    }
    pub(super) fn acquire(path: &Path, create: bool) -> Result<Self, ProfileError> {
        if !path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        {
            return Err(ProfileError::InvalidPath);
        }
        crate::paths::assert_outside_repository(path).map_err(|_| ProfileError::InvalidPath)?;
        let parent = path
            .parent()
            .ok_or(ProfileError::InvalidPath)?
            .canonicalize()
            .map_err(|_| ProfileError::Io)?;
        crate::paths::assert_outside_repository(&parent).map_err(|_| ProfileError::InvalidPath)?;
        let leaf = name(
            path.file_name()
                .and_then(|s| s.to_str())
                .ok_or(ProfileError::InvalidPath)?,
        )?;
        let mut directory = opened(unsafe {
            libc::open(
                c"/".as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        })?;
        let mut pins = Vec::new();
        for part in parent.components() {
            if let Component::Normal(part) = part {
                let part = CString::new(part.as_bytes()).map_err(|_| ProfileError::InvalidPath)?;
                let next = opened(unsafe {
                    libc::openat(
                        directory.as_raw_fd(),
                        part.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    )
                })?;
                pins.push(directory);
                directory = next;
                let mut st = std::mem::MaybeUninit::<libc::stat>::uninit();
                if unsafe {
                    libc::fstatat(
                        directory.as_raw_fd(),
                        c".git".as_ptr(),
                        st.as_mut_ptr(),
                        libc::AT_SYMLINK_NOFOLLOW,
                    )
                } == 0
                    || std::io::Error::last_os_error().raw_os_error() != Some(libc::ENOENT)
                {
                    return Err(ProfileError::InvalidPath);
                }
            }
        }
        if create && unsafe { libc::mkdirat(directory.as_raw_fd(), leaf.as_ptr(), 0o700) } != 0 {
            return Err(
                if std::io::Error::last_os_error().raw_os_error() == Some(libc::EEXIST) {
                    ProfileError::AlreadyExists
                } else {
                    ProfileError::Io
                },
            );
        }
        let selected = opened(unsafe {
            libc::openat(
                directory.as_raw_fd(),
                leaf.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        })?;
        validate(&selected, true)?;
        pins.push(directory);
        let lock = opened(unsafe {
            libc::openat(
                selected.as_raw_fd(),
                c"session.lock".as_ptr(),
                libc::O_RDWR
                    | libc::O_CREAT
                    | libc::O_NOFOLLOW
                    | libc::O_CLOEXEC
                    | libc::O_NONBLOCK,
                0o600,
            )
        })?;
        validate(&lock, false)?;
        fs2::FileExt::try_lock_exclusive(&lock).map_err(|_| ProfileError::Busy)?;
        Ok(Self {
            path: parent.join(path.file_name().unwrap()),
            directory: selected,
            _lock: lock,
            _pins: pins,
        })
    }
    fn require_current(&self) -> Result<(), ProfileError> {
        let current =
            std::fs::symlink_metadata(&self.path).map_err(|_| ProfileError::UnsafeObject)?;
        let retained = self.directory.metadata().map_err(|_| ProfileError::Io)?;
        if !current.is_dir() || current.dev() != retained.dev() || current.ino() != retained.ino() {
            return Err(ProfileError::UnsafeObject);
        }
        validate(&self.directory, true)
    }
    pub(super) fn child(&self, leaf: &str, allow_create: bool) -> Result<Self, ProfileError> {
        self.require_current()?;
        if !matches!(leaf, "generation" | "native") {
            return Err(ProfileError::InvalidPath);
        }
        let path = self.path.join(leaf);
        let exists = path.try_exists().map_err(|_| ProfileError::Io)?;
        if !exists && !allow_create {
            return Err(ProfileError::InvalidState);
        }
        Self::acquire(&path, !exists)
    }
    pub(super) fn create_file(&self, leaf: &str) -> Result<File, ProfileError> {
        self.require_current()?;
        let leaf = name(leaf)?;
        let file = opened(unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                leaf.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        })?;
        validate(&file, false)?;
        self.directory.sync_all().map_err(|_| ProfileError::Io)?;
        Ok(file)
    }
    pub(super) fn open_read(&self, leaf: &str) -> Result<Option<File>, ProfileError> {
        self.require_current()?;
        let leaf = name(leaf)?;
        let fd = unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                leaf.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            )
        };
        if fd < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ENOENT) {
            return Ok(None);
        }
        let file = opened(fd)?;
        validate(&file, false)?;
        Ok(Some(file))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    #[test]
    fn retained_private_profile_refuses_links_permissions_and_competing_session() {
        let mut nonce = [0; 16];
        getrandom::getrandom(&mut nonce).unwrap();
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "cx-private-{}",
            nonce.iter().map(|b| format!("{b:02x}")).collect::<String>()
        ));
        let directory = PrivateDirectory::acquire(&root, true).unwrap();
        assert!(matches!(
            PrivateDirectory::acquire(&root, false),
            Err(ProfileError::Busy)
        ));
        drop(directory.create_file("fixture").unwrap());
        std::fs::hard_link(root.join("fixture"), root.join("alias")).unwrap();
        assert!(directory.open_read("fixture").is_err());
        std::fs::set_permissions(root.join("fixture"), std::fs::Permissions::from_mode(0o600))
            .unwrap();
        assert!(directory.open_read("fixture").unwrap().is_some());
        assert!(
            std::process::Command::new("/bin/chmod")
                .args(["+a", "everyone allow read"])
                .arg(root.join("fixture"))
                .status()
                .unwrap()
                .success()
        );
        assert!(directory.open_read("fixture").is_err());
        std::fs::remove_file(root.join("alias")).unwrap();
        symlink(root.join("fixture"), root.join("link")).unwrap();
        assert!(directory.open_read("link").is_err());
        std::fs::set_permissions(root.join("fixture"), std::fs::Permissions::from_mode(0o620))
            .unwrap();
        assert!(directory.open_read("fixture").is_err());
        assert!(directory.create_file("../escape").is_err());
        drop(directory);
        std::fs::remove_dir_all(root).unwrap();
    }
}
