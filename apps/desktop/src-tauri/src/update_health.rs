use super::{
    DaemonStop, ProbeState, default_runtime_root_path, record_daemon_stopped,
    start_daemon_for_desktop_launch, stop_supervised_daemon_for,
};
use cryptex_updater::Health;
use cryptexd::ipc::{HealthProtocolMismatch, desktop_health_ping};
use std::{env, time::Duration};

pub(super) fn owned_health(state: &ProbeState) -> Result<Health, String> {
    let pid = state
        .child
        .lock()
        .map_err(|_| "child lock")?
        .as_ref()
        .ok_or("Desktop did not start this daemon")?
        .id();
    let lock = cryptexd::instance::observe_instance(&default_runtime_root_path()?)
        .map_err(|e| e.to_string())?;
    if !lock.lock_held || lock.holder_pid != Some(pid) {
        return Err("daemon does not own the profile lock".into());
    }
    let (status, protocol) = match desktop_health_ping(Duration::from_secs(5)) {
        Ok(reply) if reply.daemon_pid == pid && reply.error.is_none() => {
            ("connected", reply.protocol_version)
        }
        Ok(_) => return Err("unexpected health responder".into()),
        Err(error) => {
            let rejection = error
                .get_ref()
                .and_then(|cause| cause.downcast_ref::<HealthProtocolMismatch>())
                .filter(|reply| reply.daemon_pid == pid)
                .ok_or_else(|| error.to_string())?;
            ("incompatible", rejection.daemon_protocol)
        }
    };
    Ok(Health {
        status: status.into(),
        desktop_pid: std::process::id(),
        daemon_pid: pid,
        desktop_protocol: cryptexd::compat::DAEMON_PROTOCOL_VERSION,
        daemon_protocol: protocol,
    })
}

pub(super) fn probe() -> Result<(), String> {
    if env::args_os().len() != 2 || env::var_os("CRYPTEXD_BIN").is_some() {
        return Err("update probes require the installed sidecar".into());
    }
    let runtime = default_runtime_root_path()?;
    let _session = cryptex_updater::Layout::current()
        .and_then(|layout| layout.probe_session())
        .map_err(|e| e.to_string())?;
    let observe = || cryptexd::instance::observe_instance(&runtime).map_err(|e| e.to_string());
    if observe()?.lock_held {
        return Err("quit the running Desktop and daemon before updating".into());
    }
    let state = ProbeState::default();
    start_daemon_for_desktop_launch(&state)?;
    if state.child.lock().map_err(|_| "child lock")?.is_none() {
        return Err("unowned daemon".into());
    }
    let result = owned_health(&state);
    #[cfg(feature = "m14-installed-update-check")]
    let result = result.and_then(|health| {
        if health.status == "incompatible" {
            super::installed_update_check::hold_live_probe_for_timeout(
                &runtime,
                health.daemon_pid,
            )?;
        }
        Ok(health)
    });
    // The updater owns this probe's process group and bounds the entire handshake.
    // Let a responsive daemon drain beyond the shorter interactive quit timeout.
    stop_supervised_daemon_for(&state, DaemonStop::UpdateProbe)?;
    record_daemon_stopped("update_probe_stop")?;
    if observe()?.lock_held {
        return Err("update daemon did not release its lock".into());
    }
    let health = result?;
    #[cfg(feature = "m14-installed-update-check")]
    if health.status == "incompatible" {
        super::installed_update_check::hold_rejected_reply(&runtime)?;
    }
    println!(
        "{}",
        serde_json::to_string(&health).map_err(|e| e.to_string())?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, process::Command, thread, time::Instant};

    #[test]
    #[ignore = "child process for the delayed clean shutdown test"]
    fn draining_daemon_fixture() {
        assert_eq!(env::var("CRYPTEX_TEST_DELAYED_DRAIN").as_deref(), Ok("1"));
        let deadline = Instant::now() + Duration::from_secs(10);
        while !std::path::Path::new("stop").exists() {
            assert!(Instant::now() < deadline, "missing stop request");
            thread::sleep(Duration::from_millis(20));
        }
        thread::sleep(Duration::from_secs(4));
        fs::write("drained", b"ok").unwrap();
    }

    #[test]
    fn update_probe_waits_for_clean_daemon_drain() {
        let root = tempfile::tempdir().unwrap();
        let child = Command::new(env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "update_health::tests::draining_daemon_fixture",
            ])
            .current_dir(root.path())
            .env("CRYPTEX_TEST_DELAYED_DRAIN", "1")
            .spawn()
            .unwrap();
        let state = ProbeState::default();
        *state.runtime_root.lock().unwrap() = Some(root.path().to_path_buf());
        *state.supervised_pid.lock().unwrap() = Some(child.id());
        *state.child.lock().unwrap() = Some(child);
        stop_supervised_daemon_for(&state, DaemonStop::UpdateProbe).unwrap();
        assert_eq!(fs::read(root.path().join("drained")).unwrap(), b"ok");
        assert!(state.supervised_pid.lock().unwrap().is_none());
    }
}
