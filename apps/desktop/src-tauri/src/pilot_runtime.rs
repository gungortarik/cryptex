//! Explicit M0 pilot isolation, validated before Tauri or daemon startup.
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static ROOT: OnceLock<Option<PathBuf>> = OnceLock::new();
pub(crate) const MARKER: &str = "m0-native-desktop-runtime-v1\n";
pub(crate) const TIMELINE_FIXTURE_MARKER: &str = "m5-timeline-fixture-v1\n";

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

fn is_absent_path_error(error: &io::Error) -> bool {
    matches!(error.kind(), io::ErrorKind::NotFound)
}

/// Compile-time repository hint from the Desktop crate layout (apps/desktop/src-tauri -> repo root).
///
/// On a machine that still has this build tree, containment under the hint is rejected.
/// Packaged binaries must not require the build machine path to exist.
fn build_time_repository_hint() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// Resolve the build-time repository boundary.
///
/// - Present and canonicalizable -> Some(repo)
/// - Absent (`NotFound`) -> `None` (portable packaged launch)
/// - Permission or unexpected I/O -> PilotError::Io (fail closed)
fn resolve_build_time_repository(hint: &Path) -> Result<Option<PathBuf>, PilotError> {
    match fs::canonicalize(hint) {
        Ok(path) => Ok(Some(path)),
        Err(error) if is_absent_path_error(&error) => Ok(None),
        Err(_) => Err(PilotError::Io),
    }
}

/// Reject runtimes that sit inside any Git checkout, independent of the build host path.
fn ancestor_contains_git(root: &Path) -> Result<bool, PilotError> {
    for ancestor in root.ancestors() {
        let git = ancestor.join(".git");
        match fs::symlink_metadata(&git) {
            Ok(_) => return Ok(true),
            Err(error) if is_absent_path_error(&error) => continue,
            Err(_) => return Err(PilotError::Io),
        }
    }
    Ok(false)
}

fn validate(root: &Path) -> Result<(), PilotError> {
    validate_with_repository_hint(root, &build_time_repository_hint())
}

fn validate_with_repository_hint(root: &Path, repository_hint: &Path) -> Result<(), PilotError> {
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
    if let Some(repository) = resolve_build_time_repository(repository_hint)?
        && fs::canonicalize(root)
            .map_err(|_| PilotError::Io)?
            .starts_with(repository)
    {
        return Err(PilotError::UnsafeRoot);
    }
    if ancestor_contains_git(root)? {
        return Err(PilotError::UnsafeRoot);
    }
    let marker = root.join("pilot-runtime.marker");
    ordinary(&marker, false)?;
    if fs::read(&marker).map_err(|_| PilotError::Io)? != MARKER.as_bytes() {
        return Err(PilotError::UnownedRoot);
    }
    let mut count = 0;
    let mut has_timeline_fixture = false;
    for entry in fs::read_dir(root).map_err(|_| PilotError::Io)? {
        let entry = entry.map_err(|_| PilotError::Io)?;
        match entry.file_name().to_str() {
            Some("pilot-runtime.marker") => count += 1,
            Some("pilot-timeline-fixture.marker") => {
                ordinary(&entry.path(), false)?;
                if fs::read(entry.path()).map_err(|_| PilotError::Io)?
                    != TIMELINE_FIXTURE_MARKER.as_bytes()
                {
                    return Err(PilotError::UnownedRoot);
                }
                has_timeline_fixture = true;
                count += 1;
            }
            _ => count += 1,
        }
    }
    if count != 1 && !has_timeline_fixture {
        return Err(PilotError::ReusedRoot);
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

    fn fixture_under(parent: &Path) -> PathBuf {
        loop {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let root = parent.join(format!(
                "cryptex-pilot-runtime-inside-repo-{}-{sequence}-{stamp}",
                std::process::id()
            ));
            match fs::create_dir(&root) {
                Ok(()) => {
                    fs::write(root.join("pilot-runtime.marker"), MARKER).unwrap();
                    return root;
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => panic!("fixture_under create_dir failed: {err}"),
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
    fn valid_isolated_runtime_is_accepted() {
        let root = fixture();
        assert_eq!(validate(&root), Ok(()));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn absent_build_time_repository_allows_isolated_runtime() {
        let root = fixture();
        let missing_build_tree = root.join("missing-build-time-cargo-manifest-repo");
        assert!(
            !missing_build_tree.exists(),
            "fixture must not create the absent build-time path"
        );
        assert_eq!(
            validate_with_repository_hint(&root, &missing_build_tree),
            Ok(())
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
    fn marked_timeline_fixture_allows_seeded_runtime_but_unmarked_reuse_is_rejected() {
        let root = fixture();
        fs::create_dir(root.join("canonical")).unwrap();
        assert_eq!(validate(&root), Err(PilotError::ReusedRoot));

        fs::write(
            root.join("pilot-timeline-fixture.marker"),
            TIMELINE_FIXTURE_MARKER,
        )
        .unwrap();
        assert_eq!(validate(&root), Ok(()));

        fs::write(
            root.join("pilot-timeline-fixture.marker"),
            "unrecognized-fixture\n",
        )
        .unwrap();
        assert_eq!(validate(&root), Err(PilotError::UnownedRoot));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn repository_and_relative_roots_are_rejected() {
        assert_eq!(validate(Path::new("relative")), Err(PilotError::UnsafeRoot));
        let repo =
            fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")).unwrap();
        assert_eq!(validate(&repo), Err(PilotError::UnsafeRoot));
        // Inside an existing checkout must remain rejected even when the build-time
        // hint is absent (portable packaged binary on another machine).
        let nested = fixture_under(&repo);
        let missing_build_tree = nested.join("missing-build-time-cargo-manifest-repo");
        assert_eq!(
            validate_with_repository_hint(&nested, &missing_build_tree),
            Err(PilotError::UnsafeRoot)
        );
        fs::remove_dir_all(nested).unwrap();
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
