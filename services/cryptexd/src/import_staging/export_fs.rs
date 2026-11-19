//! Original-capability file operations for the daemon export checkpoint.
//! All leaves are backend-generated. No pathname from the renderer is accepted.
use super::*;

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
        self.validate()?;
        #[cfg(windows)]
        {
            fs::create_dir(self.path.join(name))?;
            self.child(name, false)
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
    pub(crate) fn open_file(&self, name: &str, create: bool) -> Result<File, Error> {
        leaf(name)?;
        self.open_regular(name, create)
    }
    pub(crate) fn open_existing_brain_identity(&self) -> Result<File, Error> {
        self.open_regular(crate::identity_runtime::BRAIN_FILE, false)
    }
    fn open_regular(&self, name: &str, create: bool) -> Result<File, Error> {
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
        self.validate()?;
        let staged = crate::instance::migration::STAGED;
        #[cfg(windows)]
        publish_staged_no_replace(&self.path.join(staged), &self.path.join("daemon.lock"))?;
        #[cfg(target_os = "macos")]
        self.native
            .rename_child_exclusive(staged.as_ref(), "daemon.lock".as_ref())?;
        self.sync()
    }

    pub(crate) fn discard_scratch(&self, file: File) -> Result<(), Error> {
        self.unlink_opened("segment.tmp", file)
    }

    pub(crate) fn retire_m8_pid(&self, file: File) -> Result<(), Error> {
        self.unlink_opened("daemon.lock", file)
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
