#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! M0 Desktop foundation with temporary P0-2 Mode B developer hooks.
//!
//! These hooks exist only to observe window-close vs daemon lifetime. They do
//! not select ADR-0012 and are not product tray UX.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Manager, RunEvent, State, WindowEvent};

#[derive(Clone)]
struct ProbeState {
    child: Arc<Mutex<Option<Child>>>,
    runtime_root: Arc<Mutex<Option<PathBuf>>>,
    cryptexd_bin: Arc<Mutex<Option<PathBuf>>>,
}

#[tauri::command]
fn p0_2_runtime_root() -> Result<String, String> {
    let root = runtime_root_path()?;
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    Ok(root.display().to_string())
}

fn runtime_root_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME unset".to_string())?;
    Ok(PathBuf::from(home)
        .join("Library/Application Support/Cryptex-Dev/p0-2/compare/mode-b-desktop"))
}

#[tauri::command]
fn p0_2_default_cryptexd() -> Result<String, String> {
    if let Ok(path) = std::env::var("CRYPTEXD_BIN")
        && Path::new(&path).exists()
    {
        return Ok(path);
    }
    Err("CRYPTEXD_BIN is unset or missing; paste the cryptexd absolute path".into())
}

#[tauri::command]
fn p0_2_start_daemon(state: State<'_, ProbeState>, cryptexd: String) -> Result<u32, String> {
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
    write_supervisor_state(&runtime, "active", Some(pid), true)?;
    write_snapshot(&runtime, "daemon_started", true, Some(pid))?;
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
    let bin = state
        .cryptexd_bin
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .or_else(|| std::env::var_os("CRYPTEXD_BIN").map(PathBuf::from))
        .ok_or_else(|| "cryptexd path unknown".to_string())?;
    let output = Command::new(bin)
        .args([
            "lifecycle",
            "status",
            "--runtime-root",
            runtime.to_str().ok_or("runtime utf8")?,
        ])
        .output()
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[tauri::command]
fn p0_2_stop_daemon(state: State<'_, ProbeState>) -> Result<(), String> {
    stop_supervised_daemon(&state)?;
    Ok(())
}

#[tauri::command]
fn p0_2_show_main_window(app: AppHandle) -> Result<(), String> {
    show_main_window(&app, "p0_2_show_main_window")
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
    let daemon_pid = read_ready_pid(&runtime);
    let path = write_snapshot(&runtime, &note, window_visible, daemon_pid)?;
    Ok(path.display().to_string())
}

#[tauri::command]
fn p0_2_quit_clean(app: AppHandle, state: State<'_, ProbeState>) -> Result<(), String> {
    stop_supervised_daemon(&state)?;
    if let Ok(root) = runtime_root_path() {
        write_supervisor_state(&root, "clean_quit", None, false)?;
        write_snapshot(&root, "clean_quit", false, None)?;
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
    if let Some(runtime) = runtime {
        let _ = fs::write(runtime.join("stop"), "stop\n");
        write_supervisor_state(&runtime, "daemon_stop_requested", None, true)?;
    }
    if let Some(mut child) = state.child.lock().map_err(|e| e.to_string())?.take() {
        for _ in 0..40 {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => thread::sleep(Duration::from_millis(25)),
                Err(_) => break,
            }
        }
        let _ = child.kill();
        let _ = child.wait();
    }
    Ok(())
}

fn show_main_window(app: &AppHandle, source: &str) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window missing".to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    if let Ok(root) = runtime_root_path() {
        let daemon_pid = read_ready_pid(&root);
        write_supervisor_state(&root, "window_restored", daemon_pid, true)?;
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

fn record_window_close(_app: &AppHandle) {
    if let Ok(runtime) = runtime_root_path() {
        let daemon_pid = read_ready_pid(&runtime);
        let _ = fs::write(
            runtime.join("window-close.event"),
            format!(
                "event=window_close\nsource=tauri_close_requested\nsupervisor_alive=true\ndaemon_expected_alive=true\ndaemon_pid={}\ndesktop_pid={}\n",
                daemon_pid
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "none".into()),
                std::process::id()
            ),
        );
        let _ = write_supervisor_state(&runtime, "window_closed_hidden", daemon_pid, true);
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
    daemon_pid: Option<u32>,
) -> Result<PathBuf, String> {
    let path = runtime.join("gui-proof.snapshot");
    let body = format!(
        "note={}\ndesktop_pid={}\ndaemon_pid={}\nwindow_visible={}\nruntime_root={}\nupdated_unix_ms={}\n",
        note.replace('\n', " "),
        std::process::id(),
        daemon_pid
            .map(|p| p.to_string())
            .unwrap_or_else(|| "none".into()),
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

fn read_ready_pid(runtime: &Path) -> Option<u32> {
    fs::read_to_string(runtime.join("ready"))
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

fn start_daemon_inner(cryptexd: &str, runtime: &Path) -> Result<(u32, Child), String> {
    fs::create_dir_all(runtime).map_err(|e| e.to_string())?;
    let ready = runtime.join("ready");
    let stop = runtime.join("stop");
    let _ = fs::remove_file(&ready);
    let _ = fs::remove_file(&stop);

    let preflight = Command::new(cryptexd)
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

    let child = Command::new(cryptexd)
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
                    let bin = request_body
                        .lines()
                        .map(str::trim)
                        .find(|line| !line.is_empty())
                        .map(str::to_string)
                        .or_else(|| std::env::var("CRYPTEXD_BIN").ok());
                    match bin {
                        Some(bin) => match start_daemon_inner(&bin, &root) {
                            Ok((pid, child)) => {
                                if let Ok(mut guard) = state.child.lock() {
                                    *guard = Some(child);
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
                                    Some(pid),
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
                let _ = show_main_window(&app, "show.request");
            }

            let quit = root.join("quit.request");
            if quit.exists() {
                let _ = fs::remove_file(&quit);
                let _ = fs::write(root.join("stop"), "stop\n");
                if let Ok(mut guard) = state.child.lock()
                    && let Some(mut child) = guard.take()
                {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                let _ = write_snapshot(&root, "clean_quit_via_request_file", false, None);
                app.exit(0);
                break;
            }

            thread::sleep(Duration::from_millis(150));
        }
    });
}

fn main() {
    let probe_state = ProbeState {
        child: Arc::new(Mutex::new(None)),
        runtime_root: Arc::new(Mutex::new(None)),
        cryptexd_bin: Arc::new(Mutex::new(None)),
    };

    tauri::Builder::default()
        .manage(probe_state.clone())
        .invoke_handler(tauri::generate_handler![
            p0_2_runtime_root,
            p0_2_default_cryptexd,
            p0_2_start_daemon,
            p0_2_daemon_status,
            p0_2_stop_daemon,
            p0_2_show_main_window,
            p0_2_write_snapshot,
            p0_2_quit_clean
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                record_window_close(window.app_handle());
                let _ = window.hide();
            }
        })
        .setup(move |app| {
            let _ = runtime_root_path().and_then(|root| {
                fs::create_dir_all(&root).map_err(|e| e.to_string())?;
                write_snapshot(&root, "desktop_launched", true, None)?;
                Ok(())
            });
            spawn_control_file_watcher(app.handle().clone(), probe_state.clone());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build Cryptex Desktop")
        .run(|app_handle, event| {
            if let RunEvent::Reopen {
                has_visible_windows,
                ..
            } = event
                && !has_visible_windows
            {
                let _ = show_main_window(app_handle, "dock_reopen");
            }
        });
}
