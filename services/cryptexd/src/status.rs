use std::fmt;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::LifecycleResult;
use crate::instance::InstanceObservation;
use crate::paths::status_path;
use crate::supervision::{RegistrationStatus, SupervisionStrategy, selected_supervision};

/// Runtime health vocabulary from docs/11_DAEMON_IPC_RUNTIME.md §13.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthState {
    Configured,
    Ready,
    Active,
    Verified,
    Attention,
}

impl HealthState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Configured => "configured",
            Self::Ready => "ready",
            Self::Active => "active",
            Self::Verified => "verified",
            Self::Attention => "attention",
        }
    }
}

impl fmt::Display for HealthState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceState {
    Absent,
    Stale,
    Held,
}

impl InstanceState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Stale => "stale",
            Self::Held => "held",
        }
    }
}

impl fmt::Display for InstanceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleReport {
    pub runtime_root: String,
    /// PID associated with the currently held instance lock.
    pub pid: Option<u32>,
    /// PID retained in status/lock metadata, including when it is stale.
    pub recorded_pid: Option<u32>,
    pub instance: InstanceState,
    pub health: HealthState,
    pub supervision: SupervisionStrategy,
    pub registration: RegistrationStatus,
    pub protocol_version: u32,
    pub note: String,
}

impl LifecycleReport {
    pub fn active_holder(runtime_root: &Path, pid: u32, note: impl Into<String>) -> Self {
        Self {
            runtime_root: runtime_root.display().to_string(),
            pid: Some(pid),
            recorded_pid: Some(pid),
            instance: InstanceState::Held,
            health: HealthState::Active,
            supervision: selected_supervision(),
            registration: crate::platform::current_registration_status(),
            protocol_version: crate::compat::DAEMON_PROTOCOL_VERSION,
            note: note.into(),
        }
    }

    pub fn from_observation(
        runtime_root: &Path,
        observation: &InstanceObservation,
        self_pid: Option<u32>,
        note: impl Into<String>,
    ) -> Self {
        let (instance, health) = match (observation.lock_held, observation.lock_present) {
            (true, _) => (InstanceState::Held, HealthState::Active),
            (false, true) => (InstanceState::Stale, HealthState::Attention),
            (false, false) => (InstanceState::Absent, HealthState::Configured),
        };

        Self {
            runtime_root: runtime_root.display().to_string(),
            pid: if observation.lock_held {
                self_pid.or(observation.holder_pid)
            } else {
                None
            },
            recorded_pid: observation.recorded_pid,
            instance,
            health,
            supervision: selected_supervision(),
            registration: crate::platform::current_registration_status(),
            protocol_version: crate::compat::DAEMON_PROTOCOL_VERSION,
            note: note.into(),
        }
    }

    pub fn render_kv(&self) -> String {
        format!(
            "\
runtime_root={}
pid={}
recorded_pid={}
instance={}
health={}
supervision={}
registration={}
protocol_version={}
note={}
",
            self.runtime_root,
            self.pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "none".into()),
            self.recorded_pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "none".into()),
            self.instance,
            self.health,
            self.supervision.as_str(),
            self.registration.as_str(),
            self.protocol_version,
            escape_note(&self.note),
        )
    }
}

/// Read the holder PID from `lifecycle.status` without touching `daemon.lock`.
///
/// Used as a side-channel while an exclusive instance lock is held: on Windows,
/// reading locked `daemon.lock` bytes can fail with ERROR_LOCK_VIOLATION (33).
/// Missing/malformed status yields `None` and is not treated as an I/O failure.
pub fn read_status_pid(runtime_root: &Path) -> Option<u32> {
    read_status_value(runtime_root, "pid")
}

/// Read the most recently recorded PID from lifecycle status, even when stale.
pub fn read_status_recorded_pid(runtime_root: &Path) -> Option<u32> {
    read_status_value(runtime_root, "recorded_pid").or_else(|| read_status_pid(runtime_root))
}

fn read_status_value(runtime_root: &Path, wanted_key: &str) -> Option<u32> {
    let body = fs::read_to_string(status_path(runtime_root)).ok()?;
    for line in body.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key == wanted_key {
            let trimmed = value.trim();
            if trimmed == "none" || trimmed.is_empty() {
                return None;
            }
            return trimmed.parse::<u32>().ok();
        }
    }
    None
}

pub fn write_status_file(runtime_root: &Path, report: &LifecycleReport) -> LifecycleResult<()> {
    let path = status_path(runtime_root);
    let mut body = report.render_kv();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    body.push_str(&format!("updated_unix={stamp}\n"));
    fs::write(path, body)?;
    Ok(())
}

fn escape_note(note: &str) -> String {
    note.replace(['\n', '\r'], " ")
}
