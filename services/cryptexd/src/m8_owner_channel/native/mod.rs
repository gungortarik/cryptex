//! Private Mach transport. Only tests can currently supply an image policy.
//! Bytes received here do not construct an approval grant or domain authority.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::process::Child;
use std::ptr::{self, NonNull};

type Epoch = [u8; 16];

struct ImagePolicy<'a> {
    requirement: &'a CStr,
    cdhash: &'a [u8],
}

#[repr(C)]
struct PolicyRef {
    requirement: *const c_char,
    cdhash: *const u8,
    hash_size: usize,
}

impl ImagePolicy<'_> {
    fn as_native(&self) -> PolicyRef {
        PolicyRef {
            requirement: self.requirement.as_ptr(),
            cdhash: self.cdhash.as_ptr(),
            hash_size: self.cdhash.len(),
        }
    }
}

unsafe extern "C" {
    fn cx_listen(out: *mut *mut c_void, epoch: *mut u8) -> c_int;
    fn cx_accept(channel: *mut c_void, child: libc::pid_t, policy: &PolicyRef) -> c_int;
    fn cx_connect(out: *mut *mut c_void, epoch: *const u8, policy: &PolicyRef) -> c_int;
    fn cx_send(channel: *mut c_void, data: *const u8, size: usize) -> c_int;
    fn cx_receive(channel: *mut c_void, data: *mut u8, capacity: usize, size: &mut usize) -> c_int;
    fn cx_close(channel: *mut c_void);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Error {
    Argument,
    Resource,
    Rendezvous,
    Deadline,
    Transport,
    Layout,
    Epoch,
    Sequence,
    Instance,
    Image,
    Closed,
    Unknown(c_int),
}

fn checked(code: c_int) -> Result<(), Error> {
    Err(match code {
        0 => return Ok(()),
        1 => Error::Argument,
        2 => Error::Resource,
        3 => Error::Rendezvous,
        4 => Error::Deadline,
        5 => Error::Transport,
        6 => Error::Layout,
        7 => Error::Epoch,
        8 => Error::Sequence,
        9 => Error::Instance,
        10 => Error::Image,
        11 => Error::Closed,
        other => Error::Unknown(other),
    })
}

struct Handle(NonNull<c_void>);

impl Handle {
    fn open(f: impl FnOnce(*mut *mut c_void) -> c_int) -> Result<Self, Error> {
        let mut raw = ptr::null_mut();
        checked(f(&mut raw))?;
        NonNull::new(raw).map(Self).ok_or(Error::Resource)
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the successful constructor transferred sole ownership.
        unsafe { cx_close(self.0.as_ptr()) };
    }
}

struct Listener(Handle);
struct Channel(Handle);

impl Listener {
    fn open() -> Result<(Self, Epoch), Error> {
        let mut epoch = [0; 16];
        // SAFETY: the output slots are valid for the entire native call.
        let listener = Handle::open(|out| unsafe { cx_listen(out, epoch.as_mut_ptr()) })?;
        Ok((Self(listener), epoch))
    }

    fn accept(self, child: &mut Child, policy: &ImagePolicy<'_>) -> Result<Channel, Error> {
        if child.try_wait().map_err(|_| Error::Instance)?.is_some() {
            return Err(Error::Instance);
        }
        let pid = i32::try_from(child.id()).map_err(|_| Error::Instance)?;
        // SAFETY: the listener is exclusively owned, the policy lives through
        // configuration, and the child handle is still supervised and unreaped.
        checked(unsafe { cx_accept(self.0.0.as_ptr(), pid, &policy.as_native()) })?;
        Ok(Channel(self.0))
    }
}

impl Channel {
    fn connect(epoch: &Epoch, policy: &ImagePolicy<'_>) -> Result<Self, Error> {
        // SAFETY: native code copies the epoch/policy and selects getppid itself.
        Handle::open(|out| unsafe { cx_connect(out, epoch.as_ptr(), &policy.as_native()) })
            .map(Self)
    }

    fn send(&mut self, bytes: &[u8]) -> Result<(), Error> {
        // SAFETY: synchronous bounded copy of a borrowed slice.
        checked(unsafe { cx_send(self.0.0.as_ptr(), bytes.as_ptr(), bytes.len()) })
    }

    fn receive<'a>(&mut self, bytes: &'a mut [u8]) -> Result<&'a [u8], Error> {
        let mut size = 0;
        // SAFETY: the decoder receives the exact capacity and only returns a
        // body after validating its framing, session and live kernel sender.
        checked(unsafe {
            cx_receive(
                self.0.0.as_ptr(),
                bytes.as_mut_ptr(),
                bytes.len(),
                &mut size,
            )
        })?;
        bytes.get(..size).ok_or(Error::Layout)
    }
}

#[cfg(test)]
mod tests;
