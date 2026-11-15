//! Closed development entry. Normal launch has no image/profile override.
//! The diagnostic accepts only an existing profile; it cannot initialize one.
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    Window,
    Check(PathBuf),
    WindowCheck(PathBuf),
}

pub fn mode(args: &[OsString]) -> Result<Mode, &'static str> {
    match args {
        [] => Ok(Mode::Window),
        [argument, root]
            if argument == "--cryptex-development-check" && Path::new(root).is_absolute() =>
        {
            Ok(Mode::Check(PathBuf::from(root)))
        }
        [argument, root]
            if argument == "--cryptex-development-window-check" && Path::new(root).is_absolute() =>
        {
            Ok(Mode::WindowCheck(PathBuf::from(root)))
        }
        _ => Err("development_arguments_refused"),
    }
}

/// UI diagnostics share the normal UI/native gates, but never select a personal
/// profile or replace HOME (which also changes macOS Keychain discovery).
pub fn window_check_runtime(root: &Path) -> Result<PathBuf, &'static str> {
    let temporary = std::env::temp_dir()
        .canonicalize()
        .map_err(|_| "development_window_check_runtime")?;
    let physical = root
        .canonicalize()
        .map_err(|_| "development_window_check_runtime")?;
    if physical != root || physical == temporary || !physical.starts_with(&temporary) {
        return Err("development_window_check_runtime");
    }
    let metadata =
        std::fs::symlink_metadata(root).map_err(|_| "development_window_check_runtime")?;
    if !metadata.is_dir() {
        return Err("development_window_check_runtime");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("development_window_check_runtime");
        }
    }
    Ok(physical)
}

pub fn runtime(home: Option<&OsStr>) -> Result<PathBuf, &'static str> {
    let home = Path::new(home.ok_or("development_home_missing")?);
    if !home.is_absolute() || home.parent().is_none() {
        return Err("development_home_invalid");
    }
    // Same stable release-profile location as the existing development Desktop.
    // Only the daemon can initialize it. Opening never seeds or adopts a profile.
    Ok(home.join("Library/Application Support/Cryptex-Dev/runtime/p0-2"))
}

/// UI lifecycle only; this state never authorizes a daemon operation.
#[derive(Debug, Default, PartialEq, Eq)]
pub enum Lifecycle {
    #[default]
    Opening,
    Ready,
    ConfirmingQuit,
    Stopping,
    ExitReady,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Quit {
    Proceed,
    Confirm,
    Prevent,
}

impl Lifecycle {
    pub fn accepts_work(&self) -> bool {
        *self == Self::Ready
    }

    pub fn ready(&mut self) -> bool {
        if *self != Self::Opening {
            return false;
        }
        *self = Self::Ready;
        true
    }

    pub fn request_quit(&mut self, draft_unresolved: bool) -> Quit {
        match self {
            Self::ConfirmingQuit | Self::Stopping => Quit::Prevent,
            Self::ExitReady => Quit::Proceed,
            Self::Ready if draft_unresolved => {
                *self = Self::ConfirmingQuit;
                Quit::Confirm
            }
            _ => {
                *self = Self::Stopping;
                Quit::Proceed
            }
        }
    }

    pub fn confirm_quit(&mut self, discard: bool) -> Quit {
        if *self != Self::ConfirmingQuit {
            return Quit::Prevent;
        }
        *self = if discard { Self::Stopping } else { Self::Ready };
        if discard {
            Quit::Proceed
        } else {
            Quit::Prevent
        }
    }

    pub fn shutdown_complete(&mut self) {
        if *self == Self::Stopping {
            *self = Self::ExitReady;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_quit_preserves_session_and_serializes_nested_requests() {
        let mut lifecycle = Lifecycle::default();
        assert!(!lifecycle.accepts_work());
        assert!(lifecycle.ready());
        assert!(lifecycle.accepts_work());
        assert_eq!(lifecycle.request_quit(true), Quit::Confirm);
        assert!(!lifecycle.accepts_work());
        assert_eq!(lifecycle.request_quit(false), Quit::Prevent);
        assert_eq!(lifecycle.confirm_quit(false), Quit::Prevent);
        assert_eq!(lifecycle, Lifecycle::Ready);
        assert!(lifecycle.accepts_work());
        assert_eq!(lifecycle.request_quit(true), Quit::Confirm);
        assert_eq!(lifecycle.confirm_quit(true), Quit::Proceed);
        assert!(!lifecycle.accepts_work());
        assert!(!lifecycle.ready());
    }

    #[test]
    fn cancelled_startup_cannot_later_open_a_window_or_request_draft_discard() {
        let mut lifecycle = Lifecycle::default();
        assert_eq!(lifecycle.request_quit(true), Quit::Proceed);
        assert!(!lifecycle.ready());
        assert_eq!(lifecycle.confirm_quit(true), Quit::Prevent);
    }

    #[test]
    fn clean_quit_does_not_prompt_and_cannot_be_revived() {
        let mut lifecycle = Lifecycle::default();
        assert!(lifecycle.ready());
        assert_eq!(lifecycle.request_quit(false), Quit::Proceed);
        assert_eq!(lifecycle.confirm_quit(false), Quit::Prevent);
        assert!(!lifecycle.ready());
        // A nested Quit during shutdown must not claim clean exit prematurely.
        assert_eq!(lifecycle.request_quit(false), Quit::Prevent);
        lifecycle.shutdown_complete();
        assert!(!lifecycle.accepts_work());
        assert_eq!(lifecycle.request_quit(true), Quit::Proceed);
        assert!(!lifecycle.ready());
    }

    #[test]
    fn normal_entry_has_no_fixture_or_path_override() {
        assert_eq!(mode(&[]), Ok(Mode::Window));
        let root = std::env::temp_dir().join("cryptex-check-existing");
        assert_eq!(
            mode(&[
                "--cryptex-development-check".into(),
                root.clone().into_os_string()
            ]),
            Ok(Mode::Check(root))
        );
        assert!(mode(&["--cryptex-development-check".into()]).is_err());
        for flag in [
            "--cryptex-pilot-resume",
            "--cryptex-m8-local-content-ui",
            "--cryptex-m8-local-startup-check",
            "--runtime-root",
            "--cryptex-background",
        ] {
            assert!(mode(&[flag.into()]).is_err());
        }
        assert!(mode(&["--cryptex-development-check".into(), "profile".into()]).is_err());
        let root = std::env::temp_dir().join("existing-window-check");
        assert_eq!(
            mode(&[
                "--cryptex-development-window-check".into(),
                root.clone().into_os_string()
            ]),
            Ok(Mode::WindowCheck(root))
        );
        assert!(
            mode(&[
                "--cryptex-development-window-check".into(),
                "relative".into()
            ])
            .is_err()
        );
    }

    #[test]
    fn window_diagnostic_requires_existing_physical_temporary_directory() {
        let temporary = std::env::temp_dir().canonicalize().unwrap();
        assert!(window_check_runtime(&temporary).is_err());
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = temporary.join(format!("cryptex-window-check-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        assert_eq!(window_check_runtime(&root), Ok(root.clone()));
        assert!(window_check_runtime(&root.join("missing")).is_err());
        assert!(!root.join("missing").exists());
        let file = root.join("file");
        std::fs::write(&file, b"not a profile").unwrap();
        assert!(window_check_runtime(&file).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::{PermissionsExt, symlink};
            let link = root.join("alias");
            symlink(&root, &link).unwrap();
            assert!(window_check_runtime(&link).is_err());
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
            assert!(window_check_runtime(&root).is_err());
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn profile_location_is_fixed_and_never_created_by_selection() {
        assert!(runtime(None).is_err());
        assert!(runtime(Some(OsStr::new("relative"))).is_err());
        let base = std::env::temp_dir().join("cryptex-development-location-test");
        assert_eq!(
            runtime(Some(base.as_os_str())).unwrap(),
            base.join("Library/Application Support/Cryptex-Dev/runtime/p0-2")
        );
    }
}
