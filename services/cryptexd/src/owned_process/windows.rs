//! Windows 10+ atomic job assignment. Windows 11 is the current test target.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::JobObjects::{
    CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectBasicAccountingInformation,
    JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
    TerminateJobObject,
};
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_UNICODE_ENVIRONMENT, CreateProcessW, DeleteProcThreadAttributeList,
    EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess, InitializeProcThreadAttributeList,
    LPPROC_THREAD_ATTRIBUTE_LIST, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
    PROC_THREAD_ATTRIBUTE_JOB_LIST, PROCESS_INFORMATION, STARTF_USESTDHANDLES, STARTUPINFOEXW,
    UpdateProcThreadAttribute, WaitForSingleObject,
};

use super::{LaunchSpec, OwnedProcessError};

#[path = "windows_endpoint.rs"]
mod connection;
pub(super) use connection::{PeerHandle, connected_server_pid};

fn failure(operation: &'static str) -> OwnedProcessError {
    OwnedProcessError::Platform {
        operation,
        code: i64::from(unsafe { GetLastError() }),
    }
}

struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: every Handle here owns a non-null handle returned by Win32;
        // it is never inherited, duplicated or transferred to another owner.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

struct Attributes {
    storage: Vec<usize>,
    initialized: bool,
}

impl Attributes {
    fn pointer(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.storage.as_mut_ptr().cast()
    }

    fn new(job: &Handle, stdio: &Handle) -> Result<Self, OwnedProcessError> {
        let mut bytes = 0;
        // Size discovery deliberately fails with insufficient buffer.
        unsafe {
            InitializeProcThreadAttributeList(ptr::null_mut(), 2, 0, &mut bytes);
        }
        if bytes == 0 || bytes > 64 * 1024 {
            return Err(failure("attribute_size"));
        }
        let mut attributes = Self {
            storage: vec![0; bytes.div_ceil(size_of::<usize>())],
            initialized: false,
        };
        // SAFETY: aligned storage is at least the requested opaque buffer size.
        if unsafe { InitializeProcThreadAttributeList(attributes.pointer(), 2, 0, &mut bytes) } == 0
        {
            return Err(failure("attribute_init"));
        }
        attributes.initialized = true;
        // lpValue must live until DeleteProcThreadAttributeList. The owning job
        // handle is allocated by spawn, and outlives this temporary list.
        if unsafe {
            UpdateProcThreadAttribute(
                attributes.pointer(),
                0,
                PROC_THREAD_ATTRIBUTE_JOB_LIST as usize,
                (&job.0 as *const HANDLE).cast(),
                size_of::<HANDLE>(),
                ptr::null_mut(),
                ptr::null(),
            )
        } == 0
        {
            return Err(failure("attribute_job"));
        }
        if unsafe {
            UpdateProcThreadAttribute(
                attributes.pointer(),
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                (&stdio.0 as *const HANDLE).cast(),
                size_of::<HANDLE>(),
                ptr::null_mut(),
                ptr::null(),
            )
        } == 0
        {
            return Err(failure("attribute_handles"));
        }
        Ok(attributes)
    }
}

impl Drop for Attributes {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                DeleteProcThreadAttributeList(self.pointer());
            }
        }
    }
}

pub(super) struct NativeProcess {
    // Drop the job first: KILL_ON_JOB_CLOSE cleans up a still-running owned tree.
    job: Handle,
    process: Handle,
    pid: u32,
}

impl NativeProcess {
    pub(super) fn spawn(spec: &LaunchSpec) -> Result<Self, OwnedProcessError> {
        let application = terminated(spec.program.as_os_str())?;
        let directory = terminated(spec.working_directory.as_os_str())?;
        if !native_executable(&spec.program) {
            return Err(OwnedProcessError::InvalidLaunch);
        }
        let mut command = quote(spec.program.as_os_str())?;
        for argument in &spec.arguments {
            command.push(b' ' as u16);
            command.extend(quote(argument)?);
        }
        if command.len() >= 32767 {
            return Err(OwnedProcessError::InvalidLaunch);
        }
        command.push(0);
        let mut environment = environment_block(spec)?;

        let job = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if job.is_null() {
            return Err(failure("job_create"));
        }
        let job = Handle(job);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        } == 0
        {
            return Err(failure("job_limits"));
        }
        let null_name = terminated(OsStr::new("NUL"))?;
        let security = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: ptr::null_mut(),
            bInheritHandle: 1,
        };
        let stdio = unsafe {
            CreateFileW(
                null_name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                &security,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                ptr::null_mut(),
            )
        };
        if stdio == INVALID_HANDLE_VALUE {
            return Err(failure("null_stdio"));
        }
        let stdio = Handle(stdio);
        let mut attributes = Attributes::new(&job, &stdio)?;
        let mut startup = STARTUPINFOEXW::default();
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        // Only NUL stdio can be inherited, no console. A future transport
        // controller must put its private native log under the protected profile.
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = stdio.0;
        startup.StartupInfo.hStdOutput = stdio.0;
        startup.StartupInfo.hStdError = stdio.0;
        startup.lpAttributeList = attributes.pointer();
        let mut process = PROCESS_INFORMATION::default();
        // SAFETY: explicit executable and mutable terminated command/environment
        // buffers live throughout the call. Job assignment is atomic at create;
        // there is no suspended-but-unowned child or post-spawn adoption fallback.
        if unsafe {
            CreateProcessW(
                application.as_ptr(),
                command.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                1,
                EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT | CREATE_NO_WINDOW,
                environment.as_mut_ptr().cast(),
                directory.as_ptr(),
                &startup.StartupInfo,
                &mut process,
            )
        } == 0
        {
            return Err(failure("process_create"));
        }
        let _thread = Handle(process.hThread);
        drop(attributes);
        Ok(Self {
            job,
            process: Handle(process.hProcess),
            pid: process.dwProcessId,
        })
    }

    pub(super) fn pid(&self) -> u32 {
        self.pid
    }

    pub(super) fn try_exit(&mut self) -> Result<Option<u32>, OwnedProcessError> {
        match unsafe { WaitForSingleObject(self.process.0, 0) } {
            WAIT_TIMEOUT => return Ok(None),
            WAIT_OBJECT_0 => {}
            _ => return Err(failure("process_wait")),
        }
        let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        if unsafe {
            QueryInformationJobObject(
                self.job.0,
                JobObjectBasicAccountingInformation,
                (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(failure("job_observe"));
        }
        if accounting.ActiveProcesses != 0 {
            return Ok(None);
        }
        let mut code = 0;
        if unsafe { GetExitCodeProcess(self.process.0, &mut code) } == 0 {
            return Err(failure("process_exit"));
        }
        Ok(Some(code))
    }

    pub(super) fn terminate_job(&self) -> Result<(), OwnedProcessError> {
        if unsafe { TerminateJobObject(self.job.0, 1) } == 0 {
            return Err(failure("job_terminate"));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn terminate_root_only_for_test(&self) -> Result<(), OwnedProcessError> {
        // Test abrupt parent death without terminating its containing job. This
        // targets only our retained owned process handle, never an arbitrary PID.
        if unsafe { windows_sys::Win32::System::Threading::TerminateProcess(self.process.0, 97) }
            == 0
        {
            return Err(failure("test_root_terminate"));
        }
        Ok(())
    }
}

fn native_executable(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
}

fn terminated(value: &OsStr) -> Result<Vec<u16>, OwnedProcessError> {
    let mut wide: Vec<_> = value.encode_wide().take(32768).collect();
    if wide.contains(&0) || wide.len() >= 32767 {
        return Err(OwnedProcessError::InvalidLaunch);
    }
    wide.push(0);
    Ok(wide)
}

fn quote(value: &OsStr) -> Result<Vec<u16>, OwnedProcessError> {
    let raw: Vec<_> = value.encode_wide().take(32768).collect();
    if raw.contains(&0) || raw.len() >= 32767 {
        return Err(OwnedProcessError::InvalidLaunch);
    }
    // Always quote; preserve non-Unicode Windows path/argument units exactly.
    let mut out = vec![b'"' as u16];
    let mut slashes = 0;
    for unit in raw {
        if unit == b'\\' as u16 {
            slashes += 1;
            continue;
        }
        out.extend(std::iter::repeat_n(
            b'\\' as u16,
            if unit == b'"' as u16 {
                slashes * 2 + 1
            } else {
                slashes
            },
        ));
        out.push(unit);
        slashes = 0;
    }
    out.extend(std::iter::repeat_n(b'\\' as u16, slashes * 2));
    out.push(b'"' as u16);
    Ok(out)
}

fn environment_block(spec: &LaunchSpec) -> Result<Vec<u16>, OwnedProcessError> {
    let mut entries = spec.environment.clone();
    // All selected names are ASCII. Reject duplicates instead of relying on
    // Windows' case-insensitive first/last value behavior.
    entries.sort_by_key(|(name, _)| name.to_string_lossy().to_ascii_uppercase());
    for pair in entries.windows(2) {
        if pair[0]
            .0
            .to_string_lossy()
            .eq_ignore_ascii_case(&pair[1].0.to_string_lossy())
        {
            return Err(OwnedProcessError::InvalidLaunch);
        }
    }
    let mut block = Vec::new();
    for (name, value) in entries {
        block.extend(terminated(&name)?.into_iter().take_while(|unit| *unit != 0));
        block.push(b'=' as u16);
        block.extend(terminated(&value)?);
        if block.len() >= 32767 {
            return Err(OwnedProcessError::InvalidLaunch);
        }
    }
    if block.is_empty() {
        block.push(0);
    }
    block.push(0);
    Ok(block)
}
