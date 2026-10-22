//! Native Windows private-directory boundary. No ACL repair/adoption fallback.

use std::ffi::c_void;
use std::fs::{File, OpenOptions};
use std::mem::{size_of, zeroed};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::{Component, Path, PathBuf};
use std::ptr;

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, ERROR_SHARING_VIOLATION, GENERIC_READ,
    GENERIC_WRITE, GetLastError, HANDLE, INVALID_HANDLE_VALUE, LocalFree,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo,
    SDDL_REVISION_1, SE_FILE_OBJECT,
};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACE_HEADER, CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, EqualSid,
    GetAce, GetSecurityDescriptorControl, GetTokenInformation, INHERITED_ACE, OBJECT_INHERIT_ACE,
    OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, SE_DACL_PROTECTED, SECURITY_ATTRIBUTES,
    TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, CREATE_NEW, CreateDirectoryW, CreateFileW, FILE_ALL_ACCESS,
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE,
    GetFileInformationByHandle, OPEN_ALWAYS, OPEN_EXISTING, READ_CONTROL,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use super::ProfileError;

fn failure(operation: &'static str) -> ProfileError {
    ProfileError::Platform {
        operation,
        code: unsafe { GetLastError() },
    }
}

struct Token(HANDLE);
impl Drop for Token {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

struct LocalAllocation(*mut c_void);
impl Drop for LocalAllocation {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}

struct CurrentUser(Vec<usize>);
impl CurrentUser {
    fn load() -> Result<Self, ProfileError> {
        let mut token = ptr::null_mut();
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(failure("open_token"));
        }
        let token = Token(token);
        let mut required = 0;
        unsafe {
            GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut required);
        }
        if required < size_of::<TOKEN_USER>() as u32 || required > 65536 {
            return Err(ProfileError::UnsafePermissions);
        }
        let mut storage = vec![0usize; (required as usize).div_ceil(size_of::<usize>())];
        if unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                storage.as_mut_ptr().cast(),
                required,
                &mut required,
            )
        } == 0
        {
            return Err(failure("token_user"));
        }
        Ok(Self(storage))
    }

    fn sid(&self) -> *mut c_void {
        // Aligned storage contains the successful TokenUser result and owns all
        // its embedded SID bytes until this CurrentUser is dropped.
        unsafe { (*self.0.as_ptr().cast::<TOKEN_USER>()).User.Sid }
    }

    fn descriptor(&self, directory: bool) -> Result<LocalAllocation, ProfileError> {
        let mut sid = ptr::null_mut();
        if unsafe { ConvertSidToStringSidW(self.sid(), &mut sid) } == 0 {
            return Err(failure("sid_text"));
        }
        let allocation = LocalAllocation(sid.cast());
        let mut length = 0;
        while length < 256 && unsafe { *sid.add(length) } != 0 {
            length += 1;
        }
        if length == 256 {
            return Err(ProfileError::UnsafePermissions);
        }
        let sid = String::from_utf16(unsafe { std::slice::from_raw_parts(sid, length) })
            .map_err(|_| ProfileError::UnsafePermissions)?;
        drop(allocation);
        // Explicit current-user owner and ONE protected inheritable allow ACE.
        // No administrator/SYSTEM/Everyone grant; not a sandbox from this user
        // or a privileged OS actor.
        let text = if directory {
            format!("O:{sid}D:P(A;OICI;FA;;;{sid})")
        } else {
            format!("O:{sid}")
        };
        let sddl: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(failure("private_descriptor"));
        }
        Ok(LocalAllocation(descriptor))
    }
}

pub(super) struct PrivateDirectory {
    path: PathBuf,
    // Close exclusive lock before directory pins on drop.
    _lock: File,
    _pins: Vec<File>,
    user: CurrentUser,
}

impl PrivateDirectory {
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn child(&self, name: &str, allow_create: bool) -> Result<Self, ProfileError> {
        // Internal fixed component, never an imported path.
        if !matches!(name, "generation" | "native") {
            return Err(ProfileError::InvalidPath);
        }
        let path = self.path.join(name);
        let exists = path.try_exists().map_err(|_| ProfileError::Io)?;
        if !exists && !allow_create {
            return Err(ProfileError::InvalidState);
        }
        Self::acquire(&path, !exists)
    }

    pub(super) fn acquire(path: &Path, create: bool) -> Result<Self, ProfileError> {
        if !path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
            || !matches!(path.components().next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), std::path::Prefix::Disk(_)))
            || crate::paths::assert_outside_repository(path).is_err()
        {
            return Err(ProfileError::InvalidPath);
        }
        let user = CurrentUser::load()?;
        let parent = path.parent().ok_or(ProfileError::InvalidPath)?;
        let mut pins = Vec::new();
        // Walk top-down, pinning EACH existing ancestor without DELETE sharing.
        // OPEN_REPARSE_POINT plus handle attributes rejects junctions/symlinks.
        // While pins live, these entries cannot be renamed/replaced through
        // ordinary Windows sharing semantics. This is not M7 Vault API proof.
        for ancestor in parent.ancestors().collect::<Vec<_>>().into_iter().rev() {
            let file = open_directory(ancestor)?;
            if ancestor
                .join(".git")
                .try_exists()
                .map_err(|_| ProfileError::Io)?
            {
                return Err(ProfileError::InvalidPath);
            }
            pins.push(file);
        }
        if create {
            let descriptor = user.descriptor(true)?;
            let attributes = SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor.0,
                bInheritHandle: 0,
            };
            let wide = wide_path(path)?;
            if unsafe { CreateDirectoryW(wide.as_ptr(), &attributes) } == 0 {
                return Err(if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
                    ProfileError::AlreadyExists
                } else {
                    failure("create_private_directory")
                });
            }
        }
        let directory = open_directory(path)?;
        validate_permissions(&directory, &user, true)?;
        pins.push(directory);
        // Lock is only an ownership/session exclusion file, never a readiness
        // indicator. A stale file from an exited owner can be reopened safely.
        let lock = create_owned_file(&path.join("session.lock"), &user, OPEN_ALWAYS)?;
        validate_file(&lock, &user)?;
        Ok(Self {
            path: path.to_owned(),
            _lock: lock,
            _pins: pins,
            user,
        })
    }

    pub(super) fn create_file(&self, name: &str) -> Result<File, ProfileError> {
        let file = create_owned_file(&self.path.join(name), &self.user, CREATE_NEW)?;
        validate_file(&file, &self.user)?;
        Ok(file)
    }

    pub(super) fn open_read(&self, name: &str) -> Result<Option<File>, ProfileError> {
        match OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(self.path.join(name))
        {
            Ok(file) => {
                validate_file(&file, &self.user)?;
                Ok(Some(file))
            }
            Err(e) if e.raw_os_error() == Some(ERROR_FILE_NOT_FOUND as i32) => Ok(None),
            Err(_) => Err(ProfileError::Io),
        }
    }
}

fn create_owned_file(
    path: &Path,
    user: &CurrentUser,
    disposition: u32,
) -> Result<File, ProfileError> {
    // Set the owner explicitly even for an elevated token whose default owner
    // might otherwise be the Administrators group. DACL inherits the sole
    // current-user grant from the pinned private parent.
    let descriptor = user.descriptor(false)?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    let wide = wide_path(path)?;
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            0,
            &attributes,
            disposition,
            FILE_FLAG_OPEN_REPARSE_POINT,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(if unsafe { GetLastError() } == ERROR_SHARING_VIOLATION {
            ProfileError::Busy
        } else {
            failure("open_private_file")
        });
    }
    Ok(unsafe { File::from_raw_handle(handle) })
}

fn wide_path(path: &Path) -> Result<Vec<u16>, ProfileError> {
    let mut result: Vec<u16> = path.as_os_str().encode_wide().collect();
    if result.contains(&0) {
        return Err(ProfileError::InvalidPath);
    }
    result.push(0);
    Ok(result)
}

fn open_directory(path: &Path) -> Result<File, ProfileError> {
    let wide = wide_path(path)?;
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_READ_ATTRIBUTES | READ_CONTROL,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(failure("pin_directory"));
    }
    // SAFETY: this new non-inherited handle transfers to File exactly once.
    let file = unsafe { File::from_raw_handle(handle) };
    let attributes = attributes(&file)?;
    if attributes & FILE_ATTRIBUTE_DIRECTORY == 0 || attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(ProfileError::UnsafeObject);
    }
    Ok(file)
}

fn attributes(file: &File) -> Result<u32, ProfileError> {
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { zeroed() };
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(failure("file_information"));
    }
    // Hard-linked metadata could have a second alias outside this profile.
    if info.nNumberOfLinks > 1 {
        return Err(ProfileError::UnsafeObject);
    }
    Ok(info.dwFileAttributes)
}

fn validate_file(file: &File, user: &CurrentUser) -> Result<(), ProfileError> {
    if attributes(file)? & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
        return Err(ProfileError::UnsafeObject);
    }
    validate_permissions(file, user, false)
}

fn validate_permissions(
    file: &File,
    user: &CurrentUser,
    directory: bool,
) -> Result<(), ProfileError> {
    let mut descriptor = ptr::null_mut();
    let mut owner = ptr::null_mut();
    let mut dacl = ptr::null_mut();
    let result = unsafe {
        GetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            ptr::null_mut(),
            &mut dacl,
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    if result != 0 {
        return Err(ProfileError::Platform {
            operation: "read_permissions",
            code: result,
        });
    }
    let _descriptor = LocalAllocation(descriptor);
    let mut control = 0;
    let mut revision = 0;
    if owner.is_null()
        || dacl.is_null()
        || unsafe { EqualSid(owner, user.sid()) } == 0
        || unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) } == 0
        || directory && control & SE_DACL_PROTECTED == 0
        || unsafe { (*dacl).AceCount } != 1
    {
        return Err(ProfileError::UnsafePermissions);
    }
    let mut ace = ptr::null_mut();
    if unsafe { GetAce(dacl, 0, &mut ace) } == 0 || ace.is_null() {
        return Err(ProfileError::UnsafePermissions);
    }
    let header = unsafe { &*ace.cast::<ACE_HEADER>() };
    // Type 0 is ACCESS_ALLOWED_ACE_TYPE. Reject denies, object/callback ACEs,
    // inherit-only entries and every additional principal/grant.
    let expected_flags = if directory {
        (OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE) as u8
    } else {
        INHERITED_ACE as u8
    };
    if header.AceType != 0
        || header.AceFlags != expected_flags
        || (header.AceSize as usize) < size_of::<ACCESS_ALLOWED_ACE>()
    {
        return Err(ProfileError::UnsafePermissions);
    }
    let allowed = unsafe { &*ace.cast::<ACCESS_ALLOWED_ACE>() };
    if allowed.Mask != FILE_ALL_ACCESS
        || unsafe {
            EqualSid(
                ptr::addr_of!(allowed.SidStart).cast_mut().cast(),
                user.sid(),
            )
        } == 0
    {
        return Err(ProfileError::UnsafePermissions);
    }
    Ok(())
}
