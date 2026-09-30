#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! M0 Desktop foundation with temporary P0-2 Mode B developer hooks.
//!
//! These hooks exist only to observe window-close vs daemon lifetime. They do
//! not select ADR-0012 and are not product tray UX.

use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use tauri::{Manager, State, WindowEvent};

struct ProbeState {
    child: Mutex<Option<Child>>,
    runtime_root: Mutex<Option<PathBuf>>,
    cryptexd_bin: Mutex<Option<PathBuf>>,
}

#[tauri::command]
fn p0_2_runtime_root() -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME unset".to_string())?;
    let root = PathBuf::from(home)
        .join("Library/Application Support/Cryptex-Dev/p0-2/compare/mode-b-desktop");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    Ok(root.display().to_string())
}

#[tauri::command]
fn p0_2_start_daemon(state: State<'_, ProbeState>, cryptexd: String) -> Result<u32, String> {
    let runtime = PathBuf::from(p0_2_runtime_root()?);
    let ready = runtime.join("ready");
    let stop = runtime.join("stop");
    let _ = fs::remove_file(&ready);
    let _ = fs::remove_file(&stop);

    let child = Command::new(&cryptexd)
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
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    let pid = fs::read_to_string(&ready)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(child.id());

    *state.runtime_root.lock().map_err(|e| e.to_string())? = Some(runtime.clone());
    *state.cryptexd_bin.lock().map_err(|e| e.to_string())? = Some(PathBuf::from(&cryptexd));
    *state.child.lock().map_err(|e| e.to_string())? = Some(child);
    fs::write(
        runtime.join("desktop-supervisor.state"),
        format!("state=active\ndaemon_pid={pid}\nsupervisor=tauri-desktop\n"),
    )
    .map_err(|e| e.to_string())?;
    Ok(pid)
}

#[tauri::command]
fn p0_2_daemon_status(state: State<'_, ProbeState>) -> Result<String, String> {
    let runtime = state
        .runtime_root
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or_else(|| "daemon not started".to_string())?;
    let bin = state
        .cryptexd_bin
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
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
    let runtime = state
        .runtime_root
        .lock()
        .map_err(|e| e.to_string())?
        .clone();
    if let Some(runtime) = runtime {
        let _ = fs::write(runtime.join("stop"), "stop\n");
    }
    if let Some(mut child) = state.child.lock().map_err(|e| e.to_string())?.take() {
        let _ = child.wait();
    }
    Ok(())
}

fn record_window_close(app: &tauri::AppHandle) {
    let root = app
        .try_state::<ProbeState>()
        .and_then(|state| state.runtime_root.lock().ok()?.clone());
    if let Some(runtime) = root {
        let _ = fs::write(
            runtime.join("window-close.event"),
            "event=window_close\nsource=tauri_close_requested\nsupervisor_alive=true\ndaemon_expected_alive=true\n",
        );
        let _ = fs::write(
            runtime.join("desktop-supervisor.state"),
            "state=window_closed_hidden\nsupervisor_alive=true\n",
        );
    }
}

fn main() {
    tauri::Builder::default()
        .manage(ProbeState {
            child: Mutex::new(None),
            runtime_root: Mutex::new(None),
            cryptexd_bin: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            p0_2_runtime_root,
            p0_2_start_daemon,
            p0_2_daemon_status,
            p0_2_stop_daemon
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Mode B candidate proof: close main window, keep process alive.
                api.prevent_close();
                record_window_close(window.app_handle());
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run Cryptex Desktop");
}
