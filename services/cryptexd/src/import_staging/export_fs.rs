//! Original-capability file operations for the daemon export checkpoint.
//! All leaves are backend-generated. No pathname from the renderer is accepted.
use super::*;

#[cfg(feature = "m17-owner-transport")]
pub(crate) mod cold_native;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Error {
    Unsafe,
    Io(io::ErrorKind, Option<i32>),
}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e.kind(), e.raw_os_error())
    }
}
impl From<ImportStagingError> for Error {
    fn from(_: ImportStagingError) -> Self {
        Self::Unsafe
    }
}

pub(crate) fn flush(file: &File) -> io::Result<()> {
    sync_import_file(file)
}

pub(crate) struct Directory {
    path: PathBuf,
    #[cfg(windows)]
    pins: Vec<DirectoryPin>,
    #[cfg(target_os = "macos")]
    native: macos_fs::Directory,
}
impl Directory {
    #[cfg(target_os = "macos")]
    pub(crate) fn into_mac_namespace(self) -> macos_fs::Directory {
        self.native
    }
    pub(crate) fn is_path(&self, path: &Path) -> bool {
        self.path == path
    }
    pub(crate) fn open(path: &Path) -> Result<Self, Error> {
        Ok(Self {
            path: path.to_owned(),
            #[cfg(windows)]
            pins: inspect_directory_chain(path, true)?,
            #[cfg(target_os = "macos")]
            native: macos_fs::Directory::open(path, true)?,
        })
    }
    pub(crate) fn validate(&self) -> Result<(), Error> {
        #[cfg(target_os = "macos")]
        self.native.validate()?;
        #[cfg(windows)]
        if self.pins.is_empty() {
            return Err(Error::Unsafe);
        }
        Ok(())
    }
    pub(crate) fn child(&self, name: &str, create: bool) -> Result<Self, Error> {
        leaf(name)?;
        self.child_validated(name, create)
    }
    fn child_validated(&self, name: &str, create: bool) -> Result<Self, Error> {
        self.validate()?;
        let path = self.path.join(name);
        #[cfg(windows)]
        {
            if create {
                match fs::create_dir(&path) {
                    Ok(()) => {}
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(e) => return Err(e.into()),
                }
            }
            let child = Self::open(&path)?;
            self.validate()?;
            Ok(child)
        }
        #[cfg(target_os = "macos")]
        Ok(Self {
            path,
            native: self.native.export_child(name.as_ref(), create)?,
        })
    }
    /// Enrollment must never adopt a pre-existing or partially prepared tree.
    pub(crate) fn create_child_exclusive(&self, name: &str) -> Result<Self, Error> {
        leaf(name)?;
        self.create_child_exclusive_validated(name)
    }
    fn create_child_exclusive_validated(&self, name: &str) -> Result<Self, Error> {
        self.validate()?;
        #[cfg(windows)]
        {
            fs::create_dir(self.path.join(name))?;
            self.child_validated(name, false)
        }
        #[cfg(target_os = "macos")]
        Ok(Self {
            path: self.path.join(name),
            native: self.native.export_create_child_exclusive(name.as_ref())?,
        })
    }
    pub(crate) fn sync(&self) -> Result<(), Error> {
        self.validate()?;
        #[cfg(target_os = "macos")]
        self.native.export_sync()?;
        #[cfg(windows)]
        sync_staged_parent(&self.path.join("enrollment"))?;
        self.validate()
    }
    pub(crate) fn open_existing_brain_identity(&self) -> Result<File, Error> {
        let name = crate::identity_runtime::BRAIN_FILE;
        self.validate()?;
        #[cfg(windows)]
        let file = open_source_from_directory_chain(&self.pins, &self.path.join(name))?;
        #[cfg(target_os = "macos")]
        let file = self.native.open_file(name.as_ref(), libc::O_RDONLY)?;
        self.checked_regular(file)
    }
    pub(crate) fn open_file(&self, name: &str, create: bool) -> Result<File, Error> {
        leaf(name)?;
        self.open_file_validated(name, create)
    }
    fn open_file_validated(&self, name: &str, create: bool) -> Result<File, Error> {
        self.validate()?;
        #[cfg(windows)]
        let file = {
            use windows_sys::Wdk::Storage::FileSystem::{
                FILE_CREATE, FILE_OPEN, FILE_OPEN_REPARSE_POINT, FILE_SYNCHRONOUS_IO_NONALERT,
            };
            use windows_sys::Win32::Storage::FileSystem::*;
            open_nt_child(
                self.pins.last().ok_or(Error::Unsafe)?,
                name.as_ref(),
                FILE_READ_DATA
                    | FILE_WRITE_DATA
                    | FILE_READ_ATTRIBUTES
                    | DELETE
                    | SYNCHRONIZE
                    | READ_CONTROL,
                FILE_SHARE_READ,
                FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
                if create { FILE_CREATE } else { FILE_OPEN },
                FILE_ATTRIBUTE_NORMAL,
            )?
        };
        #[cfg(target_os = "macos")]
        let file = self.native.export_open(name.as_ref(), create)?;
        self.checked_regular(file)
    }
    fn checked_regular(&self, file: File) -> Result<File, Error> {
        let meta = source_snapshot(&file)?;
        if !meta.is_regular_file || meta.link_count != 1 {
            return Err(Error::Unsafe);
        }
        self.validate()?;
        Ok(file)
    }
    pub(crate) fn exists(&self, name: &str) -> Result<bool, Error> {
        leaf(name)?;
        self.validate()?;
        let result = entry_exists(&self.path.join(name)).map_err(Into::into);
        self.validate()?;
        result
    }
    /// Serialize daemon staging work without holding the event inventory lock.
    pub(crate) fn lock_exclusive(&self, name: &str) -> Result<File, Error> {
        self.lock(name, true)
    }
    pub(crate) fn try_lock_exclusive(&self, name: &str) -> Result<File, Error> {
        self.lock(name, false)
    }
    fn lock(&self, name: &str, wait: bool) -> Result<File, Error> {
        leaf(name)?;
        self.validate()?;
        #[cfg(windows)]
        let file = {
            use windows_sys::Wdk::Storage::FileSystem::{
                FILE_OPEN_IF, FILE_OPEN_REPARSE_POINT, FILE_SYNCHRONOUS_IO_NONALERT,
            };
            use windows_sys::Win32::Storage::FileSystem::*;
            open_nt_child(
                self.pins.last().ok_or(Error::Unsafe)?,
                name.as_ref(),
                FILE_READ_DATA
                    | FILE_WRITE_DATA
                    | FILE_READ_ATTRIBUTES
                    | SYNCHRONIZE
                    | READ_CONTROL,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
                FILE_OPEN_IF,
                FILE_ATTRIBUTE_NORMAL,
            )?
        };
        #[cfg(target_os = "macos")]
        let file = self.native.open_lock(name.as_ref())?;
        let meta = source_snapshot(&file)?;
        if !meta.is_regular_file || meta.link_count != 1 {
            return Err(Error::Unsafe);
        }
        if wait {
            fs2::FileExt::lock_exclusive(&file)?;
        } else {
            fs2::FileExt::try_lock_exclusive(&file)?;
        }
        self.validate()?;
        #[cfg(target_os = "macos")]
        self.native.validate_lock(name.as_ref(), &file)?;
        Ok(file)
    }
    pub(crate) fn promote(
        &self,
        name: &str,
        file: &File,
        to: &Self,
        target: &str,
    ) -> Result<(), Error> {
        leaf(name)?;
        leaf(target)?;
        self.validate()?;
        to.validate()?;
        sync_import_file(file)?;
        #[cfg(target_os = "macos")]
        self.native
            .export_promote(name.as_ref(), file, &to.native, target.as_ref())?;
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Wdk::Storage::FileSystem::{
                FILE_RENAME_INFORMATION, FileRenameInformation, NtSetInformationFile,
            };
            use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;
            let wide: Vec<u16> = target.encode_utf16().collect();
            let length = std::mem::offset_of!(FILE_RENAME_INFORMATION, FileName) + wide.len() * 2;
            let mut storage = vec![0usize; length.div_ceil(std::mem::size_of::<usize>())];
            let info = storage.as_mut_ptr().cast::<FILE_RENAME_INFORMATION>();
            // Aligned allocation, initialized zero (ReplaceIfExists=false),
            // exact trailing UTF-16 byte count and retained target directory.
            unsafe {
                (*info).RootDirectory = to.pins.last().unwrap().as_raw_handle();
                (*info).FileNameLength = (wide.len() * 2) as u32;
                std::ptr::copy_nonoverlapping(
                    wide.as_ptr(),
                    (*info).FileName.as_mut_ptr(),
                    wide.len(),
                );
            }
            let mut ios = IO_STATUS_BLOCK::default();
            let status = unsafe {
                NtSetInformationFile(
                    file.as_raw_handle(),
                    &mut ios,
                    info.cast(),
                    length as u32,
                    FileRenameInformation,
                )
            };
            if status < 0 {
                return Err(io::Error::from_raw_os_error(unsafe {
                    windows_sys::Win32::Foundation::RtlNtStatusToDosError(status)
                } as i32)
                .into());
            }
            sync_import_file(file)?;
            sync_staged_parent(&to.path.join(target))?;
            if self.path != to.path {
                sync_staged_parent(&self.path.join(name))?;
            }
        }
        self.validate()?;
        to.validate()
    }
    /// Publish the prepared profile lock directory without displacing a legacy
    /// process that recreated its PID file during the cutover.
    pub(crate) fn publish_m9_lock(&self) -> Result<(), Error> {
        let staged = crate::instance::migration::STAGED;
        self.publish_child(staged, self, "daemon.lock")
    }

    pub(crate) fn publish_child(&self, staged: &str, to: &Self, name: &str) -> Result<(), Error> {
        leaf(staged)?;
        leaf(name)?;
        self.validate()?;
        to.validate()?;
        #[cfg(windows)]
        publish_staged_no_replace(&self.path.join(staged), &to.path.join(name))?;
        #[cfg(target_os = "macos")]
        self.native
            .move_child_exclusive(staged.as_ref(), &to.native, name.as_ref())?;
        if to.path != self.path {
            to.sync()?;
        }
        self.sync()
    }

    #[cfg(all(
        test,
        any(windows, all(target_os = "macos", feature = "m8-local-startup-check"))
    ))]
    pub(crate) fn remove_purged_exchange(&self, index: u64, file: File) -> Result<(), Error> {
        self.unlink_opened(&format!("{index:020}.seg"), file)
    }

    pub(crate) fn discard_scratch(&self, file: File) -> Result<(), Error> {
        self.unlink_opened("segment.tmp", file)
    }

    #[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
    pub(crate) fn discard_partial_manual_export(&self, id: u128, file: File) -> Result<(), Error> {
        self.unlink_opened(&format!("manual-{id:032x}.partial"), file)
    }

    #[cfg(all(
        test,
        any(windows, all(target_os = "macos", feature = "m8-local-startup-check"))
    ))]
    pub(crate) fn archived_segments(&self) -> Result<Vec<(u64, String)>, Error> {
        self.validate()?;
        let mut segments = Vec::new();
        for entry in fs::read_dir(&self.path)? {
            let name = entry?.file_name();
            if let Some(name) = name.to_str()
                && let Some(index) = archived_segment_index(name)
            {
                segments.push((index, name.to_owned()));
            }
        }
        self.validate()?;
        Ok(segments)
    }

    #[cfg(all(
        test,
        any(windows, all(target_os = "macos", feature = "m8-local-startup-check"))
    ))]
    pub(crate) fn discard_archived_segment(&self, name: &str, file: File) -> Result<(), Error> {
        archived_segment_index(name).ok_or(Error::Unsafe)?;
        self.unlink_opened(name, file)
    }

    #[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
    pub(crate) fn remove_reader_slot(&self, name: &str, file: File) -> Result<(), Error> {
        if !name.strip_suffix(".control").is_some_and(|id| {
            id.len() == 64
                && id
                    .bytes()
                    .all(|c| c.is_ascii_digit() || matches!(c, b'a'..=b'f'))
        }) {
            return Err(Error::Unsafe);
        }
        self.unlink_opened(name, file)
    }

    pub(crate) fn remove_blob(&self, digest: &[u8; 32], file: File) -> Result<(), Error> {
        let name = format!("{:x}", sha2::digest::Output::<sha2::Sha256>::from(*digest));
        self.unlink_opened(&name, file)
    }

    pub(crate) fn remove_reader_receipt(&self, name: &str, file: File) -> Result<(), Error> {
        if !name.strip_suffix(".receipt").is_some_and(|id| {
            id.len() == 32
                && id
                    .bytes()
                    .all(|c| c.is_ascii_digit() || matches!(c, b'a'..=b'f'))
        }) {
            return Err(Error::Unsafe);
        }
        self.unlink_opened(name, file)
    }

    pub(crate) fn retire_m8_pid(&self, file: File) -> Result<(), Error> {
        self.unlink_opened("daemon.lock", file)
    }

    #[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
    pub(crate) fn remove_backup_generation(&self, number: u64, file: File) -> Result<(), Error> {
        self.unlink_opened(&format!("backup-{number:020}.pgp"), file)
    }

    fn unlink_opened(&self, name: &str, file: File) -> Result<(), Error> {
        self.validate()?;
        #[cfg(target_os = "macos")]
        self.native.remove_owned_file(name.as_ref(), &file)?;
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::Storage::FileSystem::{
                FILE_DISPOSITION_INFO, FileDispositionInfo, SetFileInformationByHandle,
            };
            let info = FILE_DISPOSITION_INFO { DeleteFile: true };
            if unsafe {
                SetFileInformationByHandle(
                    file.as_raw_handle(),
                    FileDispositionInfo,
                    (&info as *const FILE_DISPOSITION_INFO).cast(),
                    std::mem::size_of_val(&info) as u32,
                )
            } == 0
            {
                return Err(io::Error::last_os_error().into());
            }
            drop(file);
            sync_staged_parent(&self.path.join(name))?;
        }
        self.validate()
    }
}
fn leaf(name: &str) -> Result<(), Error> {
    #[cfg(all(
        test,
        any(windows, all(target_os = "macos", feature = "m8-local-startup-check"))
    ))]
    if name.contains('~') && archived_segment_index(name).is_some() {
        return Ok(());
    }
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        || matches!(name, "." | "..")
    {
        return Err(Error::Unsafe);
    }
    Ok(())
}

/// Syncthing trashcan and timestamped versions of a daemon-generated sealed pack.
#[cfg(all(
    test,
    any(windows, all(target_os = "macos", feature = "m8-local-startup-check"))
))]
fn archived_segment_index(name: &str) -> Option<u64> {
    let stem = name.strip_suffix(".seg")?;
    let index = if let Some((index, stamp)) = stem.split_once('~') {
        let (date, time) = stamp.split_once('-')?;
        if date.len() != 8
            || time.len() != 6
            || !date.bytes().chain(time.bytes()).all(|b| b.is_ascii_digit())
        {
            return None;
        }
        index
    } else {
        stem
    };
    if index.len() != 20 || !index.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    index.parse().ok()
}
