//! Shared boundary between the installed Desktop and the local package updater.
use serde::{Deserialize, Serialize};
use std::{
    env,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

pub const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
pub const KEYCHAIN_PROBE: &str = "--cryptex-update-keychain";
pub const KEYCHAIN_REOPEN: &str = "--cryptex-update-keychain-unattended";
pub const PROBE: &str = "--cryptex-update-probe";

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Health {
    pub status: String,
    pub desktop_pid: u32,
    pub daemon_pid: u32,
    pub desktop_protocol: u32,
    pub daemon_protocol: u32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub requires_existing_key: bool,
}

pub struct Layout {
    pub active: PathBuf,
    pub pending: PathBuf,
    pub previous: PathBuf,
    state: PathBuf,
}

impl Layout {
    pub fn current() -> io::Result<Self> {
        let base = |key| {
            env::var_os(key)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .ok_or_else(|| io::Error::other("missing user profile"))
        };
        let (parent, state, name) = if cfg!(windows) {
            let local = base("LOCALAPPDATA")?;
            (
                local.join("Programs"),
                local.join("Cryptex-Updater"),
                "Cryptex",
            )
        } else if cfg!(target_os = "macos") {
            let home = base("HOME")?;
            (
                home.join("Applications"),
                home.join("Library/Application Support/Cryptex-Updater"),
                "Cryptex.app",
            )
        } else {
            return Err(io::Error::other("unsupported platform"));
        };
        Ok(Self {
            active: parent.join(name),
            pending: parent.join(format!("{name}.pending")),
            previous: parent.join(format!("{name}.previous")),
            state,
        })
    }

    fn lock_file(&self, name: &str) -> io::Result<File> {
        fs::create_dir_all(&self.state)?;
        File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.state.join(name))
    }

    pub fn begin_update(&self) -> io::Result<File> {
        let transaction = self.lock_file("update.lock")?;
        transaction
            .try_lock()
            .map_err(|_| io::Error::other("another update is running"))?;
        self.lock_file("desktop.lock")?
            .try_lock()
            .map_err(|_| io::Error::other("quit Cryptex before updating"))?;
        Ok(transaction)
    }

    pub fn desktop_session(&self) -> io::Result<Option<File>> {
        if fs::canonicalize(desktop(&self.active)).ok().as_ref()
            != Some(&fs::canonicalize(env::current_exe()?)?)
        {
            return Ok(None);
        }
        let session = self.lock_file("desktop.lock")?;
        session
            .try_lock_shared()
            .map_err(|_| io::Error::other("desktop session unavailable"))?;
        self.lock_file("update.lock")?
            .try_lock_shared()
            .map_err(|_| io::Error::other("Cryptex update in progress"))?;
        if self.previous.try_exists()? || self.intent().try_exists()? {
            return Err(io::Error::other("unfinished package update"));
        }
        Ok(Some(session))
    }

    pub fn probe_session(&self) -> io::Result<File> {
        let exe = env::current_exe()?.canonicalize()?;
        if ![&self.active, &self.pending, &self.previous]
            .iter()
            .any(|image| desktop(image).canonicalize().is_ok_and(|path| path == exe))
        {
            return Err(io::Error::other("probe requires the installed Desktop"));
        }
        let session = self.lock_file("desktop.lock")?;
        session
            .try_lock()
            .map_err(|_| io::Error::other("Desktop is running"))?;
        match self.lock_file("update.lock")?.try_lock_shared() {
            Err(std::fs::TryLockError::WouldBlock) => Ok(session),
            Err(std::fs::TryLockError::Error(e)) => Err(e),
            Ok(()) => Err(io::Error::other("no package transaction is running")),
        }
    }

    pub fn release_receipts(&self) -> PathBuf {
        self.state.join("release-receipts")
    }

    pub fn intent(&self) -> PathBuf {
        self.state.join("package-transition.json")
    }
}

pub fn desktop(package: &Path) -> PathBuf {
    if cfg!(windows) {
        package.join("cryptex-desktop.exe")
    } else {
        package.join("Contents/MacOS/cryptex-desktop")
    }
}
