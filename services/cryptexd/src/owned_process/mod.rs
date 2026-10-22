//! Owned native child lifecycle for the M8 managed transport backend.
//!
//! Process ownership only: this does not validate a Syncthing package's pin,
//! publisher, profile isolation/ACLs, API ownership or peer authorization.
//! The future transport controller must establish those before launching.
//! Nothing exposes this API through Desktop/CLI IPC or daemon startup.

use std::ffi::OsString;
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use crate::paths::assert_outside_repository;

pub(crate) mod endpoint;

#[cfg(windows)]
mod windows;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnedProcessError {
    InvalidLaunch,
    UnsafeWorkingDirectory,
    UnsupportedPlatform,
    Platform { operation: &'static str, code: i64 },
    Deadline,
}

impl fmt::Display for OwnedProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "owned_process_{self:?}")
    }
}

impl std::error::Error for OwnedProcessError {}

/// Fixed, backend-chosen Syncthing role. Not proof that this package or profile
/// is safe to execute. Never accept these paths from an untrusted renderer.
pub struct LaunchSpec {
    program: PathBuf,
    working_directory: PathBuf,
    arguments: Vec<OsString>,
    environment: Vec<(OsString, OsString)>,
}

impl LaunchSpec {
    pub(crate) fn syncthing_generate(
        packaged_binary: &Path,
        isolated_profile: &Path,
    ) -> Result<Self, OwnedProcessError> {
        let mut environment = minimal_system_environment();
        environment.push(("STHOMEDIR".into(), isolated_profile.as_os_str().to_owned()));
        environment.push(("STNODEFAULTFOLDER".into(), "1".into()));
        Self::validate(
            packaged_binary,
            isolated_profile,
            vec![
                "generate".into(),
                "--home".into(),
                isolated_profile.as_os_str().to_owned(),
                "--no-port-probing".into(),
            ],
            environment,
        )
    }

    pub fn syncthing(
        packaged_binary: &Path,
        isolated_profile: &Path,
    ) -> Result<Self, OwnedProcessError> {
        let arguments = vec![
            "serve".into(),
            "--home".into(),
            isolated_profile.as_os_str().to_owned(),
            "--no-browser".into(),
            "--no-restart".into(),
            "--no-upgrade".into(),
            "--no-port-probing".into(),
            "--log-file".into(),
            isolated_profile.join("native.log").into_os_string(),
        ];
        let mut environment = minimal_system_environment();
        environment.push(("STHOMEDIR".into(), isolated_profile.as_os_str().to_owned()));
        environment.push(("STNODEFAULTFOLDER".into(), "1".into()));
        Self::validate(packaged_binary, isolated_profile, arguments, environment)
    }

    fn validate(
        program: &Path,
        working_directory: &Path,
        arguments: Vec<OsString>,
        environment: Vec<(OsString, OsString)>,
    ) -> Result<Self, OwnedProcessError> {
        if !ordinary_absolute_path(program)
            || !ordinary_absolute_path(working_directory)
            || arguments.len() > 32
            || environment.len() > 32
        {
            return Err(OwnedProcessError::InvalidLaunch);
        }
        if assert_outside_repository(working_directory).is_err() {
            return Err(OwnedProcessError::UnsafeWorkingDirectory);
        }
        for ancestor in working_directory.ancestors() {
            match ancestor.join(".git").try_exists() {
                Ok(false) => {}
                _ => return Err(OwnedProcessError::UnsafeWorkingDirectory),
            }
        }
        if environment.iter().any(|(key, _)| {
            key.is_empty()
                || key
                    .to_str()
                    .is_none_or(|key| key.contains('=') || key.contains('\0'))
        }) {
            return Err(OwnedProcessError::InvalidLaunch);
        }
        Ok(Self {
            program: program.to_owned(),
            working_directory: working_directory.to_owned(),
            arguments,
            environment,
        })
    }
}

impl fmt::Debug for LaunchSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LaunchSpec(<backend paths/arguments/environment redacted>)")
    }
}

fn ordinary_absolute_path(path: &Path) -> bool {
    path.is_absolute()
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
}

fn minimal_system_environment() -> Vec<(OsString, OsString)> {
    // No ambient ST*, proxy, loader or PATH override reaches the transport.
    // The native executable is explicit; no shell or PATH lookup is used.
    [
        "SYSTEMROOT",
        "WINDIR",
        "TEMP",
        "TMP",
        "LOCALAPPDATA",
        "APPDATA",
        "USERPROFILE",
    ]
    .into_iter()
    .filter_map(|name| std::env::var_os(name).map(|value| (name.into(), value)))
    .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitObservation {
    pub exit_code: Option<i64>,
    /// A forced cleanup may NEVER be reported as a clean stop, even if the
    /// process raced to return exit 0 before termination reached it.
    pub forced: bool,
}

impl ExitObservation {
    pub fn is_clean(self) -> bool {
        self.exit_code == Some(0) && !self.forced
    }
}

pub struct OwnedProcess {
    #[cfg(windows)]
    native: windows::NativeProcess,
    forced: bool,
}

impl OwnedProcess {
    /// On Windows, the job is attached atomically by CreateProcess's attribute
    /// list, before child code runs. Failure never falls back to an unowned spawn.
    pub fn spawn(spec: &LaunchSpec) -> Result<Self, OwnedProcessError> {
        #[cfg(windows)]
        {
            Ok(Self {
                native: windows::NativeProcess::spawn(spec)?,
                forced: false,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = (
                &spec.program,
                &spec.working_directory,
                &spec.arguments,
                &spec.environment,
            );
            Err(OwnedProcessError::UnsupportedPlatform)
        }
    }

    pub fn pid(&self) -> u32 {
        #[cfg(windows)]
        {
            self.native.pid()
        }
        #[cfg(not(windows))]
        {
            0
        }
    }

    /// Returns exit only after the owned process AND job descendants are gone.
    pub fn try_exit(&mut self) -> Result<Option<ExitObservation>, OwnedProcessError> {
        #[cfg(windows)]
        {
            Ok(self.native.try_exit()?.map(|exit_code| ExitObservation {
                exit_code: Some(i64::from(exit_code)),
                forced: self.forced,
            }))
        }
        #[cfg(not(windows))]
        {
            Err(OwnedProcessError::UnsupportedPlatform)
        }
    }

    /// Caller requests graceful shutdown through its separately authenticated
    /// control channel, then waits here. Timeout itself does not kill anything.
    pub fn wait_for_exit(
        &mut self,
        timeout: Duration,
    ) -> Result<ExitObservation, OwnedProcessError> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(OwnedProcessError::InvalidLaunch)?;
        loop {
            if let Some(exit) = self.try_exit()? {
                return Ok(exit);
            }
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .filter(|duration| !duration.is_zero())
                .ok_or(OwnedProcessError::Deadline)?;
            std::thread::sleep(remaining.min(Duration::from_millis(10)));
        }
    }

    /// Only this object's owned OS job can be terminated. No PID/name lookup,
    /// global kill command, or way to adopt an existing process is provided.
    pub fn force_cleanup(
        &mut self,
        timeout: Duration,
    ) -> Result<ExitObservation, OwnedProcessError> {
        self.forced = true;
        #[cfg(windows)]
        {
            self.native.terminate_job()?;
            self.wait_for_exit(timeout)
        }
        #[cfg(not(windows))]
        {
            let _ = timeout;
            Err(OwnedProcessError::UnsupportedPlatform)
        }
    }
}

impl fmt::Debug for OwnedProcess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnedProcess")
            .field("forced", &self.forced)
            .finish_non_exhaustive()
    }
}

#[cfg(all(test, windows))]
mod tests;
