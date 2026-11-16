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
    edges: Vec<CString>,
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
    pub(super) fn sync_exchange_files(&self) -> Result<(), ProfileError> {
        use super::exchange_install::Leaf;
        self.sync_leaves(&[Leaf::Config, Leaf::Binding, Leaf::Journal])
    }
    pub(super) fn sync_settings_file(&self) -> Result<(), ProfileError> {
        self.sync_leaves(&[super::exchange_install::Leaf::Settings])
    }
    fn sync_leaves(&self, leaves: &[super::exchange_install::Leaf]) -> Result<(), ProfileError> {
        self.require_current()?;
        let mut retained = Vec::new();
        for leaf in leaves {
            let leaf = name(leaf.name())?;
            let file = opened(unsafe {
                libc::openat(
                    self.directory.as_raw_fd(),
                    leaf.as_ptr(),
                    libc::O_RDWR | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                )
            })?;
            validate(&file, false)?;
            file.sync_all().map_err(|_| ProfileError::Io)?;
            if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_FULLFSYNC) } != 0 {
                return Err(ProfileError::Io);
            }
            retained.push(file);
        }
        self.directory.sync_all().map_err(|_| ProfileError::Io)?;
        self.require_current()
    }
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
        let mut edges = Vec::new();
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
                edges.push(part);
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
        edges.push(leaf);
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
            edges,
        })
    }
    fn require_current(&self) -> Result<(), ProfileError> {
        // Retain and recheck the ORIGINAL ancestor-to-child edges. Merely
        // resolving the final absolute path can miss replacement of an ancestor.
        for (index, (parent, edge)) in self._pins.iter().zip(&self.edges).enumerate() {
            let retained = self
                ._pins
                .get(index + 1)
                .unwrap_or(&self.directory)
                .metadata()
                .map_err(|_| ProfileError::Io)?;
            let mut current = std::mem::MaybeUninit::<libc::stat>::uninit();
            if unsafe {
                libc::fstatat(
                    parent.as_raw_fd(),
                    edge.as_ptr(),
                    current.as_mut_ptr(),
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            } != 0
            {
                return Err(ProfileError::UnsafeObject);
            }
            let current = unsafe { current.assume_init() };
            if current.st_mode & libc::S_IFMT != libc::S_IFDIR
                || current.st_dev as u64 != retained.dev()
                || current.st_ino != retained.ino()
            {
                return Err(ProfileError::UnsafeObject);
            }
        }
        let current =
            std::fs::symlink_metadata(&self.path).map_err(|_| ProfileError::UnsafeObject)?;
        let retained = self.directory.metadata().map_err(|_| ProfileError::Io)?;
        if !current.is_dir() || current.dev() != retained.dev() || current.ino() != retained.ino() {
            return Err(ProfileError::UnsafeObject);
        }
        validate(&self.directory, true)
    }
    pub(super) fn replace_exchange_file(
        &self,
        leaf: super::exchange_install::Leaf,
        bytes: &[u8],
    ) -> Result<(), ProfileError> {
        use std::io::{Read, Write};
        self.require_current()?;
        if bytes.len() > leaf.limit() {
            return Err(ProfileError::InvalidState);
        }
        let target = self.open_read(leaf.name())?;
        let temporary = name(leaf.temporary())?;
        let destination = name(leaf.name())?;
        let existed = self.open_read(leaf.temporary())?.is_some();
        let flags = libc::O_RDWR
            | libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | libc::O_NONBLOCK
            | if existed {
                0
            } else {
                libc::O_CREAT | libc::O_EXCL
            };
        let mut stage = opened(unsafe {
            libc::openat(self.directory.as_raw_fd(), temporary.as_ptr(), flags, 0o600)
        })?;
        validate(&stage, false)?;
        if existed {
            let mut actual = Vec::new();
            (&mut stage)
                .take((leaf.limit() + 1) as u64)
                .read_to_end(&mut actual)
                .map_err(|_| ProfileError::Io)?;
            if actual != bytes {
                return Err(ProfileError::InvalidState);
            }
        } else {
            stage.write_all(bytes).map_err(|_| ProfileError::Io)?;
        }
        stage.sync_all().map_err(|_| ProfileError::Io)?;
        if unsafe { libc::fcntl(stage.as_raw_fd(), libc::F_FULLFSYNC) } != 0 {
            return Err(ProfileError::Io);
        }
        self.require_current()?;
        let identity = stage.metadata().map_err(|_| ProfileError::Io)?;
        drop(target);
        if unsafe {
            libc::renameat(
                self.directory.as_raw_fd(),
                temporary.as_ptr(),
                self.directory.as_raw_fd(),
                destination.as_ptr(),
            )
        } != 0
        {
            return Err(ProfileError::Io);
        }
        self.directory.sync_all().map_err(|_| ProfileError::Io)?;
        self.require_current()?;
        let installed = self
            .open_read(leaf.name())?
            .ok_or(ProfileError::InvalidState)?;
        let actual = installed.metadata().map_err(|_| ProfileError::Io)?;
        if actual.dev() != identity.dev() || actual.ino() != identity.ino() {
            return Err(ProfileError::UnsafeObject);
        }
        Ok(())
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

// Darwin may inherit O_NONBLOCK from the listening socket. A read timeout
// alone does not clear it: WouldBlock is not evidence that no bytes arrived.
pub(super) fn foreign_socket_received_no_bytes(mut stream: std::net::TcpStream) -> bool {
    use std::io::Read;
    if stream.set_nonblocking(false).is_err()
        || stream
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .is_err()
    {
        return false;
    }
    let mut byte = [0; 1];
    matches!(stream.read(&mut byte), Ok(0))
}

pub(super) struct ExchangeTree(Vec<crate::import_staging::macos_fs::Directory>);
impl ExchangeTree {
    pub(super) fn validate(&self) -> Result<(), ProfileError> {
        for directory in &self.0 {
            directory
                .validate()
                .map_err(|_| ProfileError::UnsafeObject)?;
        }
        Ok(())
    }
}
pub(super) fn ensure_exchange_tree(root: &Path) -> Result<ExchangeTree, ProfileError> {
    use crate::import_staging::macos_fs::{Directory, create_dir_all_owned};
    let mut tree = ExchangeTree(vec![
        Directory::open(root, true).map_err(|_| ProfileError::UnsafeObject)?,
    ]);
    for path in [root.join("transport"), root.join("transport/incoming")] {
        tree.validate()?;
        create_dir_all_owned(&path).map_err(|_| ProfileError::Io)?;
        tree.0
            .push(Directory::open(&path, true).map_err(|_| ProfileError::UnsafeObject)?);
    }
    tree.validate()?;
    Ok(tree)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    #[test]
    fn original_ancestor_edge_is_required_even_if_final_directory_is_reattached() {
        let mut nonce = [0; 16];
        getrandom::getrandom(&mut nonce).unwrap();
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "cx-private-edges-{}",
            nonce.iter().map(|b| format!("{b:02x}")).collect::<String>()
        ));
        std::fs::create_dir_all(root.join("parent")).unwrap();
        let path = root.join("parent/profile");
        let home = PrivateDirectory::acquire(&path, true).unwrap();
        drop(home.create_file("fixture").unwrap());
        std::fs::rename(root.join("parent"), root.join("detached")).unwrap();
        std::fs::create_dir(root.join("parent")).unwrap();
        std::fs::rename(root.join("detached/profile"), &path).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().ino(),
            home.directory.metadata().unwrap().ino()
        );
        assert!(home.open_read("fixture").is_err());
        assert!(
            home.replace_exchange_file(super::super::exchange_install::Leaf::Config, b"no")
                .is_err()
        );
        assert!(!path.join("config.xml").exists());
        drop(home);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn foreign_socket_probe_waits_for_eof_and_rejects_any_byte() {
        use std::io::Write;
        for send_byte in [false, true] {
            let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
            let mut client = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (accepted, _) = listener.accept().unwrap();
            // Exercise the inherited nonblocking state explicitly, on every OS.
            accepted.set_nonblocking(true).unwrap();
            let reader = std::thread::spawn(move || foreign_socket_received_no_bytes(accepted));
            std::thread::sleep(std::time::Duration::from_millis(30));
            if send_byte {
                client.write_all(b"x").unwrap();
            }
            drop(client);
            assert_eq!(reader.join().unwrap(), !send_byte);
        }
    }

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
        drop(directory.create_file("acl-fixture").unwrap());
        assert!(directory.open_read("acl-fixture").unwrap().is_some());
        assert!(
            std::process::Command::new("/bin/chmod")
                .args(["+a", "everyone allow read"])
                .arg(root.join("acl-fixture"))
                .status()
                .unwrap()
                .success()
        );
        assert!(directory.open_read("acl-fixture").is_err());
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
