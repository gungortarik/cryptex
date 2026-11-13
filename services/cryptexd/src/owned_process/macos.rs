//! Engine owner used only inside the authenticated, default-off transport helper.
//! The helper is the private session/group leader; its parent retains it unreaped.
use super::*;
use std::ffi::{CString, c_int, c_void};
use std::marker::PhantomData;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;
use std::ptr::NonNull;
use std::rc::Rc;

unsafe extern "C" {
    fn cx_transport_spawn_suspended(
        path: *const libc::c_char,
        cwd: *const libc::c_char,
        argv: *const *const libc::c_char,
        envp: *const *const libc::c_char,
        out: *mut libc::pid_t,
    ) -> c_int;
    fn cx_transport_image_hash(path: *const libc::c_char, hash: *mut u8) -> c_int;
    fn cx_transport_retain_child(pid: libc::pid_t, hash: *const u8, out: *mut *mut c_void)
    -> c_int;
    fn cx_transport_pin_live(pin: *const c_void) -> c_int;
    fn cx_transport_pin_close(pin: *mut c_void) -> c_int;
    fn cx_transport_socket_peer(root: *const c_void, fd: c_int, out: *mut *mut c_void) -> c_int;
    fn cx_transport_peer_validate(root: *const c_void, peer: *const c_void, socket: c_int) -> c_int;
    fn cx_transport_group_empty(
        group: libc::pid_t,
        except: libc::pid_t,
        exited_child: libc::pid_t,
    ) -> c_int;
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
    child: Option<CapturedChild>,
    pin: PeerPin,
    pid: u32,
    exit: Option<i64>,
}

struct CapturedChild {
    pid: libc::pid_t,
    reaped: bool,
}
impl CapturedChild {
    fn spawn(spec: &LaunchSpec) -> Result<Self, OwnedProcessError> {
        let string =
            |bytes: &[u8]| CString::new(bytes).map_err(|_| OwnedProcessError::InvalidLaunch);
        let path = string(spec.program.as_os_str().as_bytes())?;
        let cwd = string(spec.working_directory.as_os_str().as_bytes())?;
        let mut args = vec![path.clone()];
        for arg in &spec.arguments {
            args.push(string(arg.as_os_str().as_bytes())?);
        }
        let mut env = Vec::new();
        for (key, value) in &spec.environment {
            let mut bytes = key.as_os_str().as_bytes().to_vec();
            bytes.push(b'=');
            bytes.extend_from_slice(value.as_os_str().as_bytes());
            env.push(string(&bytes)?);
        }
        let pointers = |values: &[CString]| {
            values
                .iter()
                .map(|value| value.as_ptr())
                .chain(std::iter::once(std::ptr::null()))
                .collect::<Vec<_>>()
        };
        let argv = pointers(&args);
        let envp = pointers(&env);
        let mut pid = 0;
        let code = unsafe {
            cx_transport_spawn_suspended(
                path.as_ptr(),
                cwd.as_ptr(),
                argv.as_ptr(),
                envp.as_ptr(),
                &mut pid,
            )
        };
        if code != 0 {
            if pid > 1 {
                let mut child = Self { pid, reaped: false };
                if child.kill().and_then(|_| child.wait()).is_err() {
                    eprintln!("mac_engine_partial_spawn_cleanup_failed");
                    kill_helper_group();
                }
            }
            return Err(OwnedProcessError::Platform {
                operation: "mac_engine_suspended_spawn",
                code: i64::from(code),
            });
        }
        if pid <= 1 {
            eprintln!("mac_engine_spawn_returned_invalid_child");
            kill_helper_group();
        }
        Ok(Self { pid, reaped: false })
    }
    fn id(&self) -> u32 {
        self.pid as u32
    }
    fn kill(&mut self) -> std::io::Result<()> {
        if unsafe { libc::kill(self.pid, libc::SIGKILL) } == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
    fn wait(&mut self) -> std::io::Result<ExitStatus> {
        loop {
            let mut status = 0;
            let result = unsafe { libc::waitpid(self.pid, &mut status, 0) };
            if result == self.pid {
                self.reaped = true;
                return Ok(ExitStatus::from_raw(status));
            }
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
}
impl Drop for CapturedChild {
    fn drop(&mut self) {
        if !self.reaped && self.kill().and_then(|_| self.wait()).is_err() {
            eprintln!("mac_engine_captured_child_cleanup_failed");
            kill_helper_group();
        }
    }
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
        let mut child = CapturedChild::spawn(spec)?;
        let mut raw = std::ptr::null_mut();
        // SAFETY: child is captured by this spawn and is still unreaped. C also
        // checks direct parent, private group, live task generation and image.
        let code = unsafe { cx_transport_retain_child(child.id() as i32, hash.as_ptr(), &mut raw) };
        if code != 0 {
            eprintln!("component=mac_transport operation=engine_pin status=refused code={code}");
            let _ = child.kill();
            let _ = child.wait();
            // The new image is still stopped. Its helper parent remains the
            // unreaped group leader until the daemon verifies group exit.
            kill_helper_group();
        }
        let raw = NonNull::new(raw).ok_or(OwnedProcessError::InvalidLaunch)?;
        // Even an instantaneous `generate` cannot exit before image capture.
        // The stopped image must match the independent package hash first.
        if !crate::m8_owner_channel::transport_helper::role_active()
            || unsafe { libc::kill(child.pid, libc::SIGCONT) } != 0
        {
            eprintln!("mac_engine_validated_resume_failed");
            kill_helper_group();
        }
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
            4 => Err(endpoint::EndpointError::PeerChanged),
            _ => Err(endpoint::EndpointError::OwnerExited),
        }
    }
    pub(super) fn validate_peer(
        &self,
        peer: &PeerPin,
        require_socket: bool,
    ) -> Result<(), endpoint::EndpointError> {
        self.require_root_alive()?;
        // Both retained task rights stay live. Native validation brackets the
        // original FD/object witness with generation and expected-image checks.
        match unsafe {
            cx_transport_peer_validate(
                self.pin.raw.as_ptr(),
                peer.raw.as_ptr(),
                i32::from(require_socket),
            )
        } {
            0 => Ok(()),
            1 => Err(endpoint::EndpointError::PeerNotOwned),
            2 => Err(endpoint::EndpointError::TableUnavailable),
            _ => Err(endpoint::EndpointError::OwnerExited),
        }
    }
    pub(super) fn try_exit(&mut self) -> Result<Option<i64>, OwnedProcessError> {
        if !crate::m8_owner_channel::transport_helper::role_active() {
            eprintln!("mac_engine_parent_generation_lost");
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
        match unsafe {
            cx_transport_group_empty(libc::getpgrp(), libc::getpid(), child.id() as i32)
        } {
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
        eprintln!("mac_engine_forced_group_cleanup");
        kill_helper_group()
    }
}
impl Drop for NativeProcess {
    fn drop(&mut self) {
        if self.child.is_some() {
            eprintln!("mac_engine_drop_requires_group_cleanup");
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

#[cfg(test)]
mod tests {
    #[test]
    fn engine_environment_has_only_private_home_and_no_monitor_or_loader_override() {
        let root = std::path::Path::new("/private/tmp/isolated-native-home");
        let environment = super::super::syncthing_environment(root);
        for name in ["HOME", "TMPDIR", "STHOMEDIR"] {
            assert_eq!(
                environment
                    .iter()
                    .find(|(key, _)| key == name)
                    .map(|(_, value)| value.as_os_str()),
                Some(root.as_os_str())
            );
        }
        for (name, _) in environment {
            let name = name.to_str().unwrap();
            assert!(!name.starts_with("DYLD_"));
            assert!(!matches!(
                name,
                "STMONITORED" | "STRESTART" | "PATH" | "HTTP_PROXY" | "HTTPS_PROXY"
            ));
        }
    }
}
