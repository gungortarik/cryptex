//! Backend-only Windows package boundary. Exact retained upstream bytes, not a
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

#[cfg(all(target_os = "macos", feature = "m8-local-startup-check"))]
pub(crate) mod macos;
#[cfg(all(windows, target_arch = "x86_64"))]
#[path = "syncthing_package/windows.rs"]
mod windows;

#[cfg(feature = "m17-owner-transport")]
mod pins {
    pub const VERSION: &str = "2.1.7";
    pub const REST_VERSION: &str = "v2.1.7";
    pub const WINDOWS_AMD64_SHA256: &str =
        "cd9aea20afda2b9bdba5aff2e6ac964ed5c2e741ed4ceb48ad9e1be096ca7891";
    pub const LICENSE_SHA256: &str =
        "9221c2f936159b8446d329249fb4c0f25be510f447383a0f13336ac7985668a3";
    pub const AUTHORS_SHA256: &str =
        "2981b9b7604222e3042f748652654ece919ff059f5dba430520ea729c67adeb2";
    #[cfg(windows)]
    pub const SIZES: [u64; 3] = [34_802_944, 17_099, 17_249];
}
#[cfg(any(
    not(feature = "m17-owner-transport"),
    all(windows, target_arch = "x86_64")
))]
mod previous {
    #[cfg(not(feature = "m17-owner-transport"))]
    pub const REST_VERSION: &str = "v2.1.5";
    #[cfg(windows)]
    pub const SIZES: [u64; 3] = [27_448_104, 16_726, 16_811];
    #[cfg(not(feature = "m17-owner-transport"))]
    pub const VERSION: &str = "2.1.5";
    pub const WINDOWS_AMD64_SHA256: &str =
        "36a0f7bc372f64fa7cc4f5654fa324c0dd9f7fef2e07565e00c6e1cf73f50344";
    pub const LICENSE_SHA256: &str =
        "3f3d9e0024b1921b067d6f7f88deb4a60cbe7a78e76c64e3f1d7fc3b779b9d04";
    pub const AUTHORS_SHA256: &str =
        "5a0044d13ddf6f013bdd5c2bc419bf45d6123c356567510237e82f304d113d48";
}
pub use pins::{AUTHORS_SHA256, LICENSE_SHA256, REST_VERSION, VERSION, WINDOWS_AMD64_SHA256};
#[cfg(not(feature = "m17-owner-transport"))]
use previous as pins;

/// Recognize the exact retained package during a stopped update. Verification
/// does not grant the new release a launch handle for the previous executable.
#[cfg(all(
    feature = "m17-owner-transport",
    any(
        all(windows, target_arch = "x86_64"),
        all(target_os = "macos", feature = "m8-local-startup-check")
    )
))]
pub fn verify_previous_owner_package(directory: &Path) -> Result<(), PackageError> {
    pin_previous_owner_package(directory).map(drop)
}

#[cfg(all(
    feature = "m17-owner-transport",
    any(
        all(windows, target_arch = "x86_64"),
        all(target_os = "macos", feature = "m8-local-startup-check")
    )
))]
pub(crate) fn pin_previous_owner_package(
    directory: &Path,
) -> Result<Vec<std::fs::File>, PackageError> {
    #[cfg(windows)]
    let files = windows::open_previous(directory)?;
    #[cfg(target_os = "macos")]
    let files = macos::open_previous(directory)?;
    check_owner_package_entries(directory)?;
    Ok(files)
}

#[cfg(all(
    feature = "m17-owner-transport",
    any(
        all(windows, target_arch = "x86_64"),
        all(target_os = "macos", feature = "m8-local-startup-check")
    )
))]
fn check_owner_package_entries(path: &Path) -> Result<(), PackageError> {
    // Both pin readers hold the three expected regular files open during this check.
    let entries = std::fs::read_dir(path).map_err(|_| PackageError::Io)?;
    let mut count = 0;
    for entry in entries.take(4) {
        entry.map_err(|_| PackageError::Io)?;
        count += 1;
    }
    if count != 3 {
        return Err(PackageError::UnsafeObject);
    }
    Ok(())
}

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
    #[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
    _files: Vec<std::fs::File>,
}

/// Only the explicit, backend-selected package directory is accepted. There is
/// no PATH/system-install fallback, network download or alternate checksum.
pub struct PinnedSyncthingPackage(Arc<PackagePins>);

impl PinnedSyncthingPackage {
    /// Copy only the three already verified package handles.
    /// Destination is a newly created backend directory, never an overwrite.
    #[cfg(any(
        windows,
        all(
            target_os = "macos",
            feature = "m8-local-startup-check",
            feature = "m17-owner-transport"
        )
    ))]
    pub(crate) fn install_fresh(
        &self,
        directory: &crate::import_staging::export_fs::Directory,
    ) -> Result<(), PackageError> {
        use std::io::{Seek, SeekFrom};
        let files = &self.0._files;
        let start = files
            .len()
            .checked_sub(3)
            .ok_or(PackageError::UnsafeObject)?;
        let executable = if cfg!(windows) {
            "syncthing.exe"
        } else {
            "syncthing"
        };
        for (name, source) in [executable, "LICENSE.txt", "AUTHORS.txt"]
            .into_iter()
            .zip(&files[start..])
        {
            let mut source = source.try_clone().map_err(|_| PackageError::Io)?;
            source
                .seek(SeekFrom::Start(0))
                .map_err(|_| PackageError::Io)?;
            let mut target = directory
                .open_file(name, true)
                .map_err(|_| PackageError::Io)?;
            std::io::copy(&mut source, &mut target).map_err(|_| PackageError::Io)?;
            #[cfg(target_os = "macos")]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = if name == executable { 0o500 } else { 0o400 };
                target
                    .set_permissions(std::fs::Permissions::from_mode(mode))
                    .map_err(|_| PackageError::Io)?;
            }
            crate::import_staging::export_fs::flush(&target).map_err(|_| PackageError::Io)?;
        }
        directory.validate().map_err(|_| PackageError::UnsafeObject)
    }
    #[cfg(all(
        feature = "m17-owner-transport",
        any(
            all(windows, target_arch = "x86_64"),
            all(target_os = "macos", feature = "m8-local-startup-check")
        )
    ))]
    pub(crate) fn stage_owner_update(
        &self,
        runtime: &crate::import_staging::export_fs::Directory,
        root: &Path,
    ) -> Result<Self, PackageError> {
        const SLOT: &str = "managed-transport-update-package";
        if !runtime.is_path(root) {
            return Err(PackageError::InvalidPath);
        }
        let slot = if runtime
            .exists(SLOT)
            .map_err(|_| PackageError::UnsafeObject)?
        {
            runtime
                .child(SLOT, false)
                .map_err(|_| PackageError::UnsafeObject)?
        } else {
            let directory = runtime
                .create_child_exclusive(SLOT)
                .map_err(|_| PackageError::Io)?;
            self.install_fresh(&directory)?;
            directory.sync().map_err(|_| PackageError::Io)?;
            runtime.sync().map_err(|_| PackageError::Io)?;
            directory
        };
        let path = root.join(SLOT);
        let package = Self::open_owner_slot(&path)?;
        slot.validate().map_err(|_| PackageError::UnsafeObject)?;
        Ok(package)
    }
    #[cfg(all(
        feature = "m17-owner-transport",
        any(
            all(windows, target_arch = "x86_64"),
            all(target_os = "macos", feature = "m8-local-startup-check")
        )
    ))]
    pub(crate) fn open_owner_slot(path: &Path) -> Result<Self, PackageError> {
        let package = Self::open(path)?;
        check_owner_package_entries(path)?;
        Ok(package)
    }
    pub fn open(directory: &Path) -> Result<Self, PackageError> {
        #[cfg(all(windows, target_arch = "x86_64"))]
        {
            let files = windows::open_pinned(directory)?;
            Ok(Self(Arc::new(PackagePins {
                binary: directory.join("syncthing.exe"),
                _files: files,
            })))
        }
        #[cfg(all(target_os = "macos", feature = "m8-local-startup-check"))]
        {
            let files = macos::open_pinned(directory)?;
            Ok(Self(Arc::new(PackagePins {
                binary: directory.join("syncthing"),
                _files: files,
            })))
        }
        #[cfg(not(any(
            all(windows, target_arch = "x86_64"),
            all(target_os = "macos", feature = "m8-local-startup-check")
        )))]
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
        #[cfg(all(target_os = "macos", feature = "m8-local-startup-check"))]
        let _fresh_pins =
            macos::open_pinned(self.0.binary.parent().ok_or(PackageError::InvalidPath)?)?;
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
        write!(f, "PinnedSyncthingPackage(<pinned native {VERSION}>)")
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
