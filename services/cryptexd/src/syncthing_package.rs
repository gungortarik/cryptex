//! Backend-only Windows package boundary. Exact retained 2.1.5 bytes, not a
//! publisher-authentication claim or authority to enable transport/peer trust.
//!
//! The executable and notices are opened without write/delete sharing, after
//! top-down non-reparse ancestor pins. Hashing and CreateProcess use that same
//! pinned pathname. Pins outlive the entire owned job, including monitor children.
//! No production startup or IPC calls this module yet.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::owned_process::{ExitObservation, LaunchSpec, OwnedProcess, OwnedProcessError};

#[cfg(all(windows, target_arch = "x86_64"))]
#[path = "syncthing_package/windows.rs"]
mod windows;

pub const VERSION: &str = "2.1.5";
pub const WINDOWS_AMD64_SHA256: &str =
    "36a0f7bc372f64fa7cc4f5654fa324c0dd9f7fef2e07565e00c6e1cf73f50344";
pub const LICENSE_SHA256: &str = "3f3d9e0024b1921b067d6f7f88deb4a60cbe7a78e76c64e3f1d7fc3b779b9d04";
pub const AUTHORS_SHA256: &str = "5a0044d13ddf6f013bdd5c2bc419bf45d6123c356567510237e82f304d113d48";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageError {
    UnsupportedTarget,
    InvalidPath,
    UnsafeObject,
    SizeMismatch,
    DigestMismatch,
    Io,
    Launch(OwnedProcessError),
}

impl fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "syncthing_package_{self:?}")
    }
}
impl std::error::Error for PackageError {}

struct PackagePins {
    binary: PathBuf,
    #[cfg(windows)]
    _files: Vec<std::fs::File>,
}

/// Only the explicit, backend-selected package directory is accepted. There is
/// no PATH/system-install fallback, network download or alternate checksum.
pub struct PinnedSyncthingPackage(Arc<PackagePins>);

impl PinnedSyncthingPackage {
    pub fn open(directory: &Path) -> Result<Self, PackageError> {
        #[cfg(all(windows, target_arch = "x86_64"))]
        {
            let files = windows::open_pinned(directory)?;
            Ok(Self(Arc::new(PackagePins {
                binary: directory.join("syncthing.exe"),
                _files: files,
            })))
        }
        #[cfg(not(all(windows, target_arch = "x86_64")))]
        {
            let _ = directory;
            Err(PackageError::UnsupportedTarget)
        }
    }

    /// Package identity only. Caller must separately retain a protected profile
    /// session and validate isolation, credential binding and API identity.
    pub fn spawn_serve(&self, profile: &Path) -> Result<PackagedProcess, PackageError> {
        self.spawn(LaunchSpec::syncthing(&self.0.binary, profile))
    }

    /// Fixed offline native generation role; no serve command/network startup.
    /// Caller must require a fresh protected profile and provision/validate its
    /// generated configuration BEFORE any subsequent serve operation.
    pub fn spawn_generate(&self, profile: &Path) -> Result<PackagedProcess, PackageError> {
        self.spawn(LaunchSpec::syncthing_generate(&self.0.binary, profile))
    }

    fn spawn(
        &self,
        spec: Result<LaunchSpec, OwnedProcessError>,
    ) -> Result<PackagedProcess, PackageError> {
        let process = OwnedProcess::spawn(&spec.map_err(PackageError::Launch)?)
            .map_err(PackageError::Launch)?;
        Ok(PackagedProcess {
            process,
            _package: self.0.clone(),
        })
    }
}

impl fmt::Debug for PinnedSyncthingPackage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PinnedSyncthingPackage(<pinned Windows amd64 2.1.5>)")
    }
}

/// No owned-process extraction: package pins cannot be dropped while this
/// process owner remains usable. Fields drop in order: job first, package last.
pub struct PackagedProcess {
    process: OwnedProcess,
    _package: Arc<PackagePins>,
}

impl PackagedProcess {
    pub fn process(&self) -> &OwnedProcess {
        &self.process
    }

    pub fn try_exit(&mut self) -> Result<Option<ExitObservation>, OwnedProcessError> {
        self.process.try_exit()
    }

    pub fn wait_for_exit(
        &mut self,
        timeout: Duration,
    ) -> Result<ExitObservation, OwnedProcessError> {
        self.process.wait_for_exit(timeout)
    }

    pub fn force_cleanup(
        &mut self,
        timeout: Duration,
    ) -> Result<ExitObservation, OwnedProcessError> {
        self.process.force_cleanup(timeout)
    }
}

impl Drop for PackagedProcess {
    fn drop(&mut self) {
        if matches!(self.process.try_exit(), Ok(Some(_))) {
            return;
        }
        if let Err(error) = self.process.force_cleanup(Duration::from_secs(3)) {
            // Failure to observe job termination must not unpin its executable.
            // Keep one non-inherited pin set until daemon process exit. This is
            // a fail-closed resource leak on an exceptional OS cleanup failure,
            // never a clean stop or permission to launch a replacement.
            std::mem::forget(self._package.clone());
            eprintln!(
                "component=managed_transport operation=package_drop_cleanup status=failed pins=retained error={error}"
            );
        }
    }
}

impl fmt::Debug for PackagedProcess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PackagedProcess")
            .field("process", &self.process)
            .finish_non_exhaustive()
    }
}

#[cfg(all(test, windows, target_arch = "x86_64"))]
#[path = "syncthing_package/tests.rs"]
mod tests;
