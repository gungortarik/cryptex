#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! M0 Desktop foundation with temporary P0-2 Mode B developer hooks.
//!
//! This remains a P0-2 foundation surface. It now contains the approved Mode B
//! tray, per-user login-start controls and Desktop-owned daemon lifecycle; the
//! product UI and M0 production evidence gates are still open.

#[cfg(target_os = "windows")]
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
use tauri::RunEvent;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, State, WindowEvent};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use tauri_plugin_autostart::ManagerExt as AutoStartManagerExt;

mod background_process;
mod pilot_runtime;

fn runtime_root_path() -> Result<PathBuf, String> {
    if let Some(root) = pilot_runtime::root() {
        return Ok(root);
    }
    default_runtime_root_path()
}

fn daemon_override() -> Option<PathBuf> {
    if pilot_runtime::root().is_some() {
        None
    } else {
        std::env::var_os("CRYPTEXD_BIN").map(PathBuf::from)
    }
}

#[derive(Clone)]
struct ProbeState {
    child: Arc<Mutex<Option<Child>>>,
    runtime_root: Arc<Mutex<Option<PathBuf>>>,
    cryptexd_bin: Arc<Mutex<Option<PathBuf>>>,
    restart_attempts: Arc<Mutex<u8>>,
    restart_not_before: Arc<Mutex<Instant>>,
    pending_exit_code: Arc<Mutex<Option<i32>>>,
    supervised_pid: Arc<Mutex<Option<u32>>>,
}

#[tauri::command]
fn p0_2_runtime_root() -> Result<String, String> {
    let root = runtime_root_path()?;
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    Ok(root.display().to_string())
}

#[cfg(all(target_os = "macos", debug_assertions))]
fn default_runtime_root_path() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("HOME unset")?;
    Ok(PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("Cryptex-Dev")
        .join("p0-2")
        .join("compare")
        .join("mode-b-desktop"))
}

#[cfg(all(target_os = "macos", not(debug_assertions)))]
fn default_runtime_root_path() -> Result<PathBuf, String> {
    mac_product_runtime_root(std::env::var_os("HOME").as_deref())
}

#[cfg(all(target_os = "macos", any(not(debug_assertions), test)))]
fn mac_product_runtime_root(home: Option<&std::ffi::OsStr>) -> Result<PathBuf, String> {
    Ok(PathBuf::from(home.ok_or("HOME unset")?)
        .join("Library")
        .join("Application Support")
        .join("Cryptex")
        .join("runtime")
        .join("p0-2"))
}

#[cfg(all(target_os = "windows", debug_assertions))]
fn default_runtime_root_path() -> Result<PathBuf, String> {
    windows_runtime_root(std::env::var_os("LOCALAPPDATA").as_deref())
}

#[cfg(all(target_os = "windows", not(debug_assertions)))]
fn default_runtime_root_path() -> Result<PathBuf, String> {
    windows_product_runtime_root(std::env::var_os("LOCALAPPDATA").as_deref())
}

#[cfg(all(target_os = "windows", any(debug_assertions, test)))]
fn windows_runtime_root(local_app_data: Option<&OsStr>) -> Result<PathBuf, String> {
    Ok(PathBuf::from(local_app_data.ok_or("LOCALAPPDATA unset")?)
        .join("Cryptex-Dev")
        .join("p0-2")
        .join("compare")
        .join("mode-b-desktop"))
}

#[cfg(all(target_os = "windows", any(not(debug_assertions), test)))]
fn windows_product_runtime_root(local_app_data: Option<&OsStr>) -> Result<PathBuf, String> {
    Ok(PathBuf::from(local_app_data.ok_or("LOCALAPPDATA unset")?)
        .join("Cryptex")
        .join("runtime")
        .join("p0-2"))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn default_runtime_root_path() -> Result<PathBuf, String> {
    Err("Cryptex Desktop P0-2 runtime root is supported only on macOS and Windows".into())
}

#[tauri::command]
fn p0_2_default_cryptexd() -> Result<String, String> {
    let override_path = daemon_override();
    resolve_cryptexd_binary(
        override_path,
        &std::env::current_exe().map_err(|e| e.to_string())?,
    )
    .map(|path| path.display().to_string())
}

#[tauri::command]
fn p0_2_autostart_status(app: AppHandle) -> Result<bool, String> {
    if pilot_runtime::root().is_some() {
        return Ok(false);
    }
    app.autolaunch()
        .is_enabled()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn p0_2_autostart_enable(app: AppHandle) -> Result<(), String> {
    if pilot_runtime::root().is_some() {
        return Err("PILOT_AUTOSTART_FORBIDDEN".into());
    }
    app.autolaunch().enable().map_err(|error| error.to_string())
}

#[tauri::command]
fn p0_2_autostart_disable(app: AppHandle) -> Result<(), String> {
    if pilot_runtime::root().is_some() {
        return Err("PILOT_AUTOSTART_FORBIDDEN".into());
    }
    app.autolaunch()
        .disable()
        .map_err(|error| error.to_string())
}

fn resolve_cryptexd_binary(
    override_path: Option<PathBuf>,
    desktop_executable: &Path,
) -> Result<PathBuf, String> {
    if let Some(path) = override_path {
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "CRYPTEXD_BIN points to a missing file: {}",
            path.display()
        ));
    }

    let filename = if cfg!(target_os = "windows") {
        "cryptexd.exe"
    } else {
        "cryptexd"
    };
    let sibling = desktop_executable
        .parent()
        .ok_or_else(|| "Desktop executable has no parent directory".to_string())?
        .join(filename);
    if sibling.is_file() {
        return Ok(sibling);
    }
    Err(format!(
        "cryptexd sidecar is missing beside Desktop: {}",
        sibling.display()
    ))
}

#[tauri::command]
fn p0_2_start_daemon(state: State<'_, ProbeState>, cryptexd: String) -> Result<u32, String> {
    if pilot_runtime::root().is_some() {
        let expected = resolve_cryptexd_binary(
            None,
            &std::env::current_exe().map_err(|error| error.to_string())?,
        )?;
        if fs::canonicalize(&cryptexd).map_err(|error| error.to_string())?
            != fs::canonicalize(expected).map_err(|error| error.to_string())?
        {
            return Err("PILOT_SIDECAR_OVERRIDE_FORBIDDEN".into());
        }
    }
    {
        let existing = state.child.lock().map_err(|e| e.to_string())?;
        if existing.is_some() {
            return Err("probe daemon already supervised by this Desktop process".into());
        }
    }

    let runtime = PathBuf::from(p0_2_runtime_root()?);
    let (pid, child) = start_daemon_inner(&cryptexd, &runtime)?;
    *state.runtime_root.lock().map_err(|e| e.to_string())? = Some(runtime.clone());
    *state.cryptexd_bin.lock().map_err(|e| e.to_string())? = Some(PathBuf::from(&cryptexd));
    *state.child.lock().map_err(|e| e.to_string())? = Some(child);
    *state.supervised_pid.lock().map_err(|e| e.to_string())? = Some(pid);
    *state.restart_attempts.lock().map_err(|e| e.to_string())? = 0;
    *state.restart_not_before.lock().map_err(|e| e.to_string())? = Instant::now();
    *state.pending_exit_code.lock().map_err(|e| e.to_string())? = None;
    write_supervisor_state(&runtime, "active", Some(pid), true)?;
    write_snapshot(&runtime, "daemon_started", true, Ok(Some(pid)))?;
    Ok(pid)
}

#[tauri::command]
fn p0_2_daemon_status(state: State<'_, ProbeState>) -> Result<String, String> {
    let runtime = state
        .runtime_root
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .unwrap_or(runtime_root_path()?);
    let bin = daemon_binary(&state).ok_or_else(|| "cryptexd path unknown".to_string())?;
    query_daemon_status(&bin, &runtime)
}

#[tauri::command]
fn p0_3_ipc_health() -> Result<cryptexd::ipc::HealthResponse, String> {
    cryptexd::ipc::desktop_health_ping(Duration::from_secs(2)).map_err(|error| error.to_string())
}

#[tauri::command]
fn p0_3_wait_for_daemon_restart(
    previous_pid: u32,
    timeout_seconds: u64,
) -> Result<cryptexd::ipc::HealthResponse, String> {
    let timeout = Duration::from_secs(timeout_seconds.clamp(1, 600));
    let started = Instant::now();
    let mut last_error = None;
    while started.elapsed() < timeout {
        match cryptexd::ipc::desktop_health_ping(Duration::from_millis(250)) {
            Ok(response) if response.daemon_pid != previous_pid => return Ok(response),
            Ok(_) => thread::sleep(Duration::from_millis(250)),
            Err(error) => {
                last_error = Some(error.to_string());
                thread::sleep(Duration::from_millis(250));
            }
        }
    }
    Err(format!(
        "no replacement daemon answered IPC before the timeout; last connection result: {}",
        last_error
            .as_deref()
            .unwrap_or("the original daemon is still responding")
    ))
}

#[tauri::command]
fn p0_2_stop_daemon(state: State<'_, ProbeState>) -> Result<(), String> {
    stop_supervised_daemon(&state)?;
    Ok(())
}

#[tauri::command]
fn p0_2_show_main_window(app: AppHandle, state: State<'_, ProbeState>) -> Result<(), String> {
    show_main_window(&app, "p0_2_show_main_window", &state)
}

#[tauri::command]
fn p0_2_write_snapshot(
    state: State<'_, ProbeState>,
    app: AppHandle,
    note: String,
) -> Result<String, String> {
    let runtime = state
        .runtime_root
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .unwrap_or(runtime_root_path()?);
    fs::create_dir_all(&runtime).map_err(|e| e.to_string())?;
    let window_visible = app
        .get_webview_window("main")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    let daemon_pid = current_daemon_pid(&runtime, &state);
    let path = write_snapshot(&runtime, &note, window_visible, daemon_pid)?;
    Ok(path.display().to_string())
}

#[tauri::command]
fn p0_2_quit_clean(app: AppHandle, state: State<'_, ProbeState>) -> Result<(), String> {
    stop_supervised_daemon(&state)?;
    if let Ok(root) = runtime_root_path() {
        write_supervisor_state(&root, "clean_quit", None, false)?;
        write_snapshot(&root, "clean_quit", false, Ok(None))?;
    }
    app.exit(0);
    Ok(())
}

fn stop_supervised_daemon(state: &State<'_, ProbeState>) -> Result<(), String> {
    let runtime = state
        .runtime_root
        .lock()
        .map_err(|e| e.to_string())?
        .clone();
    if let Some(runtime) = &runtime {
        let _ = fs::write(runtime.join("stop"), "stop\n");
        write_supervisor_state(runtime, "daemon_stop_requested", None, true)?;
    }
    if let Some(mut child) = state.child.lock().map_err(|e| e.to_string())?.take() {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match child.try_wait().map_err(|error| error.to_string())? {
                Some(status) => {
                    *state.supervised_pid.lock().map_err(|e| e.to_string())? = None;
                    if pilot_runtime::root().is_some() && !status.success() {
                        return Err("PILOT_DAEMON_UNCLEAN_EXIT".into());
                    }
                    return Ok(());
                }
                None if Instant::now() < deadline => thread::sleep(Duration::from_millis(25)),
                None => {
                    child.kill().map_err(|error| error.to_string())?;
                    child.wait().map_err(|error| error.to_string())?;
                    *state.supervised_pid.lock().map_err(|e| e.to_string())? = None;
                    return Err("daemon ignored clean stop and had to be force-stopped".into());
                }
            }
        }
    }

    if let Some(runtime) = &runtime {
        let bin = daemon_binary(state).ok_or_else(|| "cryptexd path unknown".to_string())?;
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let status = query_daemon_status(&bin, runtime)?;
            if parse_live_daemon_pid(&status)?.is_none() {
                *state.supervised_pid.lock().map_err(|e| e.to_string())? = None;
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err("daemon did not release its lock after clean stop request".into());
            }
            thread::sleep(Duration::from_millis(25));
        }
    }
    Ok(())
}

#[cfg(not(debug_assertions))]
fn start_daemon_for_desktop_launch(state: &ProbeState) -> Result<u32, String> {
    let runtime = runtime_root_path()?;
    fs::create_dir_all(&runtime).map_err(|error| error.to_string())?;
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let bin = resolve_cryptexd_binary(daemon_override(), &executable)?;

    let status = query_daemon_status(&bin, &runtime)
        .map_err(|error| format!("failed to inspect existing daemon before startup: {error}"))?;
    if let Some(pid) = parse_live_daemon_pid(&status)? {
        *state.runtime_root.lock().map_err(|e| e.to_string())? = Some(runtime.clone());
        *state.cryptexd_bin.lock().map_err(|e| e.to_string())? = Some(bin);
        *state.supervised_pid.lock().map_err(|e| e.to_string())? = Some(pid);
        write_supervisor_state(&runtime, "attached_to_existing_daemon", Some(pid), true)?;
        return Ok(pid);
    }

    let (pid, child) = start_daemon_inner(
        bin.to_str()
            .ok_or_else(|| "cryptexd path is not valid UTF-8".to_string())?,
        &runtime,
    )?;
    *state.runtime_root.lock().map_err(|e| e.to_string())? = Some(runtime.clone());
    *state.cryptexd_bin.lock().map_err(|e| e.to_string())? = Some(bin);
    *state.child.lock().map_err(|e| e.to_string())? = Some(child);
    *state.supervised_pid.lock().map_err(|e| e.to_string())? = Some(pid);
    write_supervisor_state(&runtime, "active", Some(pid), true)?;
    write_snapshot(&runtime, "daemon_started_with_desktop", true, Ok(Some(pid)))?;
    Ok(pid)
}

fn build_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let open = MenuItem::with_id(app, "open", "Open Cryptex", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Cryptex", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or("default tray icon is missing")?;

    TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => {
                let state = app.state::<ProbeState>();
                if let Err(error) = show_main_window(app, "tray_menu", &state)
                    && let Ok(runtime) = runtime_root_path()
                {
                    let _ = fs::write(runtime.join("window.error"), format!("{error}\n"));
                }
            }
            "quit" => {
                let state = app.state::<ProbeState>();
                match stop_supervised_daemon(&state) {
                    Ok(()) => {
                        if let Ok(root) = runtime_root_path() {
                            let _ =
                                write_supervisor_state(&root, "clean_quit_from_tray", None, false);
                            let _ = write_snapshot(&root, "clean_quit_from_tray", false, Ok(None));
                        }
                        app.exit(0);
                    }
                    Err(error) => {
                        if let Ok(root) = runtime_root_path() {
                            let _ = fs::write(root.join("quit.error"), format!("{error}\n"));
                        }
                    }
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                let state = app.state::<ProbeState>();
                if let Err(error) = show_main_window(app, "tray_left_click", &state)
                    && let Ok(runtime) = runtime_root_path()
                {
                    let _ = fs::write(runtime.join("window.error"), format!("{error}\n"));
                }
            }
        })
        .build(app)?;
    Ok(())
}

fn show_main_window(app: &AppHandle, source: &str, state: &ProbeState) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window missing".to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    if let Ok(root) = runtime_root_path() {
        let daemon_pid = current_daemon_pid(&root, state);
        write_supervisor_state(
            &root,
            "window_restored",
            daemon_pid.as_ref().ok().copied().flatten(),
            true,
        )?;
        write_snapshot(
            &root,
            &format!("window_restored_via_{source}"),
            true,
            daemon_pid,
        )?;
        let _ = fs::write(
            root.join("window-restore.event"),
            format!("event=window_restore\nsource={source}\n"),
        );
    }
    Ok(())
}

fn record_window_close(_app: &AppHandle, state: &ProbeState) {
    if let Ok(runtime) = runtime_root_path() {
        let daemon_pid = current_daemon_pid(&runtime, state);
        let observed_pid = daemon_pid.as_ref().ok().copied().flatten();
        let (pid_text, observation_text) = match &daemon_pid {
            Ok(Some(pid)) => (pid.to_string(), "live".to_string()),
            Ok(None) => ("none".into(), "not_running".into()),
            Err(error) => (
                "unavailable".into(),
                format!("unavailable: {}", error.replace(['\r', '\n'], " ")),
            ),
        };
        let _ = fs::write(
            runtime.join("window-close.event"),
            format!(
                "event=window_close\nsource=tauri_close_requested\nsupervisor_alive=true\ndaemon_expected_alive=true\ndaemon_pid={}\ndaemon_pid_observation={}\ndesktop_pid={}\n",
                pid_text,
                observation_text,
                std::process::id()
            ),
        );
        let _ = write_supervisor_state(&runtime, "window_closed_hidden", observed_pid, true);
        let _ = write_snapshot(&runtime, "window_closed_hidden", false, daemon_pid);
    }
}

fn write_supervisor_state(
    runtime: &Path,
    state: &str,
    daemon_pid: Option<u32>,
    supervisor_alive: bool,
) -> Result<(), String> {
    let body = format!(
        "supervisor_pid={}\nstate={}\ndaemon_pid={}\nsupervisor_alive={}\nruntime_root={}\n",
        std::process::id(),
        state,
        daemon_pid
            .map(|p| p.to_string())
            .unwrap_or_else(|| "none".into()),
        supervisor_alive,
        runtime.display()
    );
    fs::write(runtime.join("desktop-supervisor.state"), body).map_err(|e| e.to_string())
}

fn write_snapshot(
    runtime: &Path,
    note: &str,
    window_visible: bool,
    daemon_pid: Result<Option<u32>, String>,
) -> Result<PathBuf, String> {
    let path = runtime.join("gui-proof.snapshot");
    let (pid_text, observation_text) = match daemon_pid {
        Ok(Some(pid)) => (pid.to_string(), "live".to_string()),
        Ok(None) => ("none".into(), "not_running".into()),
        Err(error) => (
            "unavailable".into(),
            format!("unavailable: {}", error.replace(['\r', '\n'], " ")),
        ),
    };
    let body = format!(
        "note={}\ndesktop_pid={}\ndaemon_pid={}\ndaemon_pid_observation={}\nwindow_visible={}\nruntime_root={}\nupdated_unix_ms={}\n",
        note.replace('\n', " "),
        std::process::id(),
        pid_text,
        observation_text,
        window_visible,
        runtime.display(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );
    fs::write(&path, body).map_err(|e| e.to_string())?;
    Ok(path)
}

fn daemon_binary(state: &ProbeState) -> Option<PathBuf> {
    state
        .cryptexd_bin
        .lock()
        .ok()
        .and_then(|bin| bin.clone())
        .or_else(|| std::env::var_os("CRYPTEXD_BIN").map(PathBuf::from))
}

fn query_daemon_status(cryptexd: &Path, runtime: &Path) -> Result<String, String> {
    let output = background_process::command(cryptexd)
        .args([
            "lifecycle",
            "status",
            "--runtime-root",
            runtime.to_str().ok_or("runtime utf8")?,
        ])
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn current_daemon_pid(runtime: &Path, state: &ProbeState) -> Result<Option<u32>, String> {
    let bin = daemon_binary(state).ok_or_else(|| "cryptexd path unknown".to_string())?;
    let status = query_daemon_status(&bin, runtime)?;
    parse_live_daemon_pid(&status)
}

fn parse_live_daemon_pid(status: &str) -> Result<Option<u32>, String> {
    let pid_field = status
        .lines()
        .find_map(|line| line.strip_prefix("pid="))
        .ok_or_else(|| "lifecycle status omitted live pid field".to_string())?;
    if pid_field == "none" {
        return Ok(None);
    }
    pid_field
        .parse::<u32>()
        .map(Some)
        .map_err(|_| "lifecycle status returned an invalid live pid".to_string())
}

fn start_daemon_inner(cryptexd: &str, runtime: &Path) -> Result<(u32, Child), String> {
    fs::create_dir_all(runtime).map_err(|e| e.to_string())?;
    let ready = runtime.join("ready");
    let stop = runtime.join("stop");
    let _ = fs::remove_file(&ready);
    let _ = fs::remove_file(&stop);

    let preflight = background_process::command(cryptexd)
        .args([
            "lifecycle",
            "status",
            "--runtime-root",
            runtime.to_str().ok_or("runtime utf8")?,
        ])
        .output()
        .map_err(|e| e.to_string())?;
    let preflight_out = String::from_utf8_lossy(&preflight.stdout);
    if preflight_out.contains("instance=held") {
        return Err("runtime lock already held; refusing second daemon".into());
    }

    let child = background_process::command(cryptexd)
        .args([
            "lifecycle",
            "hold",
            "--runtime-root",
            runtime.to_str().ok_or("runtime utf8")?,
            "--ready-file",
            ready.to_str().ok_or("ready utf8")?,
            "--stop-file",
            stop.to_str().ok_or("stop utf8")?,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;

    for _ in 0..100 {
        if ready.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    let pid = fs::read_to_string(&ready)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(child.id());
    Ok((pid, child))
}

fn restart_exited_daemon(state: &ProbeState, runtime: &Path) -> Result<(), String> {
    let newly_exited = {
        let mut child = state.child.lock().map_err(|e| e.to_string())?;
        match child.as_mut() {
            Some(process) => match process.try_wait().map_err(|e| e.to_string())? {
                Some(status) => {
                    child.take();
                    Some(status.code().unwrap_or(1))
                }
                None => None,
            },
            None => None,
        }
    };
    if let Some(exit_code) = newly_exited {
        *state.supervised_pid.lock().map_err(|e| e.to_string())? = None;
        *state.pending_exit_code.lock().map_err(|e| e.to_string())? = Some(exit_code);
    }
    let child_is_supervised = state.child.lock().map_err(|e| e.to_string())?.is_some();
    let expected_pid = *state.supervised_pid.lock().map_err(|e| e.to_string())?;
    if newly_exited.is_none()
        && !child_is_supervised
        && let Some(expected_pid) = expected_pid
    {
        match current_daemon_pid(runtime, state) {
            Ok(Some(actual_pid)) if actual_pid == expected_pid => return Ok(()),
            Ok(Some(actual_pid)) => {
                *state.supervised_pid.lock().map_err(|e| e.to_string())? = Some(actual_pid);
                return Ok(());
            }
            Ok(None) => {
                *state.supervised_pid.lock().map_err(|e| e.to_string())? = None;
                *state.pending_exit_code.lock().map_err(|e| e.to_string())? = Some(1);
            }
            Err(error) => return Err(format!("failed to observe attached daemon: {error}")),
        }
    }
    let exit_code = *state.pending_exit_code.lock().map_err(|e| e.to_string())?;
    let Some(exit_code) = exit_code else {
        return Ok(());
    };

    if newly_exited.is_some() {
        write_supervisor_state(runtime, "daemon_child_exited", None, true)?;
    }
    if runtime.join("stop").exists() {
        *state.pending_exit_code.lock().map_err(|e| e.to_string())? = None;
        write_supervisor_state(runtime, "daemon_stopped_cleanly", None, true)?;
        return Ok(());
    }

    let retry_at = *state.restart_not_before.lock().map_err(|e| e.to_string())?;
    if Instant::now() < retry_at {
        return Ok(());
    }

    let attempts = {
        let mut attempts = state.restart_attempts.lock().map_err(|e| e.to_string())?;
        if *attempts >= 5 {
            *state.pending_exit_code.lock().map_err(|e| e.to_string())? = None;
            write_supervisor_state(runtime, "daemon_restart_attention", None, true)?;
            fs::write(
                runtime.join("restart.error"),
                format!("daemon exited with code {exit_code}; restart limit reached\n"),
            )
            .map_err(|e| e.to_string())?;
            return Ok(());
        }
        *attempts += 1;
        *attempts
    };
    let bin = daemon_binary(state).ok_or_else(|| "cryptexd path unknown".to_string())?;
    match start_daemon_inner(
        bin.to_str()
            .ok_or_else(|| "cryptexd path is not valid UTF-8".to_string())?,
        runtime,
    ) {
        Ok((pid, child)) => {
            *state.child.lock().map_err(|e| e.to_string())? = Some(child);
            *state.supervised_pid.lock().map_err(|e| e.to_string())? = Some(pid);
            *state.restart_attempts.lock().map_err(|e| e.to_string())? = 0;
            *state.restart_not_before.lock().map_err(|e| e.to_string())? = Instant::now();
            *state.pending_exit_code.lock().map_err(|e| e.to_string())? = None;
            write_supervisor_state(runtime, "daemon_restarted", Some(pid), true)?;
            write_snapshot(runtime, "daemon_restarted_after_exit", true, Ok(Some(pid)))?;
            Ok(())
        }
        Err(error) => {
            let delay_seconds = 1_u64 << (attempts - 1).min(4);
            *state.restart_not_before.lock().map_err(|e| e.to_string())? =
                Instant::now() + Duration::from_secs(delay_seconds);
            let message =
                format!("restart attempt {attempts}/5 after exit {exit_code} failed: {error}");
            fs::write(runtime.join("start.error"), format!("{message}\n"))
                .map_err(|e| e.to_string())?;
            if attempts == 5 {
                *state.pending_exit_code.lock().map_err(|e| e.to_string())? = None;
                write_supervisor_state(runtime, "daemon_restart_attention", None, true)?;
            }
            Err(message)
        }
    }
}

fn spawn_control_file_watcher(app: AppHandle, state: ProbeState) {
    thread::spawn(move || {
        loop {
            let Ok(root) = runtime_root_path() else {
                thread::sleep(Duration::from_millis(200));
                continue;
            };

            let start = root.join("start.request");
            if start.exists() {
                let request_body = fs::read_to_string(&start).unwrap_or_default();
                let _ = fs::remove_file(&start);
                let already = state.child.lock().ok().is_some_and(|g| g.is_some());
                if !already {
                    let bin = if pilot_runtime::root().is_some() {
                        p0_2_default_cryptexd().ok()
                    } else {
                        request_body
                            .lines()
                            .map(str::trim)
                            .find(|line| !line.is_empty())
                            .map(str::to_string)
                            .or_else(|| std::env::var("CRYPTEXD_BIN").ok())
                    };
                    match bin {
                        Some(bin) => match start_daemon_inner(&bin, &root) {
                            Ok((pid, child)) => {
                                if let Ok(mut guard) = state.child.lock() {
                                    *guard = Some(child);
                                }
                                if let Ok(mut guard) = state.supervised_pid.lock() {
                                    *guard = Some(pid);
                                }
                                if let Ok(mut guard) = state.runtime_root.lock() {
                                    *guard = Some(root.clone());
                                }
                                if let Ok(mut guard) = state.cryptexd_bin.lock() {
                                    *guard = Some(PathBuf::from(&bin));
                                }
                                let _ = write_supervisor_state(&root, "active", Some(pid), true);
                                let _ = write_snapshot(
                                    &root,
                                    "daemon_started_via_request_file",
                                    true,
                                    Ok(Some(pid)),
                                );
                            }
                            Err(error) => {
                                let _ = fs::write(root.join("start.error"), format!("{error}\n"));
                            }
                        },
                        None => {
                            let _ = fs::write(
                                root.join("start.error"),
                                "cryptexd path missing in start.request and CRYPTEXD_BIN unset\n",
                            );
                        }
                    }
                } else {
                    let _ = fs::write(
                        root.join("start.error"),
                        "probe daemon already supervised by this Desktop process\n",
                    );
                }
            }

            let close = root.join("close.request");
            if close.exists() {
                let _ = fs::remove_file(&close);
                if let Some(window) = app.get_webview_window("main") {
                    // Emits WindowEvent::CloseRequested — same handler path as the
                    // macOS traffic-light close button. Used when Accessibility TCC
                    // blocks agent-driven UI clicks.
                    let _ = window.close();
                }
            }

            let show = root.join("show.request");
            if show.exists() {
                let _ = fs::remove_file(&show);
                let _ = show_main_window(&app, "show.request", &state);
            }

            let quit = root.join("quit.request");
            let pilot_quit = root.join("pilot-quit.request");
            if pilot_runtime::root().is_some() && pilot_quit.exists() {
                let _ = fs::remove_file(&pilot_quit);
                if let Err(error) = p0_2_quit_clean(app.clone(), app.state::<ProbeState>()) {
                    let _ = fs::write(root.join("pilot-quit.error"), format!("{error}\n"));
                    app.exit(2);
                }
                break;
            }
            if quit.exists() {
                let _ = fs::remove_file(&quit);
                let _ = fs::write(root.join("stop"), "stop\n");
                if let Ok(mut guard) = state.child.lock()
                    && let Some(mut child) = guard.take()
                {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                let _ = write_snapshot(&root, "clean_quit_via_request_file", false, Ok(None));
                app.exit(0);
                break;
            }

            if let Err(error) = restart_exited_daemon(&state, &root) {
                let _ = fs::write(root.join("restart.error"), format!("{error}\n"));
            }

            thread::sleep(Duration::from_millis(150));
        }
    });
}

fn main() {
    if let Err(error) = pilot_runtime::initialize() {
        eprintln!("PILOT_RUNTIME_REJECTED: {error:?}");
        std::process::exit(2);
    }
    let mut context = tauri::generate_context!();
    if pilot_runtime::root().is_some() {
        pilot_runtime::configure(context.config_mut());
    }
    let probe_state = ProbeState {
        child: Arc::new(Mutex::new(None)),
        runtime_root: Arc::new(Mutex::new(None)),
        cryptexd_bin: Arc::new(Mutex::new(None)),
        restart_attempts: Arc::new(Mutex::new(0)),
        restart_not_before: Arc::new(Mutex::new(Instant::now())),
        pending_exit_code: Arc::new(Mutex::new(None)),
        supervised_pid: Arc::new(Mutex::new(None)),
    };

    let builder =
        tauri::Builder::default().plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            let state = app.state::<ProbeState>();
            if let Err(error) = show_main_window(app, "single_instance_launch", &state)
                && let Ok(runtime) = runtime_root_path()
            {
                let _ = fs::write(runtime.join("window.error"), format!("{error}\n"));
            }
        }));

    #[cfg(target_os = "macos")]
    let builder = builder.plugin(
        tauri_plugin_autostart::Builder::new()
            .app_name("Cryptex")
            .args(["--cryptex-background"])
            .macos_launcher(tauri_plugin_autostart::MacosLauncher::LaunchAgent)
            .build(),
    );
    #[cfg(target_os = "windows")]
    let builder = builder.plugin(
        tauri_plugin_autostart::Builder::new()
            .app_name("Cryptex")
            .args(["--cryptex-background"])
            .build(),
    );

    builder
        .manage(probe_state.clone())
        .invoke_handler(tauri::generate_handler![
            p0_2_runtime_root,
            p0_2_default_cryptexd,
            p0_2_autostart_status,
            p0_2_autostart_enable,
            p0_2_autostart_disable,
            p0_2_start_daemon,
            p0_2_daemon_status,
            p0_3_ipc_health,
            p0_3_wait_for_daemon_restart,
            p0_2_stop_daemon,
            p0_2_show_main_window,
            p0_2_write_snapshot,
            p0_2_quit_clean
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let app = window.app_handle();
                record_window_close(app, &app.state::<ProbeState>());
                let _ = window.hide();
            }
        })
        .setup(move |app| {
            build_tray(app.handle())?;
            #[cfg(not(debug_assertions))]
            if let Err(error) = start_daemon_for_desktop_launch(&probe_state) {
                if let Ok(root) = runtime_root_path() {
                    let _ = fs::create_dir_all(&root);
                    let _ = fs::write(root.join("start.error"), format!("{error}\n"));
                }
            }
            let _ = runtime_root_path().and_then(|root| {
                fs::create_dir_all(&root).map_err(|e| e.to_string())?;
                let observation = current_daemon_pid(&root, &probe_state);
                let visible = app
                    .get_webview_window("main")
                    .and_then(|window| window.is_visible().ok())
                    .unwrap_or(false);
                write_snapshot(&root, "desktop_launched", visible, observation)?;
                Ok(())
            });
            if std::env::args_os().any(|argument| argument == "--cryptex-background")
                && let Some(window) = app.get_webview_window("main")
            {
                window.hide()?;
            }
            spawn_control_file_watcher(app.handle().clone(), probe_state.clone());
            Ok(())
        })
        .build(context)
        .expect("failed to build Cryptex Desktop")
        .run(|app_handle, event| {
            #[cfg(target_os = "macos")]
            {
                if let RunEvent::Reopen {
                    has_visible_windows,
                    ..
                } = event
                    && !has_visible_windows
                {
                    let _ = show_main_window(
                        app_handle,
                        "dock_reopen",
                        &app_handle.state::<ProbeState>(),
                    );
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = (app_handle, event);
            }
        });
}

#[cfg(test)]
mod sidecar_resolution_tests {
    use super::resolve_cryptexd_binary;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_root() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("cryptex-desktop-path-{stamp}"));
        fs::create_dir_all(&root).expect("mkdir");
        root
    }

    fn sidecar_name() -> &'static str {
        if cfg!(target_os = "windows") {
            "cryptexd.exe"
        } else {
            "cryptexd"
        }
    }

    #[test]
    fn packaged_sidecar_is_resolved_next_to_desktop_executable() {
        let root = unique_root();
        let desktop = root.join("cryptex-desktop");
        let sidecar = root.join(sidecar_name());
        fs::write(&sidecar, b"fixture").expect("sidecar fixture");

        assert_eq!(
            resolve_cryptexd_binary(None, &desktop).expect("sibling sidecar"),
            sidecar
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn explicit_sidecar_override_is_validated_and_takes_precedence() {
        let root = unique_root();
        let desktop = root.join("desktop");
        let override_path = root.join("custom-cryptexd");
        fs::write(&override_path, b"fixture").expect("override fixture");
        assert_eq!(
            resolve_cryptexd_binary(Some(override_path.clone()), &desktop)
                .expect("explicit override"),
            override_path
        );
        assert!(resolve_cryptexd_binary(Some(root.join("missing")), &desktop).is_err());
        let _ = fs::remove_dir_all(root);
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::{parse_live_daemon_pid, windows_product_runtime_root, windows_runtime_root};
    use std::ffi::OsStr;
    use std::path::PathBuf;

    #[test]
    fn windows_runtime_root_uses_local_app_data() {
        let root = windows_runtime_root(Some(OsStr::new(r"C:\Users\Tarik\AppData\Local")))
            .expect("Windows runtime root");

        assert_eq!(
            root,
            PathBuf::from(r"C:\Users\Tarik\AppData\Local")
                .join("Cryptex-Dev")
                .join("p0-2")
                .join("compare")
                .join("mode-b-desktop")
        );
        assert!(!root.to_string_lossy().contains("Library"));
    }

    #[test]
    fn release_runtime_root_uses_hidden_local_app_data() {
        let root = windows_product_runtime_root(Some(OsStr::new(r"C:\Users\Tarik\AppData\Local")))
            .expect("product runtime root");

        assert_eq!(
            root,
            PathBuf::from(r"C:\Users\Tarik\AppData\Local")
                .join("Cryptex")
                .join("runtime")
                .join("p0-2")
        );
    }

    #[test]
    fn missing_platform_runtime_root_is_a_typed_message() {
        assert_eq!(
            windows_runtime_root(None).unwrap_err(),
            "LOCALAPPDATA unset"
        );
    }

    #[test]
    fn live_pid_parser_ignores_recorded_pid_when_lock_is_stale() {
        let status = "pid=none\nrecorded_pid=731\ninstance=stale\nhealth=attention\n";

        assert_eq!(parse_live_daemon_pid(status).unwrap(), None);
    }

    #[test]
    fn live_pid_parser_uses_fresh_lock_observation() {
        let status = "pid=904\nrecorded_pid=731\ninstance=held\nhealth=active\n";

        assert_eq!(parse_live_daemon_pid(status).unwrap(), Some(904));
    }

    #[test]
    fn live_pid_parser_surfaces_malformed_status_instead_of_calling_it_stopped() {
        assert!(parse_live_daemon_pid("recorded_pid=731\ninstance=stale\n").is_err());
        assert!(parse_live_daemon_pid("pid=not-a-number\n").is_err());
    }
}

#[cfg(all(test, target_os = "macos"))]
mod mac_runtime_path_tests {
    use super::mac_product_runtime_root;
    use std::ffi::OsStr;
    use std::path::PathBuf;

    #[test]
    fn missing_home_is_reported_for_product_runtime() {
        assert_eq!(mac_product_runtime_root(None).unwrap_err(), "HOME unset");
    }

    #[test]
    fn release_runtime_root_uses_hidden_application_support() {
        assert_eq!(
            mac_product_runtime_root(Some(OsStr::new("/Users/tarik")))
                .expect("product runtime root"),
            PathBuf::from("/Users/tarik")
                .join("Library")
                .join("Application Support")
                .join("Cryptex")
                .join("runtime")
                .join("p0-2")
        );
    }
}
