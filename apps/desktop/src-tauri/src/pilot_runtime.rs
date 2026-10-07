//! Explicit M0 pilot isolation, validated before Tauri or daemon startup.
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static ROOT: OnceLock<Option<PathBuf>> = OnceLock::new();
pub(crate) const MARKER: &str = "m0-native-desktop-runtime-v1\n";

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PilotError {
    Arguments,
    UnsafeRoot,
    UnownedRoot,
    ReusedRoot,
    Io,
}

fn ordinary(file: &Path, directory: bool) -> Result<(), PilotError> {
    let metadata = fs::symlink_metadata(file).map_err(|_| PilotError::Io)?;
    let unsafe_path = metadata.file_type().is_symlink();
    #[cfg(windows)]
    let unsafe_path = {
        use std::os::windows::fs::MetadataExt;
        unsafe_path || metadata.file_attributes() & 0x400 != 0
    };
    if unsafe_path || (directory && !metadata.is_dir()) || (!directory && !metadata.is_file()) {
        return Err(PilotError::UnsafeRoot);
    }
    Ok(())
}

fn validate(root: &Path) -> Result<(), PilotError> {
    if !root.is_absolute()
        || root
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(PilotError::UnsafeRoot);
    }
    for ancestor in root.ancestors() {
        ordinary(ancestor, true)?;
    }
    let repository = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.."))
        .map_err(|_| PilotError::Io)?;
    if fs::canonicalize(root)
        .map_err(|_| PilotError::Io)?
        .starts_with(repository)
    {
        return Err(PilotError::UnsafeRoot);
    }
    let marker = root.join("pilot-runtime.marker");
    ordinary(&marker, false)?;
    if fs::read(&marker).map_err(|_| PilotError::Io)? != MARKER.as_bytes() {
        return Err(PilotError::UnownedRoot);
    }
    let mut count = 0;
    for entry in fs::read_dir(root).map_err(|_| PilotError::Io)? {
        let entry = entry.map_err(|_| PilotError::Io)?;
        if entry.file_name() != "pilot-runtime.marker" {
            return Err(PilotError::ReusedRoot);
        }
        count += 1;
    }
    if count != 1 {
        return Err(PilotError::UnownedRoot);
    }
    Ok(())
}

fn parse(args: &[OsString]) -> Result<Option<PathBuf>, PilotError> {
    let mut root = None;
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--cryptex-pilot-runtime" {
            if root.is_some() || index + 1 == args.len() {
                return Err(PilotError::Arguments);
            }
            let path = PathBuf::from(&args[index + 1]);
            validate(&path)?;
            root = Some(path);
            index += 1;
        }
        index += 1;
    }
    Ok(root)
}

pub(crate) fn initialize() -> Result<(), PilotError> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    ROOT.set(parse(&args)?).map_err(|_| PilotError::Arguments)
}

pub(crate) fn root() -> Option<PathBuf> {
    ROOT.get().cloned().flatten()
}

pub(crate) fn configure(config: &mut tauri::utils::config::Config) {
    // Avoid sharing the product's single-instance identity or initial window.
    config.identifier = "com.cryptex.desktop.m0-pilot".into();
    for window in &mut config.app.windows {
        window.visible = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    fn fixture() -> PathBuf {
        let temporary = fs::canonicalize(std::env::temp_dir()).unwrap();
        loop {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let root = temporary.join(format!(
                "cryptex-pilot-runtime-{}-{sequence}-{stamp}",
                std::process::id()
            ));
            match fs::create_dir(&root) {
                Ok(()) => {
                    fs::write(root.join("pilot-runtime.marker"), MARKER).unwrap();
                    return root;
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => panic!("fixture create_dir failed: {err}"),
            }
        }
    }

    #[test]
    fn ordinary_launch_has_no_override_and_missing_option_is_rejected() {
        assert_eq!(parse(&["--cryptex-background".into()]).unwrap(), None);
        assert_eq!(
            parse(&["--cryptex-pilot-runtime".into()]),
            Err(PilotError::Arguments)
        );
    }

    #[test]
    fn fresh_owned_root_is_accepted_and_duplicate_rejected() {
        let root = fixture();
        let args = vec![
            "--cryptex-pilot-runtime".into(),
            root.clone().into_os_string(),
        ];
        assert_eq!(parse(&args).unwrap(), Some(root.clone()));
        assert_eq!(
            parse(&[args.clone(), args].concat()),
            Err(PilotError::Arguments)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn wrong_marker_and_reused_root_are_rejected() {
        let root = fixture();
        fs::write(root.join("pilot-runtime.marker"), "wrong").unwrap();
        assert_eq!(validate(&root), Err(PilotError::UnownedRoot));
        fs::write(root.join("pilot-runtime.marker"), MARKER).unwrap();
        fs::write(root.join("ready"), "old").unwrap();
        assert_eq!(validate(&root), Err(PilotError::ReusedRoot));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn repository_and_relative_roots_are_rejected() {
        assert_eq!(validate(Path::new("relative")), Err(PilotError::UnsafeRoot));
        let repo =
            fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")).unwrap();
        assert_eq!(validate(&repo), Err(PilotError::UnsafeRoot));
    }

    #[test]
    fn unsafe_root_entry_is_rejected() {
        let root = fixture();
        let link = root.with_extension("linked");
        #[cfg(windows)]
        {
            // Link creation may require a host privilege; this test uses the
            // owned marker as a directory-type mismatch without elevating.
            assert_eq!(
                validate(&root.join("pilot-runtime.marker")),
                Err(PilotError::UnsafeRoot)
            );
            let _ = link;
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&root, &link).unwrap();
            assert_eq!(validate(&link), Err(PilotError::UnsafeRoot));
            fs::remove_file(link).unwrap();
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pilot_context_uses_separate_identity_and_no_visible_window() {
        // Context generation embeds a macOS Info.plist symbol. Main already
        // generates that context; exercise the config transform directly here.
        let mut config = tauri::utils::config::Config {
            identifier: "com.cryptex.desktop".into(),
            ..Default::default()
        };
        config.app.windows = ["main", "secondary"]
            .into_iter()
            .map(|label| tauri::utils::config::WindowConfig {
                label: label.into(),
                visible: true,
                ..Default::default()
            })
            .collect();
        assert_eq!(config.app.windows.len(), 2);
        assert!(config.app.windows.iter().all(|window| window.visible));

        configure(&mut config);

        assert_eq!(config.identifier, "com.cryptex.desktop.m0-pilot");
        assert_eq!(
            config
                .app
                .windows
                .iter()
                .map(|window| window.label.as_str())
                .collect::<Vec<_>>(),
            ["main", "secondary"]
        );
        assert!(config.app.windows.iter().all(|window| !window.visible));
    }
}
