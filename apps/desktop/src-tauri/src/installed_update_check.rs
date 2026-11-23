//! Installed-package observation for disposable native CI accounts only.
use super::{ProbeState, default_runtime_root_path, stop_daemon_for_user_quit};
use std::{env, fs};
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
        || !(env::args_os().len() == 1
            || (env::args_os().len() == 2
                && env::args_os().nth(1).as_deref()
                    == Some(std::ffi::OsStr::new(cryptex_updater::PROBE))))
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
    let health = super::update_health::owned_health(&app.state::<ProbeState>())?;
    let mut receipt = serde_json::to_value(health).map_err(|e| e.to_string())?;
    receipt["version"] = app.package_info().version.to_string().into();
    Ok(receipt)
}

/// Keep both the native probe session and its daemon alive beyond the production deadline.
pub(super) fn hold_live_probe_for_timeout(
    root: &std::path::Path,
    daemon: u32,
) -> Result<(), String> {
    if root
        .join("m14-timeout-probe")
        .try_exists()
        .map_err(|e| e.to_string())?
    {
        fs::create_dir(root.join(format!("m14-held-live-{}-{daemon}", std::process::id())))
            .map_err(|e| e.to_string())?;
        std::thread::sleep(std::time::Duration::from_secs(120));
        return Err("timeout fixture outlived its updater".into());
    }
    Ok(())
}

/// Hold only the disposable candidate's reply. The actual updater remains
/// uninstrumented and the probe still owns its native session lock.
pub(super) fn hold_rejected_reply(root: &std::path::Path) -> Result<(), String> {
    if root
        .join("m17-fail-probe")
        .try_exists()
        .map_err(|e| e.to_string())?
    {
        return Err("disposable candidate failed to report health".into());
    }
    let armed = root.join("m14-interrupt-probe");
    if !armed.try_exists().map_err(|e| e.to_string())? {
        return Ok(());
    }
    fs::create_dir(root.join(format!("m14-held-probe-{}", std::process::id())))
        .map_err(|e| e.to_string())?;
    let started = std::time::Instant::now();
    while armed.try_exists().map_err(|e| e.to_string())? {
        if started.elapsed() >= std::time::Duration::from_secs(25) {
            return Err("timed out waiting for updater interruption".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    Err("disposable probe released after updater interruption".into())
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
            let mut pending = tempfile::NamedTempFile::new_in(&root).map_err(|e| e.to_string())?;
            serde_json::to_writer(pending.as_file_mut(), &receipt).map_err(|e| e.to_string())?;
            pending
                .persist_noclobber(root.join("m14-update-result.json"))
                .map_err(|e| e.to_string())?;
            Ok(())
        });
        if let Err(error) = result {
            eprintln!("Installed update observation failed: {error}");
            app.exit(2);
        } else {
            app.exit(0);
        }
    });
}
