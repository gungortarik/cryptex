//! Test-only HKCU Run login-start probe for P0-2 Mode A.
//!
//! Registers a reversible current-user Run value. This does **not** provide
//! OS-level crash restart (unlike Task Scheduler RestartOnFailure / launchd KeepAlive).
//! Does not select ADR-0012 or any supervision winner.

use std::path::{Path, PathBuf};

use winreg::RegKey;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};

use crate::error::{LifecycleError, LifecycleResult};
use crate::instance::observe_instance;
use crate::paths::assert_outside_repository;

/// Test-only Run value. Must never be used as a production identity.
pub const TEST_HKCU_RUN_VALUE_NAME: &str = "CryptexDevP02HkcuRun";

/// Relative path under HKCU for the per-user Run key.
pub const HKCU_RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

#[derive(Debug, Clone)]
pub struct HkcuRunSpec {
    pub value_name: String,
    pub cryptexd_bin: PathBuf,
    pub runtime_root: PathBuf,
    pub ready_file: PathBuf,
    pub stop_file: PathBuf,
}

impl HkcuRunSpec {
    pub fn for_runtime(runtime_root: &Path, cryptexd_bin: &Path) -> LifecycleResult<Self> {
        assert_outside_repository(runtime_root)?;
        let root = runtime_root.to_path_buf();
        Ok(Self {
            value_name: TEST_HKCU_RUN_VALUE_NAME.into(),
            cryptexd_bin: cryptexd_bin.to_path_buf(),
            runtime_root: root.clone(),
            ready_file: root.join("ready"),
            stop_file: root.join("stop"),
        })
    }

    /// Default P0-2 compare root for this probe.
    pub fn default_runtime_root() -> LifecycleResult<PathBuf> {
        let local = std::env::var_os("LOCALAPPDATA").ok_or_else(|| {
            LifecycleError::Io(
                "LOCALAPPDATA is not set; cannot resolve Cryptex-Dev HKCU probe root".into(),
            )
        })?;
        let root = PathBuf::from(local)
            .join("Cryptex-Dev")
            .join("p0-2")
            .join("compare")
            .join("mode-a-hkcu");
        assert_outside_repository(&root)?;
        Ok(root)
    }

    pub fn command_line(&self) -> String {
        format!(
            "{} lifecycle login-hold --runtime-root {}",
            quote_windows_arg(&self.cryptexd_bin.display().to_string()),
            quote_windows_arg(&self.runtime_root.display().to_string()),
        )
    }
}

/// Build the REG_SZ command line with Windows quoting for exe + path args.
pub fn build_run_command_line(
    cryptexd_bin: &Path,
    runtime_root: &Path,
    ready_file: &Path,
    stop_file: &Path,
) -> String {
    format!(
        "{} lifecycle hold --runtime-root {} --ready-file {} --stop-file {}",
        quote_windows_arg(&cryptexd_bin.display().to_string()),
        quote_windows_arg(&runtime_root.display().to_string()),
        quote_windows_arg(&ready_file.display().to_string()),
        quote_windows_arg(&stop_file.display().to_string()),
    )
}

/// Quote a Windows command-line argument when required.
pub fn quote_windows_arg(value: &str) -> String {
    if value.is_empty() {
        return "\"\"".to_string();
    }
    let needs_quotes = value.chars().any(|c| c.is_whitespace() || c == '"');
    if !needs_quotes {
        return value.to_string();
    }
    let mut out = String::from('"');
    let mut backslashes = 0usize;
    for ch in value.chars() {
        match ch {
            '\\' => {
                backslashes += 1;
            }
            '"' => {
                out.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            _ => {
                if backslashes > 0 {
                    out.extend(std::iter::repeat_n('\\', backslashes));
                    backslashes = 0;
                }
                out.push(ch);
            }
        }
    }
    if backslashes > 0 {
        out.extend(std::iter::repeat_n('\\', backslashes * 2));
    }
    out.push('"');
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HkcuRunStatus {
    pub value_name: String,
    pub registered: bool,
    pub command_line: Option<String>,
    pub daemon_lock_held: bool,
    pub daemon_pid: Option<u32>,
    pub recorded_pid: Option<u32>,
    pub ready_present: bool,
}

/// Ensure runtime root exists and clear control files (does not touch the registry).
pub fn prepare_runtime(spec: &HkcuRunSpec) -> LifecycleResult<()> {
    std::fs::create_dir_all(&spec.runtime_root)?;
    assert_outside_repository(&spec.runtime_root)?;
    remove_if_present(&spec.ready_file)?;
    clear_stop_request(spec)?;
    Ok(())
}

/// Remove the one-shot stop marker after the on-demand registration probe ends.
/// The registered login command reuses this path, so leaving the marker behind
/// would make the next login-start process exit immediately.
pub fn clear_stop_request(spec: &HkcuRunSpec) -> LifecycleResult<()> {
    remove_if_present(&spec.stop_file)
}

fn remove_if_present(path: &Path) -> LifecycleResult<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(LifecycleError::Io(format!(
            "failed to remove lifecycle control file {}: {error}",
            path.display()
        ))),
    }
}

/// Create or update only the test-only HKCU Run value.
pub fn register(spec: &HkcuRunSpec) -> LifecycleResult<()> {
    let command = spec.command_line();
    // Windows documents a maximum of 260 characters for a Run command.
    // Reject before touching control files or the registry.
    if command.encode_utf16().count() > 260 {
        return Err(LifecycleError::Usage(
            "HKCU Run command exceeds 260 characters; use shorter test binary/runtime paths".into(),
        ));
    }
    prepare_runtime(spec)?;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(HKCU_RUN_SUBKEY)
        .map_err(|error| LifecycleError::Io(format!("failed to open HKCU Run key: {error}")))?;
    key.set_value(&spec.value_name, &command).map_err(|error| {
        LifecycleError::Io(format!(
            "failed to set HKCU Run value {}: {error}",
            spec.value_name
        ))
    })?;
    Ok(())
}

/// Query only the test-only value; never enumerates for mutation.
pub fn status(spec: &HkcuRunSpec) -> LifecycleResult<HkcuRunStatus> {
    let command_line = read_test_value(&spec.value_name)?;
    let observation = if spec.runtime_root.exists() {
        observe_instance(&spec.runtime_root)?
    } else {
        crate::instance::InstanceObservation {
            lock_present: false,
            lock_held: false,
            holder_pid: None,
            recorded_pid: None,
        }
    };
    Ok(HkcuRunStatus {
        value_name: spec.value_name.clone(),
        registered: command_line.is_some(),
        command_line,
        daemon_lock_held: observation.lock_held,
        daemon_pid: observation.holder_pid,
        recorded_pid: observation.recorded_pid,
        ready_present: spec.ready_file.exists(),
    })
}

/// Delete only the test-only value name. Idempotent if absent.
pub fn unregister(spec: &HkcuRunSpec) -> LifecycleResult<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = match hkcu.open_subkey_with_flags(HKCU_RUN_SUBKEY, KEY_SET_VALUE | KEY_READ) {
        Ok(key) => key,
        Err(error) => {
            // Missing Run key is fine for teardown.
            let msg = error.to_string().to_ascii_lowercase();
            if msg.contains("not found") || msg.contains("cannot find") {
                return Ok(());
            }
            return Err(LifecycleError::Io(format!(
                "failed to open HKCU Run key for delete: {error}"
            )));
        }
    };
    match key.delete_value(&spec.value_name) {
        Ok(()) => Ok(()),
        Err(error) => {
            let msg = error.to_string().to_ascii_lowercase();
            if msg.contains("not found") || msg.contains("cannot find") || msg.contains("error 2") {
                Ok(())
            } else {
                Err(LifecycleError::Io(format!(
                    "failed to delete HKCU Run value {}: {error}",
                    spec.value_name
                )))
            }
        }
    }
}

fn read_test_value(value_name: &str) -> LifecycleResult<Option<String>> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = match hkcu.open_subkey_with_flags(HKCU_RUN_SUBKEY, KEY_READ) {
        Ok(key) => key,
        Err(_) => return Ok(None),
    };
    match key.get_value::<String, _>(value_name) {
        Ok(value) => Ok(Some(value)),
        Err(error) => {
            let msg = error.to_string().to_ascii_lowercase();
            if msg.contains("not found") || msg.contains("cannot find") || msg.contains("error 2") {
                Ok(None)
            } else {
                Err(LifecycleError::Io(format!(
                    "failed to read HKCU Run value {value_name}: {error}"
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_root(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("cryptexd-hkcu-{label}-{stamp}"));
        assert_outside_repository(&root).expect("outside repo");
        std::fs::create_dir_all(&root).expect("mkdir");
        root
    }

    #[test]
    fn registry_path_and_value_identity_are_test_only() {
        assert_eq!(
            HKCU_RUN_SUBKEY,
            r"Software\Microsoft\Windows\CurrentVersion\Run"
        );
        assert_eq!(TEST_HKCU_RUN_VALUE_NAME, "CryptexDevP02HkcuRun");
        assert!(TEST_HKCU_RUN_VALUE_NAME.contains("Dev"));
        assert!(TEST_HKCU_RUN_VALUE_NAME.contains("P02"));
        assert!(!TEST_HKCU_RUN_VALUE_NAME.eq_ignore_ascii_case("Cryptex"));
    }

    #[test]
    fn command_line_quotes_paths_with_spaces() {
        let exe = Path::new(r"C:\Program Files\Cryptex-Dev\cryptexd.exe");
        let root = Path::new(r"C:\Users\Tarik\AppData\Local\Cryptex-Dev\p0-2\compare\mode-a-hkcu");
        let ready = root.join("ready");
        let stop = root.join("stop");
        let cmd = build_run_command_line(exe, root, &ready, &stop);
        assert!(cmd.starts_with("\"C:\\Program Files\\Cryptex-Dev\\cryptexd.exe\""));
        assert!(cmd.contains("lifecycle hold"));
        assert!(cmd.contains("--runtime-root"));
        assert!(cmd.contains("mode-a-hkcu"));
        assert!(!cmd.contains("syncthing-p0-1"));
    }

    #[test]
    fn compact_login_command_uses_runtime_control_files_without_repeating_paths() {
        let root = Path::new(r"C:\Users\Tarik\AppData\Local\Cryptex-Dev\p0-2\wl-1002");
        let bin = root.join("cryptexd.exe");
        let spec = HkcuRunSpec::for_runtime(root, &bin).expect("spec");
        let command = spec.command_line();
        assert!(command.contains("lifecycle login-hold --runtime-root"));
        assert!(command.encode_utf16().count() <= 260);
        assert_eq!(spec.ready_file, root.join("ready"));
        assert_eq!(spec.stop_file, root.join("stop"));
    }

    #[test]
    fn oversized_command_is_rejected_before_runtime_or_registry_mutation() {
        let root = unique_root("oversized");
        let mut spec =
            HkcuRunSpec::for_runtime(&root, &root.join(format!("{}.exe", "x".repeat(260))))
                .expect("spec");
        spec.value_name = format!(
            "CryptexDevP02Oversized{}",
            root.file_name().unwrap().to_string_lossy()
        );
        std::fs::write(&spec.stop_file, "sentinel\n").expect("stop sentinel");
        let before = read_test_value(&spec.value_name).expect("registry snapshot");
        assert!(matches!(register(&spec), Err(LifecycleError::Usage(_))));
        assert_eq!(
            std::fs::read_to_string(&spec.stop_file).unwrap(),
            "sentinel\n"
        );
        assert_eq!(read_test_value(&spec.value_name).unwrap(), before);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn clear_stop_request_removes_one_shot_marker_idempotently() {
        let root = unique_root("stop-cleanup");
        let spec = HkcuRunSpec::for_runtime(&root, Path::new(r"C:\Cryptex-Dev\cryptexd.exe"))
            .expect("HKCU run spec");
        std::fs::write(&spec.stop_file, "stop\n").expect("write stop request");

        clear_stop_request(&spec).expect("clear stop request");
        assert!(!spec.stop_file.exists());
        clear_stop_request(&spec).expect("clear missing stop request");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn quote_windows_arg_is_stable_for_plain_and_spaced_paths() {
        assert_eq!(
            quote_windows_arg(r"C:\Cryptex-Dev\cryptexd.exe"),
            r"C:\Cryptex-Dev\cryptexd.exe"
        );
        assert_eq!(
            quote_windows_arg(r"C:\Program Files\cryptexd.exe"),
            "\"C:\\Program Files\\cryptexd.exe\""
        );
    }

    #[test]
    fn register_status_and_remove_are_idempotent_and_scoped() {
        let root = unique_root("roundtrip");
        let bin = root.join("cryptexd.exe");
        std::fs::write(&bin, b"fake").expect("bin placeholder");
        let spec = HkcuRunSpec::for_runtime(&root, &bin).expect("spec");
        let sentinel_name = format!("CryptexDevP02HkcuRunSentinel{}", std::process::id());

        // Create an unrelated sentinel value that must survive unregister of our test name.
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu.create_subkey(HKCU_RUN_SUBKEY).expect("open run");
        key.set_value(&sentinel_name, &"sentinel-do-not-delete")
            .expect("sentinel");

        register(&spec).expect("register");
        let first = status(&spec).expect("status");
        assert!(first.registered);
        assert_eq!(first.value_name, TEST_HKCU_RUN_VALUE_NAME);
        assert_eq!(
            first.command_line.as_deref(),
            Some(spec.command_line().as_str())
        );

        // Idempotent update.
        register(&spec).expect("re-register");
        let second = status(&spec).expect("status2");
        assert!(second.registered);
        assert_eq!(second.command_line, first.command_line);

        unregister(&spec).expect("unregister");
        unregister(&spec).expect("unregister again");
        let after = status(&spec).expect("status after");
        assert!(!after.registered);
        assert!(after.command_line.is_none());

        let survivor: String = key.get_value(&sentinel_name).expect("sentinel survived");
        assert_eq!(survivor, "sentinel-do-not-delete");
        key.delete_value(&sentinel_name).expect("cleanup sentinel");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn default_runtime_root_is_mode_a_hkcu_under_cryptex_dev() {
        let root = HkcuRunSpec::default_runtime_root().expect("default root");
        assert!(root.ends_with("mode-a-hkcu"));
        let text = root.to_string_lossy();
        assert!(text.contains("Cryptex-Dev"));
        assert!(text.contains("p0-2"));
        assert!(text.contains("compare"));
        assert!(!text.contains("syncthing-p0-1"));
    }
}
