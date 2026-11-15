//! Darwin import capability operations. A retained FD pins an object, not its
//! pathname: every operation checks the observed parent/child identities before
//! and after use. No publication/cleanup operation follows an unchecked path.

use std::ffi::{CStr, CString, OsStr, OsString};
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

#[cfg(test)]
pub(super) use std::fs::{copy, create_dir_all, hard_link, read, remove_dir_all, rename, write};

unsafe extern "C" {
    #[cfg(feature = "m8-local-startup-check")]
    fn cx_import_product_os_supported() -> libc::c_int;
    fn cx_import_acl_safe(fd: libc::c_int) -> libc::c_int;
    fn cx_import_rename_exclusive(
        source: libc::c_int,
        from: *const libc::c_char,
        target: libc::c_int,
        to: *const libc::c_char,
    ) -> libc::c_int;
}

fn unsafe_object() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "unsafe or changed import filesystem object",
    )
}

fn leaf(name: &OsStr) -> io::Result<CString> {
    let bytes = name.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 255
        || bytes == b"."
        || bytes == b".."
        || bytes.contains(&b'/')
    {
        return Err(unsafe_object());
    }
    CString::new(bytes).map_err(|_| unsafe_object())
}

fn owned(fd: libc::c_int) -> io::Result<File> {
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful open/openat transfers one uniquely owned descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn stat_at(parent: &File, name: &CStr) -> io::Result<libc::stat> {
    let mut result = std::mem::MaybeUninit::uninit();
    // SAFETY: name is terminated; the live parent owns its descriptor; stat is
    // initialized by the kernel only when the syscall succeeds.
    if unsafe {
        libc::fstatat(
            parent.as_raw_fd(),
            name.as_ptr(),
            result.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { result.assume_init() })
}

fn normalize(path: &Path) -> io::Result<PathBuf> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(unsafe_object());
    }
    // macOS's OS-owned aliases are not an invitation to canonicalize user
    // symlinks. Check these two exact public aliases, then walk no-follow.
    for (alias, target) in [("/var", "/private/var"), ("/tmp", "/private/tmp")] {
        if let Ok(suffix) = path.strip_prefix(alias) {
            let metadata = std::fs::symlink_metadata(alias)?;
            if metadata.uid() != 0
                || !metadata.file_type().is_symlink()
                || std::fs::canonicalize(alias)? != Path::new(target)
            {
                return Err(unsafe_object());
            }
            return Ok(Path::new(target).join(suffix));
        }
    }
    Ok(path.to_path_buf())
}

fn directory_permissions(file: &File, current_owner: bool) -> io::Result<()> {
    let m = file.metadata()?;
    let uid = unsafe { libc::geteuid() };
    let trusted_sticky = m.uid() == 0 && m.mode() & libc::S_ISVTX as u32 != 0;
    if !m.is_dir()
        || (m.uid() != uid && m.uid() != 0)
        || (current_owner && m.uid() != uid)
        || (m.mode() & 0o022 != 0 && !trusted_sticky)
        || unsafe { cx_import_acl_safe(file.as_raw_fd()) } != 0
    {
        return Err(unsafe_object());
    }
    Ok(())
}

fn regular_permissions(file: &File) -> io::Result<()> {
    let m = file.metadata()?;
    if m.nlink() != 1 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "import regular file must have exactly one link",
        ));
    }
    if !m.is_file()
        || m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o022 != 0
        || unsafe { cx_import_acl_safe(file.as_raw_fd()) } != 0
    {
        return Err(unsafe_object());
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) struct Directory {
    path: PathBuf,
    files: Vec<File>,
    names: Vec<CString>,
}

pub(crate) struct RuntimeIdentity {
    root: PathBuf,
    pins: std::cell::RefCell<std::collections::BTreeMap<PathBuf, Directory>>,
}
impl RuntimeIdentity {
    pub(crate) fn capture(root: &Path) -> io::Result<Self> {
        let directory = Directory::open(root, true)?;
        let root = normalize(root)?;
        Ok(Self {
            root: root.clone(),
            pins: std::cell::RefCell::new(std::collections::BTreeMap::from([(root, directory)])),
        })
    }
    pub(crate) fn pin(&self, path: &Path) -> io::Result<()> {
        self.validate()?;
        let key = normalize(path)?;
        if !key.starts_with(&self.root) {
            return Err(unsafe_object());
        }
        if !self.pins.borrow().contains_key(&key) {
            let directory = Directory::open(path, true)?;
            self.pins.borrow_mut().insert(key, directory);
        }
        self.validate()
    }
    pub(crate) fn validate(&self) -> io::Result<()> {
        for directory in self.pins.borrow().values() {
            directory.validate()?;
        }
        Ok(())
    }
    // Only called after the daemon's authorized bounded cleanup removed this
    // exact subtree. Other original capabilities are never replaced/refreshed.
    pub(super) fn forget_removed(&self, path: &Path) -> io::Result<()> {
        let key = normalize(path)?;
        if key == self.root || !key.starts_with(&self.root) {
            return Err(unsafe_object());
        }
        match symlink_metadata(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            _ => return Err(unsafe_object()),
        }
        self.pins
            .borrow_mut()
            .retain(|path, _| !path.starts_with(&key));
        self.validate()
    }
}

#[derive(Debug)]
pub(crate) struct LockBinding {
    directory: Directory,
    name: OsString,
}
impl LockBinding {
    pub(crate) fn open(path: &Path) -> io::Result<(File, Self)> {
        let directory = Directory::open(path.parent().ok_or_else(unsafe_object)?, true)?;
        let name = path.file_name().ok_or_else(unsafe_object)?.to_owned();
        let file = directory.open_file(&name, libc::O_RDWR | libc::O_CREAT)?;
        let binding = Self { directory, name };
        binding.validate(&file)?;
        Ok((file, binding))
    }
    pub(crate) fn validate(&self, file: &File) -> io::Result<()> {
        self.directory.validate()?;
        regular_permissions(file)?;
        let observed = stat_at(self.directory.last()?, &leaf(&self.name)?)?;
        let pinned = file.metadata()?;
        if (observed.st_dev as u64, observed.st_ino) != (pinned.dev(), pinned.ino()) {
            return Err(unsafe_object());
        }
        self.directory.validate()
    }
}

impl Directory {
    pub(crate) fn open(path: &Path, current_owner: bool) -> io::Result<Self> {
        let requested = path.to_path_buf();
        let path = normalize(path)?;
        crate::paths::assert_outside_repository(&path).map_err(|_| unsafe_object())?;
        let root = owned(unsafe {
            libc::open(
                c"/".as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        })?;
        directory_permissions(&root, false)?;
        let mut result = Self {
            path: requested,
            files: vec![root],
            names: Vec::new(),
        };
        for part in path.components() {
            let Component::Normal(part) = part else {
                continue;
            };
            let name = leaf(part)?;
            let parent = result.files.last().ok_or_else(unsafe_object)?;
            let next = owned(unsafe {
                libc::openat(
                    parent.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            })?;
            directory_permissions(&next, false)?;
            match stat_at(&next, c".git") {
                Ok(_) => return Err(unsafe_object()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            result.files.push(next);
            result.names.push(name);
        }
        directory_permissions(result.last()?, current_owner)?;
        result.validate()?;
        Ok(result)
    }

    fn last(&self) -> io::Result<&File> {
        self.files.last().ok_or_else(unsafe_object)
    }

    pub(crate) fn validate(&self) -> io::Result<()> {
        for (index, name) in self.names.iter().enumerate() {
            let visible = stat_at(&self.files[index], name)?;
            let pinned = self.files[index + 1].metadata()?;
            if (visible.st_dev as u64) != pinned.dev()
                || visible.st_ino != pinned.ino()
                || visible.st_mode & libc::S_IFMT != libc::S_IFDIR
            {
                return Err(unsafe_object());
            }
            directory_permissions(&self.files[index + 1], false)?;
        }
        Ok(())
    }

    /// Remove only this captured file in the original captured directory.
    /// A replacement namespace/file is uncertainty, never a cleanup target.
    #[cfg(feature = "m8-local-startup-check")]
    pub(crate) fn remove_owned_file(&self, name: &OsStr, expected: &File) -> io::Result<()> {
        self.validate()?;
        let current = self.open_file(name, libc::O_RDONLY)?;
        let a = current.metadata()?;
        let b = expected.metadata()?;
        if (a.dev(), a.ino()) != (b.dev(), b.ino()) {
            return Err(unsafe_object());
        }
        let name = leaf(name)?;
        if unsafe { libc::unlinkat(self.last()?.as_raw_fd(), name.as_ptr(), 0) } != 0 {
            return Err(io::Error::last_os_error());
        }
        self.validate()?;
        self.last()?.sync_all()?;
        self.validate()
    }

    pub(super) fn into_files(self) -> Vec<File> {
        self.files
    }

    fn sync(&self) -> io::Result<()> {
        self.validate()?;
        self.last()?.sync_all()?;
        self.validate()
    }

    fn open_file(&self, name: &OsStr, flags: libc::c_int) -> io::Result<File> {
        self.validate()?;
        let name = leaf(name)?;
        let file = owned(unsafe {
            libc::openat(
                self.last()?.as_raw_fd(),
                name.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
                0o600,
            )
        })?;
        regular_permissions(&file)?;
        self.validate()?;
        let visible = stat_at(self.last()?, &name)?;
        let metadata = file.metadata()?;
        if (visible.st_dev as u64) != metadata.dev() || visible.st_ino != metadata.ino() {
            return Err(unsafe_object());
        }
        Ok(file)
    }
}

fn parent(path: &Path) -> io::Result<(Directory, &OsStr)> {
    Ok((
        Directory::open(path.parent().ok_or_else(unsafe_object)?, false)?,
        path.file_name().ok_or_else(unsafe_object)?,
    ))
}

pub(super) fn open_read(path: &Path) -> io::Result<File> {
    let (directory, name) = parent(path)?;
    directory.open_file(name, libc::O_RDONLY)
}

pub(super) fn open_new(path: &Path) -> io::Result<File> {
    let (directory, name) = parent(path)?;
    directory.open_file(name, libc::O_RDWR | libc::O_CREAT | libc::O_EXCL)
}

pub(super) fn open_lock(path: &Path) -> io::Result<File> {
    let (directory, name) = parent(path)?;
    directory.open_file(name, libc::O_RDWR | libc::O_CREAT)
}

pub(super) fn read_child(files: &[File], path: &Path) -> io::Result<File> {
    let directory = Directory::open(path.parent().ok_or_else(unsafe_object)?, true)?;
    if files.len() != directory.files.len() {
        return Err(unsafe_object());
    }
    for (expected, actual) in files.iter().zip(&directory.files) {
        let e = expected.metadata()?;
        let a = actual.metadata()?;
        if (e.dev(), e.ino()) != (a.dev(), a.ino()) {
            return Err(unsafe_object());
        }
    }
    let name = leaf(path.file_name().ok_or_else(unsafe_object)?)?;
    let file = owned(unsafe {
        libc::openat(
            files.last().ok_or_else(unsafe_object)?.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    })?;
    regular_permissions(&file)?;
    directory.validate()?;
    Ok(file)
}

pub(super) fn create_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let (directory, name) = parent(path.as_ref())?;
    let name = leaf(name)?;
    directory.validate()?;
    if unsafe { libc::mkdirat(directory.last()?.as_raw_fd(), name.as_ptr(), 0o700) } != 0 {
        return Err(io::Error::last_os_error());
    }
    directory.sync()
}

pub(crate) fn create_dir_all_owned(path: &Path) -> io::Result<()> {
    let mut missing = Vec::new();
    let mut ancestor = path;
    loop {
        match symlink_metadata(ancestor) {
            Ok(metadata) if metadata.is_dir() => break,
            Ok(_) => return Err(unsafe_object()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                missing.push(ancestor.to_path_buf());
                ancestor = ancestor.parent().ok_or_else(unsafe_object)?;
            }
            Err(error) => return Err(error),
        }
    }
    let original = Directory::open(ancestor, false)?;
    for directory in missing.iter().rev() {
        original.validate()?;
        match create_dir(directory) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        Directory::open(directory, true)?.validate()?;
    }
    original.validate()
}

pub(super) fn remove_file<P: AsRef<Path>>(path: P) -> io::Result<()> {
    remove(path.as_ref(), false)
}
pub(super) fn remove_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
    remove(path.as_ref(), true)
}

fn remove(path: &Path, is_directory: bool) -> io::Result<()> {
    let (parent, name) = parent(path)?;
    let name = leaf(name)?;
    parent.validate()?;
    let file = if is_directory {
        let file = owned(unsafe {
            libc::openat(
                parent.last()?.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        })?;
        directory_permissions(&file, true)?;
        file
    } else {
        parent.open_file(OsStr::from_bytes(name.to_bytes()), libc::O_RDONLY)?
    };
    let observed = stat_at(parent.last()?, &name)?;
    let pinned = file.metadata()?;
    if (observed.st_dev as u64, observed.st_ino) != (pinned.dev(), pinned.ino()) {
        return Err(unsafe_object());
    }
    if unsafe {
        libc::unlinkat(
            parent.last()?.as_raw_fd(),
            name.as_ptr(),
            if is_directory { libc::AT_REMOVEDIR } else { 0 },
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    parent.sync()
}

pub(super) fn flush(file: &File) -> io::Result<()> {
    file.sync_all()?;
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_FULLFSYNC) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub(super) fn sync_parent(path: &Path) -> io::Result<()> {
    Directory::open(path.parent().ok_or_else(unsafe_object)?, true)?.sync()
}

pub(super) fn promote(source: &Path, destination: &Path) -> io::Result<()> {
    let (from, from_name) = parent(source)?;
    let (to, to_name) = parent(destination)?;
    let source_file = from.open_file(from_name, libc::O_RDWR)?;
    let source_meta = source_file.metadata()?;
    let from_name = leaf(from_name)?;
    let to_name = leaf(to_name)?;
    if from.last()?.metadata()?.dev() != to.last()?.metadata()?.dev() {
        return Err(io::Error::from_raw_os_error(libc::EXDEV));
    }
    match stat_at(to.last()?, &to_name) {
        Ok(_) => return Err(io::Error::from(io::ErrorKind::AlreadyExists)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    flush(&source_file)?;
    from.validate()?;
    to.validate()?;
    if unsafe {
        cx_import_rename_exclusive(
            from.last()?.as_raw_fd(),
            from_name.as_ptr(),
            to.last()?.as_raw_fd(),
            to_name.as_ptr(),
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    from.sync()?;
    to.sync()?;
    let published = stat_at(to.last()?, &to_name)?;
    if (published.st_dev as u64, published.st_ino) != (source_meta.dev(), source_meta.ino())
        || published.st_nlink != 1
    {
        return Err(unsafe_object());
    }
    from.validate()?;
    to.validate()
}

pub(super) struct Metadata(libc::stat);
pub(super) struct FileType(libc::mode_t);
impl FileType {
    pub(super) fn is_symlink(&self) -> bool {
        self.0 & libc::S_IFMT == libc::S_IFLNK
    }
}
impl Metadata {
    pub(super) fn dev(&self) -> u64 {
        self.0.st_dev as u64
    }
    pub(super) fn is_dir(&self) -> bool {
        self.0.st_mode & libc::S_IFMT == libc::S_IFDIR
    }
    pub(super) fn is_file(&self) -> bool {
        self.0.st_mode & libc::S_IFMT == libc::S_IFREG
    }
    pub(super) fn file_type(&self) -> FileType {
        FileType(self.0.st_mode)
    }
    pub(super) fn len(&self) -> u64 {
        self.0.st_size.max(0) as u64
    }
}
pub(super) fn symlink_metadata<P: AsRef<Path>>(path: P) -> io::Result<Metadata> {
    let (parent, name) = parent(path.as_ref())?;
    let result = stat_at(parent.last()?, &leaf(name)?)?;
    parent.validate()?;
    Ok(Metadata(result))
}

pub(super) fn metadata<P: AsRef<Path>>(path: P) -> io::Result<Metadata> {
    let result = symlink_metadata(path)?;
    if result.file_type().is_symlink() {
        return Err(unsafe_object());
    }
    Ok(result)
}

pub(super) struct DirEntry {
    path: PathBuf,
    name: OsString,
}
impl DirEntry {
    pub(super) fn path(&self) -> PathBuf {
        self.path.clone()
    }
    pub(super) fn file_name(&self) -> OsString {
        self.name.clone()
    }
}
pub(super) struct ReadDir {
    stream: *mut libc::DIR,
    directory: Directory,
    finished: bool,
}
impl Drop for ReadDir {
    fn drop(&mut self) {
        unsafe {
            libc::closedir(self.stream);
        }
    }
}
impl Iterator for ReadDir {
    type Item = io::Result<DirEntry>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        if let Err(error) = self.directory.validate() {
            self.finished = true;
            return Some(Err(error));
        }
        loop {
            // readdir's result belongs to this stream and is consumed/copied
            // before the next call; errno distinguishes EOF from read failure.
            unsafe {
                *libc::__error() = 0;
            }
            let entry = unsafe { libc::readdir(self.stream) };
            if entry.is_null() {
                self.finished = true;
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(0) {
                    return Some(Err(error));
                }
                return self.directory.validate().err().map(Err);
            }
            let bytes = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
            if bytes == b"." || bytes == b".." {
                continue;
            }
            let name = OsString::from_vec(bytes.to_vec());
            if let Err(error) = leaf(&name) {
                self.finished = true;
                return Some(Err(error));
            }
            return Some(Ok(DirEntry {
                path: self.directory.path.join(&name),
                name,
            }));
        }
    }
}
pub(super) fn read_dir<P: AsRef<Path>>(path: P) -> io::Result<ReadDir> {
    let directory = Directory::open(path.as_ref(), true)?;
    let fd = unsafe { libc::fcntl(directory.last()?.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let stream = unsafe { libc::fdopendir(fd) };
    if stream.is_null() {
        let error = io::Error::last_os_error();
        unsafe {
            libc::close(fd);
        }
        return Err(error);
    }
    Ok(ReadDir {
        stream,
        directory,
        finished: false,
    })
}

#[cfg(feature = "m8-local-startup-check")]
pub(crate) fn require_product_os() -> io::Result<()> {
    // SAFETY: no pointer arguments; the native SDK/runtime availability guard
    // refuses older systems instead of choosing a weaker transport fallback.
    if unsafe { cx_import_product_os_supported() } == 1 {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "native_profile_requires_macos_26",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs as fixture_fs;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn root() -> PathBuf {
        crate::test_temp::exclusive_temp_dir("mac-import-capability")
    }

    #[test]
    fn concurrent_canonical_directory_and_lock_creation_preserves_one_namespace() {
        use std::sync::{Arc, Barrier};
        for _ in 0..16 {
            let root = crate::test_temp::exclusive_temp_dir("mac-concurrent-canonical-lock");
            let directory = root.join("canonical/events/6465766963652d61");
            let barrier = Arc::new(Barrier::new(8));
            let workers = (0..8).map(|_| {
                let directory = directory.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || -> io::Result<()> {
                    barrier.wait();
                    create_dir_all_owned(&directory)?;
                    let (file, binding) = LockBinding::open(&directory.join("writer.lock"))?;
                    fs2::FileExt::lock_exclusive(&file)?;
                    binding.validate(&file)
                })
            }).collect::<Vec<_>>();
            let outcomes = workers.into_iter().map(|worker| worker.join().unwrap()).collect::<Vec<_>>();
            assert!(outcomes.iter().all(Result::is_ok), "concurrent namespace outcomes: {outcomes:?}");
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn private_descriptor_promotion_is_atomic_and_never_replaces_an_occupied_destination() {
        let root = root();
        let staging = root.join("staged");
        let canonical = root.join("canonical");
        create_dir(&staging).unwrap();
        create_dir(&canonical).unwrap();
        let source = staging.join("source.seg");
        let destination = canonical.join("target.seg");
        fixture_fs::write(&source, b"verified staged bytes").unwrap();
        fixture_fs::write(&destination, b"immutable existing bytes").unwrap();
        assert!(promote(&source, &destination).is_err());
        assert_eq!(
            fixture_fs::read(&destination).unwrap(),
            b"immutable existing bytes"
        );
        remove_file(&destination).unwrap();
        promote(&source, &destination).unwrap();
        assert!(!source.exists());
        assert_eq!(
            fixture_fs::read(&destination).unwrap(),
            b"verified staged bytes"
        );
        assert_eq!(
            fixture_fs::metadata(&staging).unwrap().permissions().mode() & 0o777,
            0o700
        );
        fixture_fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn retained_directory_and_runtime_identity_refuse_replacement_instead_of_following_it() {
        let root = root();
        let selected = root.join("selected");
        create_dir(&selected).unwrap();
        let identity = RuntimeIdentity::capture(&selected).unwrap();
        let pins = Directory::open(&selected, true).unwrap().into_files();
        fixture_fs::write(selected.join("note.seg"), b"original").unwrap();
        fixture_fs::rename(&selected, root.join("moved")).unwrap();
        create_dir(&selected).unwrap();
        fixture_fs::write(selected.join("note.seg"), b"replacement").unwrap();
        assert!(identity.validate().is_err());
        assert!(read_child(&pins, &selected.join("note.seg")).is_err());
        assert_eq!(
            fixture_fs::read(root.join("moved/note.seg")).unwrap(),
            b"original"
        );
        drop(identity);
        drop(pins);
        fixture_fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn directory_stream_refuses_an_observed_namespace_change() {
        let root = root();
        let selected = root.join("selected");
        create_dir(&selected).unwrap();
        fixture_fs::write(selected.join("original.seg"), b"original").unwrap();
        let mut entries = read_dir(&selected).unwrap();
        fixture_fs::rename(&selected, root.join("moved")).unwrap();
        create_dir(&selected).unwrap();
        fixture_fs::write(selected.join("replacement.seg"), b"replacement").unwrap();
        assert!(entries.next().unwrap().is_err());
        drop(entries);
        fixture_fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn staging_refusal_never_rolls_back_a_replacement_transaction() {
        let root = root();
        let device = cryptex_core::DeviceId::new("peer").unwrap();
        let relative = PathBuf::from(device.as_filename_stem()).join("00000000000000000000.seg");
        let input = root.join("transport/incoming").join(&relative);
        fixture_fs::create_dir_all(input.parent().unwrap()).unwrap();
        fixture_fs::write(input, b"untrusted candidate").unwrap();
        let mut session = super::super::ImportStagingSession::begin(&root).unwrap();
        let selected = session.staging_dir.clone();
        let moved = root.join("original-transaction");
        fixture_fs::rename(&selected, &moved).unwrap();
        fixture_fs::create_dir_all(selected.join(device.as_filename_stem())).unwrap();
        let bait = selected.join(&relative);
        fixture_fs::write(&bait, b"replacement must survive refusal").unwrap();
        assert!(matches!(
            session.stage_segment(&relative, &device, 0),
            Err(super::super::ImportStagingError::RecoveryConflict(_))
        ));
        assert_eq!(
            fixture_fs::read(&bait).unwrap(),
            b"replacement must survive refusal"
        );
        assert_eq!(fixture_fs::read_dir(&moved).unwrap().count(), 0);
        drop(session);
        fixture_fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unprepared_inventory_never_deletes_a_substituted_valid_transaction_tree() {
        use super::super::namespace_fault::{self, Point};
        let root = root();
        let device = cryptex_core::DeviceId::new("peer").unwrap();
        let relative = PathBuf::from(device.as_filename_stem()).join("00000000000000000000.seg");
        let input = root.join("transport/incoming").join(&relative);
        fixture_fs::create_dir_all(input.parent().unwrap()).unwrap();
        fixture_fs::write(&input, b"same valid unprepared shape").unwrap();
        let mut session = super::super::ImportStagingSession::begin(&root).unwrap();
        session.stage_segment(&relative, &device, 0).unwrap();
        let selected = session.staging_dir.clone();
        let moved = root.join("original-unprepared");
        let changed = selected.clone();
        let saved = moved.clone();
        let path = relative.clone();
        let _hook = namespace_fault::install(Point::UnpreparedValidated, move || {
            fixture_fs::rename(&changed, &saved).unwrap();
            fixture_fs::create_dir_all(changed.join(path.parent().unwrap())).unwrap();
            fixture_fs::copy(saved.join(&path), changed.join(&path)).unwrap();
        });
        assert!(
            matches!(session.discard(), Err(super::super::ImportStagingError::RecoveryConflict(message)) if message.contains("namespace"))
        );
        for transaction in [selected, moved] {
            assert_eq!(
                fixture_fs::read(transaction.join(&relative)).unwrap(),
                b"same valid unprepared shape"
            );
        }
        fixture_fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn symlink_hardlink_and_fifo_sources_never_become_regular_snapshots() {
        let root = root();
        let source = root.join("original.seg");
        let alias = root.join("alias.seg");
        fixture_fs::write(&source, b"original").unwrap();
        symlink(&source, &alias).unwrap();
        assert!(open_read(&alias).is_err());
        fixture_fs::remove_file(&alias).unwrap();
        fixture_fs::hard_link(&source, &alias).unwrap();
        assert!(open_read(&alias).is_err());
        assert!(open_read(&source).is_err());
        fixture_fs::remove_file(&alias).unwrap();
        let fifo = CString::new(alias.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        assert!(open_read(&alias).is_err());
        fixture_fs::remove_file(&alias).unwrap();
        symlink(&root, root.join("redirect")).unwrap();
        assert!(open_read(&root.join("redirect/original.seg")).is_err());
        fixture_fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unsafe_write_permissions_and_extended_allow_acl_are_not_repaired_or_adopted() {
        let root = root();
        let selected = root.join("selected");
        create_dir(&selected).unwrap();
        fixture_fs::set_permissions(&selected, fixture_fs::Permissions::from_mode(0o770)).unwrap();
        assert!(Directory::open(&selected, true).is_err());
        fixture_fs::set_permissions(&selected, fixture_fs::Permissions::from_mode(0o700)).unwrap();
        assert!(
            std::process::Command::new("/bin/chmod")
                .args(["+a", "everyone allow read,write"])
                .arg(&selected)
                .status()
                .unwrap()
                .success()
        );
        assert!(Directory::open(&selected, true).is_err());
        assert!(
            std::process::Command::new("/bin/chmod")
                .arg("-N")
                .arg(&selected)
                .status()
                .unwrap()
                .success()
        );
        assert!(Directory::open(&selected, true).is_ok());
        fixture_fs::remove_dir_all(root).unwrap();
    }
}
