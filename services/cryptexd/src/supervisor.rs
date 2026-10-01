//! Mode B desktop/tray-sidecar supervision candidate (process-level proof).
//!
//! This harness proves ownership/restart semantics without selecting ADR-0012
//! and without product tray UX. A Desktop window-close physical proof may use
//! the Tauri CloseRequested hide path separately.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{LifecycleError, LifecycleResult};
use crate::instance::observe_instance;
use crate::paths::assert_outside_repository;

#[derive(Debug)]
pub struct DesktopSupervisor {
    pub runtime_root: PathBuf,
    pub cryptexd_bin: PathBuf,
    pub ready_file: PathBuf,
    pub stop_file: PathBuf,
    pub supervisor_state_path: PathBuf,
    pub window_close_event_path: PathBuf,
    child: Option<Child>,
    pub restart_on_exit: bool,
}

impl DesktopSupervisor {
    pub fn new(runtime_root: &Path, cryptexd_bin: &Path) -> LifecycleResult<Self> {
        assert_outside_repository(runtime_root)?;
        fs::create_dir_all(runtime_root)?;
        Ok(Self {
            runtime_root: runtime_root.to_path_buf(),
            cryptexd_bin: cryptexd_bin.to_path_buf(),
            ready_file: runtime_root.join("ready"),
            stop_file: runtime_root.join("stop"),
            supervisor_state_path: runtime_root.join("supervisor.state"),
            window_close_event_path: runtime_root.join("window-close.event"),
            child: None,
            restart_on_exit: true,
        })
    }

    pub fn start_daemon(&mut self) -> LifecycleResult<u32> {
        let _ = fs::remove_file(&self.ready_file);
        let _ = fs::remove_file(&self.stop_file);
        let child = Command::new(&self.cryptexd_bin)
            .args([
                "lifecycle",
                "hold",
                "--runtime-root",
                self.runtime_root
                    .to_str()
                    .ok_or_else(|| LifecycleError::Io("runtime root is not valid UTF-8".into()))?,
                "--ready-file",
                self.ready_file
                    .to_str()
                    .ok_or_else(|| LifecycleError::Io("ready path is not valid UTF-8".into()))?,
                "--stop-file",
                self.stop_file
                    .to_str()
                    .ok_or_else(|| LifecycleError::Io("stop path is not valid UTF-8".into()))?,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        self.child = Some(child);
        match self.wait_ready(Duration::from_secs(5)) {
            Ok(pid) => {
                self.write_state("active", Some(pid))?;
                Ok(pid)
            }
            Err(error) => {
                let _ = self.force_kill_daemon();
                Err(error)
            }
        }
    }

    pub fn wait_ready(&mut self, timeout: Duration) -> LifecycleResult<u32> {
        let started = Instant::now();
        while started.elapsed() < timeout {
            if let Some(code) = self.poll_child_exit()? {
                return Err(LifecycleError::Attention(format!(
                    "daemon exited before ready (code {code})"
                )));
            }
            if self.ready_file.exists() {
                let contents = fs::read_to_string(&self.ready_file)?;
                if let Ok(pid) = contents.trim().parse::<u32>() {
                    return Ok(pid);
                }
            }
            thread::sleep(Duration::from_millis(25));
        }
        Err(LifecycleError::Attention(
            "desktop supervisor daemon did not become ready".into(),
        ))
    }

    pub fn poll_child_exit(&mut self) -> LifecycleResult<Option<i32>> {
        let Some(child) = self.child.as_mut() else {
            return Ok(None);
        };
        match child.try_wait()? {
            Some(status) => {
                self.write_state("child_exited", None)?;
                Ok(Some(status.code().unwrap_or(1)))
            }
            None => Ok(None),
        }
    }

    pub fn force_kill_daemon(&mut self) -> LifecycleResult<()> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
            self.write_state("forced_kill", None)?;
        }
        Ok(())
    }

    pub fn restart_daemon(&mut self) -> LifecycleResult<(u32, u128)> {
        let started = Instant::now();
        if self.child.is_some() {
            let _ = self.force_kill_daemon();
        }
        // Ensure lock is released before respawn.
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            let obs = observe_instance(&self.runtime_root)?;
            if !obs.lock_held {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        let pid = self.start_daemon()?;
        Ok((pid, started.elapsed().as_millis()))
    }

    pub fn simulate_window_close_keep_running(&self) -> LifecycleResult<()> {
        // Mode B candidate: main window closes; supervisor process remains.
        fs::write(
            &self.window_close_event_path,
            "event=window_close\nsupervisor_alive=true\ndaemon_expected_alive=true\n",
        )?;
        self.write_state("window_closed_supervisor_alive", None)?;
        Ok(())
    }

    pub fn clean_stop(&mut self) -> LifecycleResult<()> {
        fs::write(&self.stop_file, "stop\n")?;
        if let Some(mut child) = self.child.take() {
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline {
                if let Some(_status) = child.try_wait()? {
                    self.write_state("clean_shutdown", None)?;
                    return Ok(());
                }
                thread::sleep(Duration::from_millis(25));
            }
            child.kill()?;
            let _ = child.wait();
            return Err(LifecycleError::Attention(
                "daemon did not exit cleanly after stop-file".into(),
            ));
        }
        self.write_state("clean_shutdown", None)?;
        Ok(())
    }

    pub fn write_state(&self, state: &str, daemon_pid: Option<u32>) -> LifecycleResult<()> {
        let body = format!(
            "supervisor_pid={}\nstate={}\ndaemon_pid={}\nruntime_root={}\n",
            std::process::id(),
            state,
            daemon_pid
                .map(|p| p.to_string())
                .unwrap_or_else(|| "none".into()),
            self.runtime_root.display()
        );
        fs::write(&self.supervisor_state_path, body)?;
        Ok(())
    }
}

impl Drop for DesktopSupervisor {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn cryptexd_bin() -> PathBuf {
        // Unit tests in this module do not spawn the binary; integration tests cover that.
        PathBuf::from("cryptexd")
    }

    #[test]
    fn window_close_event_is_recorded_without_killing_paths() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("cryptex-mode-b-{stamp}"));
        let supervisor = DesktopSupervisor::new(&root, &cryptexd_bin()).expect("supervisor");
        supervisor
            .simulate_window_close_keep_running()
            .expect("window close");
        let event = fs::read_to_string(&supervisor.window_close_event_path).expect("event");
        assert!(event.contains("supervisor_alive=true"));
        assert!(event.contains("daemon_expected_alive=true"));
        let _ = fs::remove_dir_all(&root);
    }
}
