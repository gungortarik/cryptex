//! Closed development entry. Normal launch has no image/profile override.
//! The diagnostic accepts only an existing profile; it cannot initialize one.
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    Window,
    Check(PathBuf),
}

pub fn mode(args: &[OsString]) -> Result<Mode, &'static str> {
    match args {
        [] => Ok(Mode::Window),
        [argument, root] if argument == "--cryptex-development-check" && Path::new(root).is_absolute() => Ok(Mode::Check(PathBuf::from(root))),
        _ => Err("development_arguments_refused"),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_entry_has_no_fixture_or_path_override() {
        assert_eq!(mode(&[]), Ok(Mode::Window));
        let root = std::env::temp_dir().join("cryptex-check-existing");
        assert_eq!(mode(&["--cryptex-development-check".into(), root.clone().into_os_string()]), Ok(Mode::Check(root)));
        assert!(mode(&["--cryptex-development-check".into()]).is_err());
        for flag in ["--cryptex-pilot-resume", "--cryptex-m8-local-content-ui", "--cryptex-m8-local-startup-check", "--runtime-root", "--cryptex-background"] {
            assert!(mode(&[flag.into()]).is_err());
        }
        assert!(mode(&["--cryptex-development-check".into(), "profile".into()]).is_err());
    }

    #[test]
    fn profile_location_is_fixed_and_never_created_by_selection() {
        assert!(runtime(None).is_err());
        assert!(runtime(Some(OsStr::new("relative"))).is_err());
        let base = std::env::temp_dir().join("cryptex-development-location-test");
        assert_eq!(runtime(Some(base.as_os_str())).unwrap(), base.join("Library/Application Support/Cryptex-Dev/runtime/p0-2"));
    }
}
