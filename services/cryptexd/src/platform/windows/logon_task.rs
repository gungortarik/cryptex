//! Test-only Windows per-user logon Task Scheduler harness for P0-2 Mode A.
//!
//! This does **not** select ADR-0012 or a production Windows startup primitive.
//! Registration is reversible and scoped to Cryptex-Dev comparison experiments.
//!
//! Privilege note (physical evidence `windows-mode-a-r1-20261002T052751Z`):
//! `RunLevel=LeastPrivilege` is the *runtime* identity of the task process.
//! Creating/changing the task via `schtasks /Create` required Administrator
//! membership in the interactive Windows session (Access denied without elevation).
//! Do not treat this harness as a proven no-admin registration path.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{LifecycleError, LifecycleResult};
use crate::instance::observe_instance;
use crate::paths::assert_outside_repository;

/// Stable test-only task name. Must never be used as a production service identity.
pub const TEST_LOGON_TASK_NAME: &str = "CryptexDevP02Compare";

#[derive(Debug, Clone)]
pub struct LogonTaskSpec {
    pub task_name: String,
    pub cryptexd_bin: PathBuf,
    pub runtime_root: PathBuf,
    pub xml_path: PathBuf,
    pub ready_file: PathBuf,
    pub stop_file: PathBuf,
}

impl LogonTaskSpec {
    pub fn for_runtime(runtime_root: &Path, cryptexd_bin: &Path) -> LifecycleResult<Self> {
        assert_outside_repository(runtime_root)?;
        let root = runtime_root.to_path_buf();
        Ok(Self {
            task_name: TEST_LOGON_TASK_NAME.into(),
            cryptexd_bin: cryptexd_bin.to_path_buf(),
            runtime_root: root.clone(),
            xml_path: root.join("LogonTask.xml"),
            ready_file: root.join("ready"),
            stop_file: root.join("stop"),
        })
    }

    pub fn hold_arguments(&self) -> String {
        format!(
            "lifecycle hold --runtime-root {} --ready-file {} --stop-file {}",
            quote_windows_arg(&self.runtime_root.display().to_string()),
            quote_windows_arg(&self.ready_file.display().to_string()),
            quote_windows_arg(&self.stop_file.display().to_string()),
        )
    }

    pub fn task_xml_body(&self) -> String {
        let command = xml_escape(&self.cryptexd_bin.display().to_string());
        let arguments = xml_escape(&self.hold_arguments());
        let description = xml_escape(
            "Cryptex M0 P0-2 test-only per-user logon task (Mode A compare). Not production.",
        );
        format!(
            r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>{description}</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RestartOnFailure>
      <Interval>PT1M</Interval>
      <Count>999</Count>
    </RestartOnFailure>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{command}</Command>
      <Arguments>{arguments}</Arguments>
      <WorkingDirectory>{working}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>
"#,
            working = xml_escape(&self.runtime_root.display().to_string()),
        )
    }
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn quote_windows_arg(value: &str) -> String {
    if value.contains(' ') || value.contains('\t') {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogonTaskStatus {
    pub task_name: String,
    pub xml_present: bool,
    pub registered: bool,
    pub daemon_lock_held: bool,
    pub daemon_pid: Option<u32>,
    pub ready_present: bool,
}

fn write_schtasks_xml(path: &Path, body: &str) -> LifecycleResult<()> {
    // schtasks /XML expects a UTF-16 LE file with BOM on Windows.
    let mut bytes = Vec::with_capacity(2 + body.len() * 2);
    bytes.extend_from_slice(&[0xFF, 0xFE]);
    for unit in body.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    fs::write(path, bytes)?;
    Ok(())
}
pub fn install(spec: &LogonTaskSpec) -> LifecycleResult<()> {
    fs::create_dir_all(&spec.runtime_root)?;
    assert_outside_repository(&spec.runtime_root)?;
    let _ = fs::remove_file(&spec.ready_file);
    let _ = fs::remove_file(&spec.stop_file);
    write_schtasks_xml(&spec.xml_path, &spec.task_xml_body())?;
    Ok(())
}

pub fn register(spec: &LogonTaskSpec) -> LifecycleResult<()> {
    if !spec.xml_path.exists() {
        return Err(LifecycleError::Attention(format!(
            "task XML missing; install first: {}",
            spec.xml_path.display()
        )));
    }
    let _ = unregister(spec);
    let xml = spec
        .xml_path
        .to_str()
        .ok_or_else(|| LifecycleError::Io("task XML path is not valid UTF-8".into()))?;
    let output = Command::new("schtasks")
        .args(["/Create", "/TN", &spec.task_name, "/XML", xml, "/F"])
        .output()?;
    if !output.status.success() {
        return Err(LifecycleError::Attention(format!(
            "schtasks /Create failed: {}",
            schtasks_message(&output.stderr, &output.stdout)
        )));
    }
    Ok(())
}

pub fn run_now(spec: &LogonTaskSpec) -> LifecycleResult<()> {
    let output = Command::new("schtasks")
        .args(["/Run", "/TN", &spec.task_name])
        .output()?;
    if !output.status.success() {
        return Err(LifecycleError::Attention(format!(
            "schtasks /Run failed: {}",
            schtasks_message(&output.stderr, &output.stdout)
        )));
    }
    Ok(())
}

pub fn unregister(spec: &LogonTaskSpec) -> LifecycleResult<()> {
    let output = Command::new("schtasks")
        .args(["/Delete", "/TN", &spec.task_name, "/F"])
        .output()?;
    if !output.status.success() {
        let message = schtasks_message(&output.stderr, &output.stdout);
        if is_task_missing_message(&message) {
            return Ok(());
        }
        return Err(LifecycleError::Attention(format!(
            "schtasks /Delete failed: {message}"
        )));
    }
    Ok(())
}

pub fn remove(spec: &LogonTaskSpec) -> LifecycleResult<()> {
    let _ = unregister(spec);
    if spec.xml_path.exists() {
        fs::remove_file(&spec.xml_path)?;
    }
    let _ = fs::remove_file(&spec.ready_file);
    let _ = fs::remove_file(&spec.stop_file);
    Ok(())
}

pub fn status(spec: &LogonTaskSpec) -> LifecycleResult<LogonTaskStatus> {
    let registered = task_is_registered(&spec.task_name)?;
    let observation = observe_instance(&spec.runtime_root)?;
    Ok(LogonTaskStatus {
        task_name: spec.task_name.clone(),
        xml_present: spec.xml_path.exists(),
        registered,
        daemon_lock_held: observation.lock_held,
        daemon_pid: observation.holder_pid,
        ready_present: spec.ready_file.exists(),
    })
}

pub fn wait_until_ready(spec: &LogonTaskSpec, timeout: Duration) -> LifecycleResult<u32> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if spec.ready_file.exists() {
            let contents = fs::read_to_string(&spec.ready_file)?;
            if let Ok(pid) = contents.trim().parse::<u32>() {
                return Ok(pid);
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(LifecycleError::Attention(
        "logon task daemon did not become ready before timeout".into(),
    ))
}

pub fn request_clean_stop(spec: &LogonTaskSpec) -> LifecycleResult<()> {
    fs::write(&spec.stop_file, "stop\n")?;
    Ok(())
}

fn task_is_registered(task_name: &str) -> LifecycleResult<bool> {
    let output = Command::new("schtasks")
        .args(["/Query", "/TN", task_name])
        .output()?;
    if output.status.success() {
        return Ok(true);
    }
    let message = schtasks_message(&output.stderr, &output.stdout);
    if is_task_missing_message(&message) {
        return Ok(false);
    }
    Err(LifecycleError::Attention(format!(
        "schtasks /Query failed: {message}"
    )))
}

fn schtasks_message(stderr: &[u8], stdout: &[u8]) -> String {
    let err = String::from_utf8_lossy(stderr).trim().to_string();
    if !err.is_empty() {
        return err;
    }
    String::from_utf8_lossy(stdout).trim().to_string()
}

fn is_task_missing_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("cannot find the file")
        || lower.contains("cannot find the task")
        || lower.contains("does not exist")
        || lower.contains("not found")
        || lower.contains("0x80070002")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_contains_test_only_task_policy_and_logon_trigger() {
        let root = std::env::temp_dir().join("cryptex-logon-task-xml");
        let spec =
            LogonTaskSpec::for_runtime(&root, Path::new("C:\\Cryptex-Dev\\bin\\cryptexd.exe"))
                .expect("spec");
        let body = spec.task_xml_body();
        assert!(body.contains("<LogonTrigger>"));
        assert!(body.contains("<LogonTrigger>"));
        assert!(body.contains("<RestartOnFailure>"));
        assert!(body.contains("<RunLevel>LeastPrivilege</RunLevel>"));
        assert!(body.contains("lifecycle"));
        assert!(body.contains("hold"));
        assert!(!body.contains("syncthing-p0-1"));
    }

    #[test]
    fn hold_arguments_quote_paths_with_spaces() {
        let root = std::env::temp_dir().join("cryptex logon task");
        let spec = LogonTaskSpec::for_runtime(&root, Path::new("C:\\Program Files\\cryptexd.exe"))
            .expect("spec");
        let args = spec.hold_arguments();
        assert!(args.contains("lifecycle"));
        assert!(args.contains("--runtime-root"));
    }

    #[test]
    fn schtasks_xml_starts_with_utf16_bom() {
        let root = std::env::temp_dir().join("cryptex-logon-task-bom");
        let spec = LogonTaskSpec::for_runtime(
            &root,
            Path::new("C:\\\\Cryptex-Dev\\\\bin\\\\cryptexd.exe"),
        )
        .expect("spec");
        fs::create_dir_all(&root).expect("dir");
        write_schtasks_xml(&spec.xml_path, &spec.task_xml_body()).expect("write");
        let bytes = fs::read(&spec.xml_path).expect("read");
        assert_eq!(&bytes[0..2], &[0xFF, 0xFE]);
        let _ = fs::remove_file(&spec.xml_path);
    }

    #[test]
    fn missing_task_messages_are_recognized() {
        assert!(is_task_missing_message(
            "ERROR: The system cannot find the file specified."
        ));
        assert!(!is_task_missing_message("ERROR: Access is denied."));
    }
}
