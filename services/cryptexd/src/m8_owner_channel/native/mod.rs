//! Private Mach transport used by tests and the default-off signed candidate.
//! Raw bytes alone do not construct an approval grant or domain authority.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::process::Child;
use std::ptr::{self, NonNull};
use std::time::Instant;

type Epoch = [u8; 16];

pub(super) struct ImagePolicy<'a> {
    requirement: &'a CStr,
    cdhash: &'a [u8],
    signer_der: Option<&'a [u8]>,
}

#[repr(C)]
struct PolicyRef {
    requirement: *const c_char,
    cdhash: *const u8,
    hash_size: usize,
    signer_der: *const u8,
    signer_size: usize,
}

impl ImagePolicy<'_> {
    fn as_native(&self) -> PolicyRef {
        PolicyRef {
            requirement: self.requirement.as_ptr(),
            cdhash: self.cdhash.as_ptr(),
            hash_size: self.cdhash.len(),
            signer_der: self.signer_der.map_or(ptr::null(), |der| der.as_ptr()),
            signer_size: self.signer_der.map_or(0, <[u8]>::len),
        }
    }
}

impl<'a> From<&'a super::image_policy::ExpectedImage> for ImagePolicy<'a> {
    fn from(image: &'a super::image_policy::ExpectedImage) -> Self {
        Self {
            requirement: image.requirement(),
            cdhash: image.cdhash(),
            signer_der: image.signer_der(),
        }
    }
}

unsafe extern "C" {
    fn cx_listen(out: *mut *mut c_void, epoch: *mut u8) -> c_int;
    fn cx_accept(
        channel: *mut c_void,
        child: libc::pid_t,
        policy: &PolicyRef,
        deadline: u64,
    ) -> c_int;
    fn cx_connect(
        out: *mut *mut c_void,
        epoch: *const u8,
        policy: &PolicyRef,
        deadline: u64,
    ) -> c_int;
    fn cx_now_ms() -> u64;
    fn cx_send(channel: *mut c_void, data: *const u8, size: usize, deadline: u64) -> c_int;
    fn cx_receive(
        channel: *mut c_void,
        data: *mut u8,
        capacity: usize,
        size: &mut usize,
        deadline: u64,
    ) -> c_int;
    fn cx_close(channel: *mut c_void) -> c_int;
    #[cfg(feature = "m8-local-startup-check")]
    fn cx_peer_liveness(channel: *mut c_void, out: *mut *mut c_void) -> c_int;
    #[cfg(feature = "m8-local-startup-check")]
    fn cx_peer_still_live(pin: *const c_void) -> c_int;
    #[cfg(feature = "m8-local-startup-check")]
    fn cx_liveness_close(pin: *mut c_void) -> c_int;
}

fn native_deadline(deadline: Instant) -> u64 {
    // Sample the native clock first, then round the caller's remaining budget
    // down. A scheduling delay during translation can only shorten the budget.
    let base = unsafe { cx_now_ms() };
    let remaining = deadline
        .saturating_duration_since(Instant::now())
        .as_millis();
    u32::try_from(remaining)
        .ok()
        .and_then(|ms| base.checked_add(u64::from(ms)))
        .unwrap_or(0)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
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
    Cleanup,
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
        12 => Error::Cleanup,
        other => Error::Unknown(other),
    })
}

struct Handle(NonNull<c_void>);

impl Handle {
    fn close(self) -> Result<(), Error> {
        let owned = std::mem::ManuallyDrop::new(self);
        // SAFETY: this consumes the sole handle, including on release failure.
        checked(unsafe { cx_close(owned.0.as_ptr()) })
    }

    fn open(f: impl FnOnce(*mut *mut c_void) -> c_int) -> Result<Self, Error> {
        let mut raw = ptr::null_mut();
        checked(f(&mut raw))?;
        NonNull::new(raw).map(Self).ok_or(Error::Resource)
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the successful constructor transferred sole ownership.
        if unsafe { cx_close(self.0.as_ptr()) } != 0 {
            // Drop cannot return a typed error. Emit a fixed diagnostic without
            // disclosing payloads, process identifiers, or session material.
            use std::io::Write;
            let _ = writeln!(std::io::stderr().lock(), "mac_owner_channel_cleanup_failed");
        }
    }
}

pub(super) struct Listener(Handle);
pub(super) struct Channel(Handle);

#[cfg(feature = "m8-local-startup-check")]
pub(super) struct PeerLiveness(NonNull<c_void>);
#[cfg(feature = "m8-local-startup-check")]
impl PeerLiveness {
    pub(super) fn live(&self) -> bool {
        // SAFETY: immutable read-only native allocation retained by this owner.
        unsafe { cx_peer_still_live(self.0.as_ptr()) != 0 }
    }
}
#[cfg(feature = "m8-local-startup-check")]
impl Drop for PeerLiveness {
    fn drop(&mut self) {
        // SAFETY: the unique allocation is released once after all local Rc pins.
        if unsafe { cx_liveness_close(self.0.as_ptr()) } != 0 {
            eprintln!("mac_peer_liveness_cleanup_failed");
        }
    }
}

impl Listener {
    pub(super) fn open() -> Result<(Self, Epoch), Error> {
        let mut epoch = [0; 16];
        // SAFETY: the output slots are valid for the entire native call.
        let listener = Handle::open(|out| unsafe { cx_listen(out, epoch.as_mut_ptr()) })?;
        Ok((Self(listener), epoch))
    }

    pub(super) fn accept(
        self,
        child: &mut Child,
        policy: &ImagePolicy<'_>,
    ) -> Result<Channel, Error> {
        self.accept_until(
            child,
            policy,
            Instant::now() + std::time::Duration::from_millis(2500),
        )
    }

    pub(super) fn accept_until(
        self,
        child: &mut Child,
        policy: &ImagePolicy<'_>,
        deadline: Instant,
    ) -> Result<Channel, Error> {
        if child.try_wait().map_err(|_| Error::Instance)?.is_some() {
            return Err(Error::Instance);
        }
        let pid = i32::try_from(child.id()).map_err(|_| Error::Instance)?;
        // SAFETY: the listener is exclusively owned, the policy lives through
        // configuration, and the child handle is still supervised and unreaped.
        checked(unsafe {
            cx_accept(
                self.0.0.as_ptr(),
                pid,
                &policy.as_native(),
                native_deadline(deadline),
            )
        })?;
        Ok(Channel(self.0))
    }
}

impl Channel {
    #[cfg(feature = "m8-local-startup-check")]
    pub(super) fn peer_liveness(&self) -> Result<PeerLiveness, Error> {
        let mut out = ptr::null_mut();
        // SAFETY: only an already authenticated channel can duplicate its peer pin.
        checked(unsafe { cx_peer_liveness(self.0.0.as_ptr(), &mut out) })?;
        Ok(PeerLiveness(NonNull::new(out).ok_or(Error::Resource)?))
    }
    pub(super) fn close(self) -> Result<(), Error> {
        self.0.close()
    }

    pub(super) fn connect(epoch: &Epoch, policy: &ImagePolicy<'_>) -> Result<Self, Error> {
        Self::connect_until(
            epoch,
            policy,
            Instant::now() + std::time::Duration::from_millis(2500),
        )
    }

    pub(super) fn connect_until(
        epoch: &Epoch,
        policy: &ImagePolicy<'_>,
        deadline: Instant,
    ) -> Result<Self, Error> {
        // SAFETY: native code copies the epoch/policy and selects getppid itself.
        Handle::open(|out| unsafe {
            cx_connect(
                out,
                epoch.as_ptr(),
                &policy.as_native(),
                native_deadline(deadline),
            )
        })
        .map(Self)
    }

    pub(super) fn send(&mut self, bytes: &[u8], deadline: Instant) -> Result<(), Error> {
        // SAFETY: synchronous bounded copy of a borrowed slice.
        checked(unsafe {
            cx_send(
                self.0.0.as_ptr(),
                bytes.as_ptr(),
                bytes.len(),
                native_deadline(deadline),
            )
        })
    }

    pub(super) fn receive<'a>(
        &mut self,
        bytes: &'a mut [u8],
        deadline: Instant,
    ) -> Result<&'a [u8], Error> {
        let mut size = 0;
        // SAFETY: the decoder receives the exact capacity and only returns a
        // body after validating its framing, session and live kernel sender.
        checked(unsafe {
            cx_receive(
                self.0.0.as_ptr(),
                bytes.as_mut_ptr(),
                bytes.len(),
                &mut size,
                native_deadline(deadline),
            )
        })?;
        bytes.get(..size).ok_or(Error::Layout)
    }
}

#[cfg(test)]
mod tests;
