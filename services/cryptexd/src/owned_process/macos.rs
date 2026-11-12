//! Engine owner used only inside the authenticated, default-off transport helper.
//! The helper is the private session/group leader; its parent retains it unreaped.
use super::*;
use std::ffi::{CString, c_int, c_void};
use std::marker::PhantomData;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::process::{Child, Command, Stdio};
use std::ptr::NonNull;
use std::rc::Rc;

unsafe extern "C" {
    fn cx_transport_image_hash(path: *const libc::c_char, hash: *mut u8) -> c_int;
    fn cx_transport_retain_child(pid: libc::pid_t, hash: *const u8, out: *mut *mut c_void)
    -> c_int;
    fn cx_transport_pin_live(pin: *const c_void) -> c_int;
    fn cx_transport_pin_close(pin: *mut c_void) -> c_int;
    fn cx_transport_socket_peer(root: *const c_void, fd: c_int, out: *mut *mut c_void) -> c_int;
    fn cx_transport_group_empty(group: libc::pid_t, except: libc::pid_t) -> c_int;
    fn cx_transport_private_acl(fd: c_int) -> c_int;
}

pub(crate) fn private_acl(file: &std::fs::File) -> bool {
    unsafe { cx_transport_private_acl(file.as_raw_fd()) == 0 }
}

fn failure(operation: &'static str) -> OwnedProcessError {
    OwnedProcessError::Platform {
        operation,
        code: i64::from(std::io::Error::last_os_error().raw_os_error().unwrap_or(0)),
    }
}
pub(super) struct PeerPin {
    raw: NonNull<c_void>,
    _thread: PhantomData<Rc<()>>,
}
impl PeerPin {
    fn live(&self) -> bool {
        // SAFETY: the unique owner keeps this native allocation alive on one thread.
        unsafe { cx_transport_pin_live(self.raw.as_ptr()) != 0 }
    }
}
impl Drop for PeerPin {
    fn drop(&mut self) {
        // SAFETY: consumed once, with no aliases which can release the allocation.
        if unsafe { cx_transport_pin_close(self.raw.as_ptr()) } != 0 {
            eprintln!("mac_transport_task_cleanup_failed");
        }
    }
}
pub(super) struct NativeProcess {
    child: Option<Child>,
    pin: PeerPin,
    pid: u32,
    exit: Option<i64>,
}
impl NativeProcess {
    pub(super) fn spawn(spec: &LaunchSpec) -> Result<Self, OwnedProcessError> {
        if !crate::m8_owner_channel::transport_helper::role_active()
            || unsafe { libc::getpgrp() != libc::getpid() || libc::getsid(0) != libc::getpid() }
        {
            return Err(OwnedProcessError::InvalidLaunch);
        }
        let path = CString::new(spec.program.as_os_str().as_bytes())
            .map_err(|_| OwnedProcessError::InvalidLaunch)?;
        let mut hash = [0; 20];
        // Package SHA pins and private paths are validated by PinnedSyncthingPackage.
        // This observation adds the independently captured native executable identity.
        if unsafe { cx_transport_image_hash(path.as_ptr(), hash.as_mut_ptr()) } != 0 {
            return Err(OwnedProcessError::InvalidLaunch);
        }
        if hash != crate::syncthing_package::macos::CDHASH {
            return Err(OwnedProcessError::InvalidLaunch);
        }
        let mut child = Command::new(&spec.program)
            .args(&spec.arguments)
            .current_dir(&spec.working_directory)
            .env_clear()
            .envs(spec.environment.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| failure("mac_engine_spawn"))?;
        let mut raw = std::ptr::null_mut();
        // SAFETY: child is captured by this spawn and is still unreaped. C also
        // checks direct parent, private group, live task generation and image.
        if unsafe { cx_transport_retain_child(child.id() as i32, hash.as_ptr(), &mut raw) } != 0 {
            let _ = child.kill();
            let _ = child.wait();
            // Root could have forked before a late validation failure. The parent
            // owns this helper's unreaped group leader and will verify group exit.
            kill_helper_group();
        }
        let raw = NonNull::new(raw).ok_or(OwnedProcessError::InvalidLaunch)?;
        Ok(Self {
            pid: child.id(),
            child: Some(child),
            exit: None,
            pin: PeerPin {
                raw,
                _thread: PhantomData,
            },
        })
    }
    pub(super) fn pid(&self) -> u32 {
        self.pid
    }
    pub(super) fn require_root_alive(&self) -> Result<(), endpoint::EndpointError> {
        if crate::m8_owner_channel::transport_helper::role_active()
            && self.child.is_some()
            && self.pin.live()
        {
            Ok(())
        } else {
            Err(endpoint::EndpointError::OwnerExited)
        }
    }
    pub(super) fn retain_socket_peer(
        &self,
        stream: &std::net::TcpStream,
    ) -> Result<PeerPin, endpoint::EndpointError> {
        self.require_root_alive()?;
        let mut raw = std::ptr::null_mut();
        // SAFETY: borrowed live root and the actual newly connected socket. The
        // C adapter enumerates only our private session/group, never a global PID.
        match unsafe {
            cx_transport_socket_peer(self.pin.raw.as_ptr(), stream.as_raw_fd(), &mut raw)
        } {
            0 => Ok(PeerPin {
                raw: NonNull::new(raw).ok_or(endpoint::EndpointError::Unavailable)?,
                _thread: PhantomData,
            }),
            1 => Err(endpoint::EndpointError::PeerNotOwned),
            2 => Err(endpoint::EndpointError::TableUnavailable),
            _ => Err(endpoint::EndpointError::OwnerExited),
        }
    }
    pub(super) fn try_exit(&mut self) -> Result<Option<i64>, OwnedProcessError> {
        if !crate::m8_owner_channel::transport_helper::role_active() {
            kill_helper_group();
        }
        let Some(child) = self.child.as_mut() else {
            return Ok(self.exit);
        };
        let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
        // SAFETY: observed without reaping, so the PID cannot be reused while
        // checking the monitor's remaining children in the helper's private group.
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                child.id(),
                info.as_mut_ptr(),
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result != 0 {
            return Err(failure("mac_engine_wait"));
        }
        if unsafe { info.assume_init().si_pid() } == 0 {
            return Ok(None);
        }
        match unsafe { cx_transport_group_empty(libc::getpgrp(), libc::getpid()) } {
            0 => return Ok(None),
            1 => {}
            _ => return Err(failure("mac_engine_group")),
        }
        let status = child.wait().map_err(|_| failure("mac_engine_reap"))?;
        self.child.take();
        use std::os::unix::process::ExitStatusExt;
        self.exit = Some(
            status
                .code()
                .map(i64::from)
                .unwrap_or_else(|| -i64::from(status.signal().unwrap_or(1))),
        );
        Ok(self.exit)
    }
    pub(super) fn terminate_job(&self) -> Result<(), OwnedProcessError> {
        kill_helper_group()
    }
}
impl Drop for NativeProcess {
    fn drop(&mut self) {
        if self.child.is_some() {
            kill_helper_group();
        }
    }
}
pub(crate) fn kill_helper_group() -> ! {
    // Only reached in the authenticated helper's private session. This intentionally
    // kills the helper too; its supervising daemon owns verification/reaping.
    if !crate::m8_owner_channel::transport_helper::role_entered()
        || unsafe { libc::getpgrp() != libc::getpid() || libc::getsid(0) != libc::getpid() }
    {
        std::process::abort();
    }
    unsafe { libc::killpg(libc::getpid(), libc::SIGKILL) };
    std::process::abort()
}
