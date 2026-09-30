//! Shared P0-2 comparison measurement contract.
//!
//! Supervision architecture remains unresolved (ADR-0012 open).

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{LifecycleError, LifecycleResult};
use crate::paths::assert_outside_repository;

pub const MEASUREMENT_SCHEMA: &str = "p0-2-supervision-compare-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorMode {
    NativeLaunchAgent,
    DesktopSupervisor,
}

impl SupervisorMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NativeLaunchAgent => "native_launchagent",
            Self::DesktopSupervisor => "desktop_supervisor",
        }
    }

    pub fn parse(value: &str) -> Result<Self, LifecycleError> {
        match value {
            "native_launchagent" | "mode-a" | "a" => Ok(Self::NativeLaunchAgent),
            "desktop_supervisor" | "mode-b" | "b" => Ok(Self::DesktopSupervisor),
            other => Err(LifecycleError::Usage(format!(
                "unknown supervisor mode '{other}'"
            ))),
        }
    }
}

impl fmt::Display for SupervisorMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceClass {
    Automated,
    PhysicalManual,
    NotYetVerified,
}

impl EvidenceClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Automated => "AUTOMATED",
            Self::PhysicalManual => "PHYSICAL_MANUAL",
            Self::NotYetVerified => "NOT_YET_VERIFIED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonMeasurement {
    pub schema: String,
    pub experiment_id: String,
    pub mode: SupervisorMode,
    pub branch: String,
    pub commit: String,
    pub macos_version: String,
    pub architecture: String,
    pub runtime_root: String,
    pub supervisor_mode: String,
    pub daemon_pid: Option<u32>,
    pub supervisor_pid: Option<u32>,
    pub startup_unix_ms: Option<u128>,
    pub time_to_daemon_ready_ms: Option<u128>,
    pub duplicate_launch_result: String,
    pub main_window_close_result: String,
    pub daemon_alive_after_window_close: String,
    pub forced_crash_unix_ms: Option<u128>,
    pub crash_detected: String,
    pub restart_attempted: String,
    pub restart_success: String,
    pub restart_latency_ms: Option<u128>,
    pub stale_lock_recovery: String,
    pub clean_shutdown_result: String,
    pub runtime_cleanup_result: String,
    pub login_startup_result: String,
    pub desktop_reconnect_result: String,
    pub errors: String,
    pub evidence_class_notes: String,
}

impl ComparisonMeasurement {
    pub fn blank(
        experiment_id: impl Into<String>,
        mode: SupervisorMode,
        runtime_root: &Path,
    ) -> Self {
        Self {
            schema: MEASUREMENT_SCHEMA.into(),
            experiment_id: experiment_id.into(),
            mode,
            branch: String::new(),
            commit: String::new(),
            macos_version: String::new(),
            architecture: std::env::consts::ARCH.into(),
            runtime_root: runtime_root.display().to_string(),
            supervisor_mode: mode.as_str().into(),
            daemon_pid: None,
            supervisor_pid: None,
            startup_unix_ms: None,
            time_to_daemon_ready_ms: None,
            duplicate_launch_result: EvidenceClass::NotYetVerified.as_str().into(),
            main_window_close_result: EvidenceClass::NotYetVerified.as_str().into(),
            daemon_alive_after_window_close: EvidenceClass::NotYetVerified.as_str().into(),
            forced_crash_unix_ms: None,
            crash_detected: EvidenceClass::NotYetVerified.as_str().into(),
            restart_attempted: EvidenceClass::NotYetVerified.as_str().into(),
            restart_success: EvidenceClass::NotYetVerified.as_str().into(),
            restart_latency_ms: None,
            stale_lock_recovery: EvidenceClass::NotYetVerified.as_str().into(),
            clean_shutdown_result: EvidenceClass::NotYetVerified.as_str().into(),
            runtime_cleanup_result: EvidenceClass::NotYetVerified.as_str().into(),
            login_startup_result: EvidenceClass::NotYetVerified.as_str().into(),
            desktop_reconnect_result: EvidenceClass::NotYetVerified.as_str().into(),
            errors: String::new(),
            evidence_class_notes: String::new(),
        }
    }

    pub fn render_kv(&self) -> String {
        format!(
            "\
schema={}
experiment_id={}
mode={}
branch={}
commit={}
macos_version={}
architecture={}
runtime_root={}
supervisor_mode={}
daemon_pid={}
supervisor_pid={}
startup_unix_ms={}
time_to_daemon_ready_ms={}
duplicate_launch_result={}
main_window_close_result={}
daemon_alive_after_window_close={}
forced_crash_unix_ms={}
crash_detected={}
restart_attempted={}
restart_success={}
restart_latency_ms={}
stale_lock_recovery={}
clean_shutdown_result={}
runtime_cleanup_result={}
login_startup_result={}
desktop_reconnect_result={}
errors={}
evidence_class_notes={}
",
            self.schema,
            self.experiment_id,
            self.mode,
            escape(&self.branch),
            escape(&self.commit),
            escape(&self.macos_version),
            self.architecture,
            self.runtime_root,
            self.supervisor_mode,
            opt_u32(self.daemon_pid),
            opt_u32(self.supervisor_pid),
            opt_u128(self.startup_unix_ms),
            opt_u128(self.time_to_daemon_ready_ms),
            escape(&self.duplicate_launch_result),
            escape(&self.main_window_close_result),
            escape(&self.daemon_alive_after_window_close),
            opt_u128(self.forced_crash_unix_ms),
            escape(&self.crash_detected),
            escape(&self.restart_attempted),
            escape(&self.restart_success),
            opt_u128(self.restart_latency_ms),
            escape(&self.stale_lock_recovery),
            escape(&self.clean_shutdown_result),
            escape(&self.runtime_cleanup_result),
            escape(&self.login_startup_result),
            escape(&self.desktop_reconnect_result),
            escape(&self.errors),
            escape(&self.evidence_class_notes),
        )
    }

    pub fn write(&self, path: &Path) -> LifecycleResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        assert_outside_repository(path.parent().unwrap_or(path))?;
        fs::write(path, self.render_kv())?;
        Ok(())
    }
}

pub fn unix_now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

pub fn mode_runtime_root(base: &Path, mode: SupervisorMode) -> PathBuf {
    match mode {
        SupervisorMode::NativeLaunchAgent => base.join("mode-a"),
        SupervisorMode::DesktopSupervisor => base.join("mode-b"),
    }
}

pub fn measurement_path(mode_root: &Path, experiment_id: &str) -> PathBuf {
    mode_root
        .join("measurements")
        .join(format!("{experiment_id}.kv"))
}

fn opt_u32(value: Option<u32>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "none".into())
}

fn opt_u128(value: Option<u128>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "none".into())
}

fn escape(value: &str) -> String {
    value.replace(['\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_required_measurement_keys() {
        let root = std::env::temp_dir().join("cryptex-measure-render");
        let m = ComparisonMeasurement::blank("exp-1", SupervisorMode::NativeLaunchAgent, &root);
        let kv = m.render_kv();
        for key in [
            "schema=",
            "experiment_id=",
            "duplicate_launch_result=",
            "daemon_alive_after_window_close=",
            "login_startup_result=",
            "desktop_reconnect_result=",
        ] {
            assert!(kv.contains(key), "missing {key}");
        }
    }
}
