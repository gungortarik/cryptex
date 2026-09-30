//! Test-only macOS LaunchAgent harness for P0-2 Mode A.
//!
//! This does **not** select ADR-0012. Registration is reversible and scoped to
//! Cryptex-Dev comparison experiments.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{LifecycleError, LifecycleResult};
use crate::instance::observe_instance;
use crate::paths::assert_outside_repository;
use crate::supervision::RegistrationStatus;

/// Stable test-only label. Must never be used as a production service identity.
pub const TEST_LAUNCH_AGENT_LABEL: &str = "com.cryptex.dev.p0-2.compare";

#[derive(Debug, Clone)]
pub struct LaunchAgentSpec {
    pub label: String,
    pub cryptexd_bin: PathBuf,
    pub runtime_root: PathBuf,
    pub plist_path: PathBuf,
    pub stdout_path: PathBuf,
    pub stderr_path: PathBuf,
    pub ready_file: PathBuf,
    pub stop_file: PathBuf,
}

impl LaunchAgentSpec {
    pub fn for_runtime(runtime_root: &Path, cryptexd_bin: &Path) -> LifecycleResult<Self> {
        assert_outside_repository(runtime_root)?;
        let root = runtime_root.to_path_buf();
        Ok(Self {
            label: TEST_LAUNCH_AGENT_LABEL.into(),
            cryptexd_bin: cryptexd_bin.to_path_buf(),
            runtime_root: root.clone(),
            plist_path: root.join("LaunchAgent.plist"),
            stdout_path: root.join("launchagent.stdout.log"),
            stderr_path: root.join("launchagent.stderr.log"),
            ready_file: root.join("ready"),
            stop_file: root.join("stop"),
        })
    }

    pub fn plist_body(&self) -> String {
        let program = xml_escape(&self.cryptexd_bin.display().to_string());
        let runtime = xml_escape(&self.runtime_root.display().to_string());
        let ready = xml_escape(&self.ready_file.display().to_string());
        let stop = xml_escape(&self.stop_file.display().to_string());
        let stdout = xml_escape(&self.stdout_path.display().to_string());
        let stderr = xml_escape(&self.stderr_path.display().to_string());
        let label = xml_escape(&self.label);
        // KeepAlive.SuccessfulExit=false => restart after crash, not clean exit.
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{label}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{program}</string>
    <string>lifecycle</string>
    <string>hold</string>
    <string>--runtime-root</string>
    <string>{runtime}</string>
    <string>--ready-file</string>
    <string>{ready}</string>
    <string>--stop-file</string>
    <string>{stop}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <dict>
    <key>SuccessfulExit</key>
    <false/>
  </dict>
  <key>StandardOutPath</key>
  <string>{stdout}</string>
  <key>StandardErrorPath</key>
  <string>{stderr}</string>
</dict>
</plist>
"#
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

pub fn registration_status() -> RegistrationStatus {
    // Production registration remains not configured. Test LaunchAgent presence
    // is reported separately by LaunchAgentStatus.
    RegistrationStatus::NotConfigured
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchAgentStatus {
    pub label: String,
    pub plist_present: bool,
    pub loaded: bool,
    pub daemon_lock_held: bool,
    pub daemon_pid: Option<u32>,
    pub ready_present: bool,
}

pub fn install(spec: &LaunchAgentSpec) -> LifecycleResult<()> {
    fs::create_dir_all(&spec.runtime_root)?;
    assert_outside_repository(&spec.runtime_root)?;
    let _ = fs::remove_file(&spec.ready_file);
    let _ = fs::remove_file(&spec.stop_file);
    fs::write(&spec.plist_path, spec.plist_body())?;
    Ok(())
}

pub fn bootstrap(spec: &LaunchAgentSpec) -> LifecycleResult<()> {
    if !spec.plist_path.exists() {
        return Err(LifecycleError::Attention(format!(
            "plist missing; install first: {}",
            spec.plist_path.display()
        )));
    }
    let domain = gui_domain()?;
    // Best-effort unload before load keeps the experiment idempotent.
    let target = format!("{domain}/{}", spec.label);
    let _ = Command::new("launchctl")
        .args(["bootout", &target])
        .output();
    let plist = spec
        .plist_path
        .to_str()
        .ok_or_else(|| LifecycleError::Io("plist path is not valid UTF-8".into()))?;
    let output = Command::new("launchctl")
        .args(["bootstrap", &domain, plist])
        .output()?;
    if !output.status.success() {
        return Err(LifecycleError::Attention(format!(
            "launchctl bootstrap failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

pub fn bootout(spec: &LaunchAgentSpec) -> LifecycleResult<()> {
    let domain = gui_domain()?;
    let target = format!("{domain}/{}", spec.label);
    let output = Command::new("launchctl")
        .args(["bootout", &target])
        .output()?;
    // Already-unloaded is acceptable for teardown idempotency.
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.contains("No such process")
            && !stderr.contains("Could not find specified service")
            && !stderr.contains("not found")
            && !stderr.contains("111:")
            && !stderr.contains("125:")
        {
            return Err(LifecycleError::Attention(format!(
                "launchctl bootout failed: {}",
                stderr.trim()
            )));
        }
    }
    Ok(())
}

pub fn remove(spec: &LaunchAgentSpec) -> LifecycleResult<()> {
    let _ = bootout(spec);
    if spec.plist_path.exists() {
        fs::remove_file(&spec.plist_path)?;
    }
    let _ = fs::remove_file(&spec.ready_file);
    let _ = fs::remove_file(&spec.stop_file);
    Ok(())
}

pub fn status(spec: &LaunchAgentSpec) -> LifecycleResult<LaunchAgentStatus> {
    let domain = gui_domain()?;
    let print = Command::new("launchctl")
        .args(["print", &format!("{domain}/{}", spec.label)])
        .output()?;
    let loaded = print.status.success();
    let observation = observe_instance(&spec.runtime_root)?;
    Ok(LaunchAgentStatus {
        label: spec.label.clone(),
        plist_present: spec.plist_path.exists(),
        loaded,
        daemon_lock_held: observation.lock_held,
        daemon_pid: observation.holder_pid,
        ready_present: spec.ready_file.exists(),
    })
}

pub fn wait_until_ready(spec: &LaunchAgentSpec, timeout: Duration) -> LifecycleResult<u32> {
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
        "LaunchAgent daemon did not become ready before timeout".into(),
    ))
}

pub fn request_clean_stop(spec: &LaunchAgentSpec) -> LifecycleResult<()> {
    fs::write(&spec.stop_file, "stop\n")?;
    Ok(())
}

fn gui_domain() -> LifecycleResult<String> {
    let output = Command::new("id").arg("-u").output()?;
    if !output.status.success() {
        return Err(LifecycleError::Io("failed to resolve user id".into()));
    }
    let uid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if uid.is_empty() {
        return Err(LifecycleError::Io("empty user id".into()));
    }
    Ok(format!("gui/{uid}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_contains_test_only_label_and_keepalive_policy() {
        let root = std::env::temp_dir().join("cryptex-launchagent-plist");
        let spec = LaunchAgentSpec::for_runtime(&root, Path::new("/tmp/cryptexd")).expect("spec");
        let body = spec.plist_body();
        assert!(body.contains(TEST_LAUNCH_AGENT_LABEL));
        assert!(body.contains("SuccessfulExit"));
        assert!(body.contains("lifecycle"));
        assert!(body.contains("hold"));
        assert!(!body.contains("syncthing-p0-1"));
    }
}
