//! Installed-package observation for disposable native CI accounts only.
use super::{ProbeState, default_runtime_root_path, stop_daemon_for_user_quit};
use cryptexd::ipc::{HealthProtocolMismatch, desktop_health_ping};
use std::{env, fs, time::Duration};
use tauri::{AppHandle, Manager};

pub(super) fn admit() -> Result<(), String> {
    let user = env::var(if cfg!(windows) { "USERNAME" } else { "USER" }).unwrap_or_default();
    let user = user.to_ascii_lowercase();
    let disposable = if cfg!(windows) {
        user.strip_prefix("cpxci").is_some_and(|suffix| {
            suffix.len() == 12 && suffix.bytes().all(|b| b.is_ascii_hexdigit())
        })
    } else {
        user == "runner"
    };
    let token = env::var("CRYPTEX_FILE_FIXTURE_SOCKET").map_err(|_| "missing rehearsal socket")?;
    if env::var("RUNNER_ENVIRONMENT").as_deref() != Ok("github-hosted")
        || env::var("GITHUB_ACTIONS").as_deref() != Ok("true")
        || !disposable
        || env::args_os().len() != 1
        || env::var_os("CRYPTEXD_BIN").is_some()
        || token.is_empty()
        || fs::read_to_string(default_runtime_root_path()?.join("m14-update-owner"))
            .map_err(|_| "missing temporary profile marker")?
            != token
    {
        return Err("disposable update account required".into());
    }
    Ok(())
}

fn check(app: &AppHandle) -> Result<serde_json::Value, String> {
    let state = app.state::<ProbeState>();
    let pid = state
        .child
        .lock()
        .map_err(|_| "child lock")?
        .as_ref()
        .ok_or("installed daemon was not started by Desktop")?
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
    Ok(serde_json::json!({
        "status": status, "version": app.package_info().version.to_string(),
        "desktop_pid": std::process::id(), "daemon_pid": pid,
        "desktop_protocol": cryptexd::compat::DAEMON_PROTOCOL_VERSION, "daemon_protocol": protocol,
    }))
}

pub(super) fn observe(app: AppHandle) {
    std::thread::spawn(move || {
        let result = check(&app);
        let stopped = stop_daemon_for_user_quit(&app.state::<ProbeState>(), "m14_update_stop");
        let result = stopped.and(result).and_then(|receipt| {
            let root = default_runtime_root_path()?;
            if cryptexd::instance::observe_instance(&root)
                .map_err(|e| e.to_string())?
                .lock_held
            {
                return Err("daemon lock remained held".into());
            }
            let file = fs::File::create_new(root.join("m14-update-result.json"))
                .map_err(|e| e.to_string())?;
            serde_json::to_writer(file, &receipt).map_err(|e| e.to_string())
        });
        if let Err(error) = result {
            eprintln!("Installed update observation failed: {error}");
            app.exit(2);
        } else {
            app.exit(0);
        }
    });
}
