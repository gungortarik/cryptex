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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use tauri::menu::{
    AboutMetadata, HELP_SUBMENU_ID, Menu, MenuItem, PredefinedMenuItem, Submenu, WINDOW_SUBMENU_ID,
};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, RunEvent, State, WindowEvent};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use tauri_plugin_autostart::ManagerExt as AutoStartManagerExt;

mod background_process;
mod frontend_build_mode;
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
    /// Session-only capture draft / unresolved submission guard for user-initiated Quit.
    capture_session_dirty: Arc<AtomicBool>,
    /// False until the renderer successfully synchronizes guard state (fail closed).
    capture_guard_sync_ok: Arc<AtomicBool>,
    /// True while a user-initiated Quit confirm/shutdown is in progress (blocks re-entry).
    user_quit_in_progress: Arc<AtomicBool>,
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
    mac_release_probe_runtime_root(std::env::var_os("HOME").as_deref())
}

#[cfg(all(target_os = "macos", any(not(debug_assertions), test)))]
fn mac_release_probe_runtime_root(home: Option<&std::ffi::OsStr>) -> Result<PathBuf, String> {
    Ok(PathBuf::from(home.ok_or("HOME unset")?)
        .join("Library")
        .join("Application Support")
        .join("Cryptex-Dev")
        .join("runtime")
        .join("p0-2"))
}

#[cfg(all(target_os = "windows", debug_assertions))]
fn default_runtime_root_path() -> Result<PathBuf, String> {
    windows_runtime_root(std::env::var_os("LOCALAPPDATA").as_deref())
}

#[cfg(all(target_os = "windows", not(debug_assertions)))]
fn default_runtime_root_path() -> Result<PathBuf, String> {
    windows_release_probe_runtime_root(std::env::var_os("LOCALAPPDATA").as_deref())
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
fn windows_release_probe_runtime_root(local_app_data: Option<&OsStr>) -> Result<PathBuf, String> {
    Ok(PathBuf::from(local_app_data.ok_or("LOCALAPPDATA unset")?)
        .join("Cryptex-Dev")
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
    let expected = resolve_cryptexd_binary(
        daemon_override(),
        &std::env::current_exe().map_err(|error| error.to_string())?,
    )?;
    let cryptexd = validate_sidecar_request(Path::new(&cryptexd), expected)?;
    {
        let existing = state.child.lock().map_err(|e| e.to_string())?;
        if existing.is_some() {
            return Err("probe daemon already supervised by this Desktop process".into());
        }
    }

    let runtime = PathBuf::from(p0_2_runtime_root()?);
    let (pid, child) = start_daemon_inner(&cryptexd.display().to_string(), &runtime)?;
    *state.runtime_root.lock().map_err(|e| e.to_string())? = Some(runtime.clone());
    *state.cryptexd_bin.lock().map_err(|e| e.to_string())? = Some(cryptexd);
    *state.child.lock().map_err(|e| e.to_string())? = Some(child);
    *state.supervised_pid.lock().map_err(|e| e.to_string())? = Some(pid);
    *state.restart_attempts.lock().map_err(|e| e.to_string())? = 0;
    *state.restart_not_before.lock().map_err(|e| e.to_string())? = Instant::now();
    *state.pending_exit_code.lock().map_err(|e| e.to_string())? = None;
    write_supervisor_state(&runtime, "active", Some(pid), true)?;
    write_snapshot(&runtime, "daemon_started", true, Ok(Some(pid)))?;
    Ok(pid)
}

// The renderer may select no executable. Resolve authority in the backend and
// validate its displayed path before any runtime creation or process launch.
fn validate_sidecar_request(requested: &Path, expected: PathBuf) -> Result<PathBuf, String> {
    if !requested.is_absolute() || !expected.is_absolute() {
        return Err("SIDECAR_REQUEST_INVALID".into());
    }
    for path in [requested, expected.as_path()] {
        let metadata = fs::symlink_metadata(path).map_err(|_| "SIDECAR_REQUEST_INVALID")?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err("SIDECAR_REQUEST_INVALID".into());
        }
    }
    if fs::canonicalize(requested).map_err(|_| "SIDECAR_REQUEST_INVALID")?
        != fs::canonicalize(&expected).map_err(|_| "SIDECAR_REQUEST_INVALID")?
    {
        return Err("SIDECAR_REQUEST_MISMATCH".into());
    }
    Ok(expected)
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
fn p0_4_guarded_search(
    query: cryptexd::ipc::DesktopSearchQuery,
) -> Result<cryptexd::ipc::DesktopSearchResponse, String> {
    let response = cryptexd::ipc::desktop_search(query, Duration::from_secs(2))
        .map_err(|error| error.to_string())?;
    if !response.accepted {
        return Err(response
            .error
            .unwrap_or_else(|| "daemon rejected guarded search".into()));
    }
    Ok(response)
}

#[tauri::command]
fn p0_4_enroll_local_device() -> Result<cryptexd::ipc::DesktopEnrollmentResponse, String> {
    let response = cryptexd::ipc::desktop_enroll_local_device(Duration::from_secs(2))
        .map_err(|error| error.to_string())?;
    if !response.accepted {
        return Err(response
            .error
            .unwrap_or_else(|| "daemon rejected local device enrollment".into()));
    }
    Ok(response)
}

fn redact_capture_transport_error(error: &std::io::Error, resolve: bool) -> &'static str {
    match error.kind() {
        std::io::ErrorKind::TimedOut => {
            if resolve {
                "capture_resolve_transport_unavailable"
            } else {
                "capture_transport_timeout"
            }
        }
        std::io::ErrorKind::InvalidData => {
            if resolve {
                "capture_resolve_rejected"
            } else {
                "capture_rejected"
            }
        }
        _ => {
            if resolve {
                "capture_resolve_transport_unavailable"
            } else {
                "capture_transport_unavailable"
            }
        }
    }
}

#[tauri::command]
fn p0_5_capture_memory(
    body: cryptexd::ipc::DesktopCaptureBody,
) -> Result<cryptexd::ipc::DesktopCaptureResponse, String> {
    // Connect quickly; capture work uses the daemon capture deadline.
    let response = cryptexd::ipc::desktop_capture(body, Duration::from_secs(2))
        .map_err(|error| redact_capture_transport_error(&error, false).to_string())?;
    if !response.accepted {
        // Stable code only — never forward raw daemon/OS diagnostics to the renderer.
        return Err("capture_rejected".into());
    }
    Ok(response)
}

#[tauri::command]
fn p0_5_resolve_capture(
    body: cryptexd::ipc::DesktopCaptureResolveBody,
) -> Result<cryptexd::ipc::DesktopCaptureResponse, String> {
    let response = cryptexd::ipc::desktop_capture_resolve(body, Duration::from_secs(2))
        .map_err(|error| redact_capture_transport_error(&error, true).to_string())?;
    if !response.accepted {
        return Err("capture_resolve_rejected".into());
    }
    Ok(response)
}

#[tauri::command]
fn p0_5_set_capture_session_guard(
    dirty: bool,
    sync_failed: Option<bool>,
    state: State<'_, ProbeState>,
) -> Result<(), String> {
    if sync_failed.unwrap_or(false) {
        state.capture_guard_sync_ok.store(false, Ordering::Release);
        state.capture_session_dirty.store(true, Ordering::Release);
        return Ok(());
    }
    state.capture_session_dirty.store(dirty, Ordering::Release);
    state.capture_guard_sync_ok.store(true, Ordering::Release);
    Ok(())
}

const CAPTURE_QUIT_WARNING: &str = concat!(
    "Discard unsaved or unresolved capture input and quit? ",
    "Drafts are session-only and are not written to disk. ",
    "Cancel keeps Cryptex open.",
);

/// Application-menu Quit id. Must not use `PredefinedMenuItem::quit` on macOS:
/// muda maps that predefined item to native `terminate:`, which bypasses
/// `RunEvent::ExitRequested` and orphans the supervised daemon.
const APP_MENU_QUIT_ID: &str = "cryptex-app-menu-quit";
const APP_MENU_QUIT_ACCELERATOR: &str = "CmdOrCtrl+Q";
const TRAY_MENU_QUIT_ID: &str = "quit";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlannedMenuEntry {
    About,
    Services,
    Hide,
    HideOthers,
    Separator,
    CloseWindow,
    /// Explicit custom Quit routed through shared guarded shutdown.
    CustomQuit,
    /// Forbidden in Cryptex menus — muda/macOS maps this to `terminate:`.
    PredefinedQuit,
}

fn planned_macos_app_submenu() -> &'static [PlannedMenuEntry] {
    &[
        PlannedMenuEntry::About,
        PlannedMenuEntry::Separator,
        PlannedMenuEntry::Services,
        PlannedMenuEntry::Separator,
        PlannedMenuEntry::Hide,
        PlannedMenuEntry::HideOthers,
        PlannedMenuEntry::Separator,
        PlannedMenuEntry::CustomQuit,
    ]
}

fn planned_non_macos_file_submenu() -> &'static [PlannedMenuEntry] {
    &[PlannedMenuEntry::CloseWindow, PlannedMenuEntry::CustomQuit]
}

fn menu_plan_contains_predefined_quit(entries: &[PlannedMenuEntry]) -> bool {
    entries
        .iter()
        .any(|entry| matches!(entry, PlannedMenuEntry::PredefinedQuit))
}

fn menu_plan_has_custom_quit(entries: &[PlannedMenuEntry]) -> bool {
    entries
        .iter()
        .any(|entry| matches!(entry, PlannedMenuEntry::CustomQuit))
}

fn is_app_menu_quit_id(id: &str) -> bool {
    id == APP_MENU_QUIT_ID
}

fn is_tray_menu_quit_id(id: &str) -> bool {
    id == TRAY_MENU_QUIT_ID
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuListenerOwner {
    /// `Builder::on_menu_event` — application menu / Cmd+Q only.
    Application,
    /// `TrayIconBuilder::on_menu_event` — tray menu ids only.
    Tray,
}

/// Tauri 2.12 delivers tray and application menu events to every registered
/// menu listener. Each owner must accept only its own ids so Cancel cannot
/// reset the re-entry gate and allow a second prompt for the same event.
const ALL_MENU_LISTENERS: &[MenuListenerOwner] =
    &[MenuListenerOwner::Application, MenuListenerOwner::Tray];

/// Application-menu listener: only `APP_MENU_QUIT_ID`.
fn route_app_menu_event_quit(id: &str) -> Option<&'static str> {
    if is_app_menu_quit_id(id) {
        Some("clean_quit_from_app_menu")
    } else {
        None
    }
}

/// Tray-menu listener: only tray Quit id (not the application Quit id).
fn route_tray_menu_event_quit(id: &str) -> Option<&'static str> {
    if is_tray_menu_quit_id(id) {
        Some("clean_quit_from_tray")
    } else {
        None
    }
}

fn route_menu_event_quit_for_owner(owner: MenuListenerOwner, id: &str) -> Option<&'static str> {
    match owner {
        MenuListenerOwner::Application => route_app_menu_event_quit(id),
        MenuListenerOwner::Tray => route_tray_menu_event_quit(id),
    }
}

/// Notes that would be acted on if every registered listener saw `id`.
/// Must contain at most one entry for any Quit id.
fn menu_quit_notes_from_all_listeners(id: &str) -> Vec<&'static str> {
    ALL_MENU_LISTENERS
        .iter()
        .filter_map(|owner| route_menu_event_quit_for_owner(*owner, id))
        .collect()
}

fn build_cryptex_app_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let pkg_info = app.package_info();
    let config = app.config();
    let about_metadata = AboutMetadata {
        name: Some(pkg_info.name.clone()),
        version: Some(pkg_info.version.to_string()),
        copyright: config.bundle.copyright.clone(),
        authors: config.bundle.publisher.clone().map(|p| vec![p]),
        ..Default::default()
    };

    let quit_item = MenuItem::with_id(
        app,
        APP_MENU_QUIT_ID,
        "Quit Cryptex",
        true,
        Some(APP_MENU_QUIT_ACCELERATOR),
    )?;
    // Keep the forbidden variant referenced so menus cannot silently regress to it.
    debug_assert!(menu_plan_contains_predefined_quit(&[
        PlannedMenuEntry::PredefinedQuit
    ]));
    // Touch both platform plans on every OS so Clippy cannot drop menu-contract helpers.
    debug_assert!(menu_plan_has_custom_quit(planned_macos_app_submenu()));
    debug_assert!(!menu_plan_contains_predefined_quit(
        planned_macos_app_submenu()
    ));
    debug_assert!(menu_plan_has_custom_quit(planned_non_macos_file_submenu()));
    debug_assert!(!menu_plan_contains_predefined_quit(
        planned_non_macos_file_submenu()
    ));
    // Single-owner contract: tray and app Quit ids must not both fire for one event.
    debug_assert_eq!(
        menu_quit_notes_from_all_listeners(APP_MENU_QUIT_ID),
        vec!["clean_quit_from_app_menu"]
    );
    debug_assert_eq!(
        menu_quit_notes_from_all_listeners(TRAY_MENU_QUIT_ID),
        vec!["clean_quit_from_tray"]
    );
    let _ = ALL_MENU_LISTENERS;

    let window_menu = Submenu::with_id_and_items(
        app,
        WINDOW_SUBMENU_ID,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
            #[cfg(target_os = "macos")]
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;

    let help_menu = Submenu::with_id_and_items(
        app,
        HELP_SUBMENU_ID,
        "Help",
        true,
        &[
            #[cfg(not(target_os = "macos"))]
            &PredefinedMenuItem::about(app, None, Some(about_metadata.clone()))?,
        ],
    )?;

    #[cfg(target_os = "macos")]
    {
        let app_submenu = Submenu::with_items(
            app,
            pkg_info.name.clone(),
            true,
            &[
                &PredefinedMenuItem::about(app, None, Some(about_metadata))?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::services(app, None)?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::hide(app, None)?,
                &PredefinedMenuItem::hide_others(app, None)?,
                &PredefinedMenuItem::separator(app)?,
                &quit_item,
            ],
        )?;
        Menu::with_items(
            app,
            &[
                &app_submenu,
                &Submenu::with_items(
                    app,
                    "File",
                    true,
                    &[&PredefinedMenuItem::close_window(app, None)?],
                )?,
                &Submenu::with_items(
                    app,
                    "Edit",
                    true,
                    &[
                        &PredefinedMenuItem::undo(app, None)?,
                        &PredefinedMenuItem::redo(app, None)?,
                        &PredefinedMenuItem::separator(app)?,
                        &PredefinedMenuItem::cut(app, None)?,
                        &PredefinedMenuItem::copy(app, None)?,
                        &PredefinedMenuItem::paste(app, None)?,
                        &PredefinedMenuItem::select_all(app, None)?,
                    ],
                )?,
                &Submenu::with_items(
                    app,
                    "View",
                    true,
                    &[&PredefinedMenuItem::fullscreen(app, None)?],
                )?,
                &window_menu,
                &help_menu,
            ],
        )
    }

    #[cfg(not(target_os = "macos"))]
    {
        Menu::with_items(
            app,
            &[
                &Submenu::with_items(
                    app,
                    "File",
                    true,
                    &[&PredefinedMenuItem::close_window(app, None)?, &quit_item],
                )?,
                &Submenu::with_items(
                    app,
                    "Edit",
                    true,
                    &[
                        &PredefinedMenuItem::undo(app, None)?,
                        &PredefinedMenuItem::redo(app, None)?,
                        &PredefinedMenuItem::separator(app)?,
                        &PredefinedMenuItem::cut(app, None)?,
                        &PredefinedMenuItem::copy(app, None)?,
                        &PredefinedMenuItem::paste(app, None)?,
                        &PredefinedMenuItem::select_all(app, None)?,
                    ],
                )?,
                &window_menu,
                &help_menu,
            ],
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureQuitGuardView {
    /// Renderer reported a clean session and sync succeeded.
    Clean,
    /// Renderer reported dirty/unresolved/archived input.
    Dirty,
    /// Guard never synced or sync failed — must not silently quit.
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UserQuitDecision {
    Cancelled,
    Confirmed,
}

/// Shared user-initiated Quit decision.
/// Always confirms so unknown/failed/stale guard sync cannot silently discard session input.
fn decide_user_initiated_quit<F>(guard: CaptureQuitGuardView, confirm: F) -> UserQuitDecision
where
    F: FnOnce(&str) -> bool,
{
    let _ = guard;
    if confirm(CAPTURE_QUIT_WARNING) {
        UserQuitDecision::Confirmed
    } else {
        UserQuitDecision::Cancelled
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UserQuitOutcome {
    Cancelled,
    Completed,
    StopFailed,
}

/// Perform user-initiated Quit with injectable confirm / stop / exit for tests.
fn perform_user_initiated_quit<C, S, E>(
    guard: CaptureQuitGuardView,
    confirm: C,
    stop_daemon: S,
    exit: E,
) -> UserQuitOutcome
where
    C: FnOnce(&str) -> bool,
    S: FnOnce() -> Result<(), String>,
    E: FnOnce(),
{
    match decide_user_initiated_quit(guard, confirm) {
        UserQuitDecision::Cancelled => UserQuitOutcome::Cancelled,
        UserQuitDecision::Confirmed => match stop_daemon() {
            Ok(()) => {
                exit();
                UserQuitOutcome::Completed
            }
            Err(_) => UserQuitOutcome::StopFailed,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExitRequestKind {
    /// App menu Quit / Cmd+Q / equivalent: Tauri reports `code: None`.
    UserInitiated,
    /// Programmatic `AppHandle::exit` after an intentional guarded shutdown.
    Intentional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExitLifecycleAction {
    /// Prevent the raw exit and run shared confirm + clean shutdown.
    InterceptAndGuard,
    /// Prevent exit only; a Quit flow is already active (no second dialog).
    PreventDuplicate,
    /// Allow exit without prompting (intentional internal shutdown).
    Allow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandledExitOutcome {
    Cancelled,
    Completed,
    StopFailed,
    DuplicateIgnored,
    IntentionalAllowed,
}

fn exit_request_kind(code: Option<i32>) -> ExitRequestKind {
    match code {
        None => ExitRequestKind::UserInitiated,
        Some(_) => ExitRequestKind::Intentional,
    }
}

fn decide_exit_lifecycle_action(
    kind: ExitRequestKind,
    user_quit_in_progress: bool,
) -> ExitLifecycleAction {
    match kind {
        ExitRequestKind::Intentional => ExitLifecycleAction::Allow,
        ExitRequestKind::UserInitiated if user_quit_in_progress => {
            ExitLifecycleAction::PreventDuplicate
        }
        ExitRequestKind::UserInitiated => ExitLifecycleAction::InterceptAndGuard,
    }
}

/// Route `RunEvent::ExitRequested` through shared Quit confirmation when user-initiated.
/// Unauthorized exits call `prevent_exit` **before** any blocking confirm/shutdown work.
fn handle_exit_requested<P, C, S, E>(
    code: Option<i32>,
    user_quit_in_progress: &AtomicBool,
    guard: CaptureQuitGuardView,
    prevent_exit: P,
    confirm: C,
    stop_daemon: S,
    exit: E,
) -> HandledExitOutcome
where
    P: FnOnce(),
    C: FnOnce(&str) -> bool,
    S: FnOnce() -> Result<(), String>,
    E: FnOnce(),
{
    let action = decide_exit_lifecycle_action(
        exit_request_kind(code),
        user_quit_in_progress.load(Ordering::Acquire),
    );
    match action {
        ExitLifecycleAction::Allow => HandledExitOutcome::IntentionalAllowed,
        ExitLifecycleAction::PreventDuplicate => {
            prevent_exit();
            HandledExitOutcome::DuplicateIgnored
        }
        ExitLifecycleAction::InterceptAndGuard => {
            // Fail closed before modal confirm / daemon stop can block the event loop.
            prevent_exit();
            if user_quit_in_progress
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                return HandledExitOutcome::DuplicateIgnored;
            }
            let outcome = perform_user_initiated_quit(guard, confirm, stop_daemon, exit);
            match outcome {
                UserQuitOutcome::Cancelled => {
                    user_quit_in_progress.store(false, Ordering::Release);
                    HandledExitOutcome::Cancelled
                }
                UserQuitOutcome::StopFailed => {
                    user_quit_in_progress.store(false, Ordering::Release);
                    HandledExitOutcome::StopFailed
                }
                UserQuitOutcome::Completed => {
                    // Keep the gate set until process exit; the follow-up
                    // `app.exit(code)` is Intentional and must not re-prompt.
                    HandledExitOutcome::Completed
                }
            }
        }
    }
}

fn run_guarded_user_quit(app: &AppHandle, note: &str) {
    let state = app.state::<ProbeState>();
    if !try_begin_user_quit(state.inner()) {
        return;
    }
    let guard = capture_quit_guard_view(state.inner());
    let mut stop_error: Option<String> = None;
    let outcome = perform_user_initiated_quit(
        guard,
        native_confirm_discard_and_quit,
        || {
            let result = stop_daemon_for_user_quit(state.inner(), note);
            if let Err(error) = &result {
                stop_error = Some(error.clone());
            }
            result
        },
        || app.exit(0),
    );
    match outcome {
        UserQuitOutcome::Cancelled => end_user_quit(state.inner()),
        UserQuitOutcome::StopFailed => {
            end_user_quit(state.inner());
            let detail = stop_error.unwrap_or_else(|| "failed to stop supervised daemon".into());
            if let Ok(root) = runtime_root_path() {
                let _ = fs::write(
                    root.join("quit.error"),
                    format!(
                        "{detail}
"
                    ),
                );
            }
        }
        UserQuitOutcome::Completed => {}
    }
}

fn try_begin_user_quit(state: &ProbeState) -> bool {
    state
        .user_quit_in_progress
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
}

fn end_user_quit(state: &ProbeState) {
    state.user_quit_in_progress.store(false, Ordering::Release);
}

fn stop_daemon_for_user_quit(state: &ProbeState, note: &str) -> Result<(), String> {
    // Adapt State-based stopper for RunEvent / tray paths that hold ProbeState.
    stop_supervised_daemon(state)?;
    if let Ok(root) = runtime_root_path() {
        write_supervisor_state(&root, note, None, false)?;
        write_snapshot(&root, note, false, Ok(None))?;
    }
    Ok(())
}

fn capture_quit_guard_view(state: &ProbeState) -> CaptureQuitGuardView {
    if !state.capture_guard_sync_ok.load(Ordering::Acquire) {
        return CaptureQuitGuardView::Unknown;
    }
    if state.capture_session_dirty.load(Ordering::Acquire) {
        CaptureQuitGuardView::Dirty
    } else {
        CaptureQuitGuardView::Clean
    }
}

#[cfg(windows)]
fn native_confirm_discard_and_quit(message: &str) -> bool {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(
            h_wnd: *mut core::ffi::c_void,
            lp_text: *const u16,
            lp_caption: *const u16,
            u_type: u32,
        ) -> i32;
    }
    const MB_YESNO: u32 = 0x0000_0004;
    const MB_ICONWARNING: u32 = 0x0000_0030;
    const MB_DEFBUTTON2: u32 = 0x0000_0100;
    const IDYES: i32 = 6;
    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }
    let text = wide(message);
    let caption = wide("Cryptex");
    let result = unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
        )
    };
    result == IDYES
}

#[cfg(target_os = "macos")]
fn native_confirm_discard_and_quit(message: &str) -> bool {
    let script = format!(
        "button returned of (display dialog \"{}\" with title \"Cryptex\" buttons {{\"Cancel\",\"Discard and Quit\"}} default button \"Cancel\" with icon caution)",
        message.replace('\\', "\\\\").replace('"', "\\\"")
    );
    let output = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout).contains("Discard and Quit")
        }
        _ => false,
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
fn native_confirm_discard_and_quit(_message: &str) -> bool {
    false
}

#[cfg(test)]
mod capture_quit_guard_tests {
    use super::{
        CaptureQuitGuardView, UserQuitDecision, UserQuitOutcome, decide_user_initiated_quit,
        perform_user_initiated_quit, redact_capture_transport_error,
    };
    use std::cell::Cell;

    #[test]
    fn cancel_never_stops_daemon_or_exits() {
        let stopped = Cell::new(false);
        let exited = Cell::new(false);
        let outcome = perform_user_initiated_quit(
            CaptureQuitGuardView::Unknown,
            |_message| false,
            || {
                stopped.set(true);
                Ok(())
            },
            || exited.set(true),
        );
        assert_eq!(outcome, UserQuitOutcome::Cancelled);
        assert!(!stopped.get());
        assert!(!exited.get());
    }

    #[test]
    fn confirm_allows_clean_shutdown() {
        let stopped = Cell::new(false);
        let exited = Cell::new(false);
        let outcome = perform_user_initiated_quit(
            CaptureQuitGuardView::Clean,
            |_message| true,
            || {
                stopped.set(true);
                Ok(())
            },
            || exited.set(true),
        );
        assert_eq!(outcome, UserQuitOutcome::Completed);
        assert!(stopped.get());
        assert!(exited.get());
    }

    #[test]
    fn unknown_guard_cannot_silently_bypass_confirmation() {
        let asked = Cell::new(false);
        let decision = decide_user_initiated_quit(CaptureQuitGuardView::Unknown, |_message| {
            asked.set(true);
            false
        });
        assert!(asked.get());
        assert_eq!(decision, UserQuitDecision::Cancelled);

        let asked_dirty = Cell::new(false);
        let _ = decide_user_initiated_quit(CaptureQuitGuardView::Dirty, |_message| {
            asked_dirty.set(true);
            false
        });
        assert!(asked_dirty.get());

        // Even Clean requires confirm in this slice — stale false Clean cannot bypass.
        let asked_clean = Cell::new(false);
        let _ = decide_user_initiated_quit(CaptureQuitGuardView::Clean, |_message| {
            asked_clean.set(true);
            true
        });
        assert!(asked_clean.get());
    }

    #[test]
    fn cancel_keeps_protection_when_unresolved_remain_after_newer_commit() {
        // B committed, A still unresolved/archived → Dirty guard; Cancel must not stop/exit.
        let stopped = Cell::new(false);
        let exited = Cell::new(false);
        let outcome = perform_user_initiated_quit(
            CaptureQuitGuardView::Dirty,
            |_message| false,
            || {
                stopped.set(true);
                Ok(())
            },
            || exited.set(true),
        );
        assert_eq!(outcome, UserQuitOutcome::Cancelled);
        assert!(!stopped.get());
        assert!(!exited.get());
    }

    #[test]
    fn transport_errors_are_stable_redacted_codes() {
        let timeout = std::io::Error::new(std::io::ErrorKind::TimedOut, r"secret path C:\x");
        assert_eq!(
            redact_capture_transport_error(&timeout, false),
            "capture_transport_timeout"
        );
        let other = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "/var/secret.sock");
        assert_eq!(
            redact_capture_transport_error(&other, false),
            "capture_transport_unavailable"
        );
        assert_eq!(
            redact_capture_transport_error(&other, true),
            "capture_resolve_transport_unavailable"
        );
    }
}

#[cfg(test)]
#[cfg(test)]
mod exit_lifecycle_routing_tests {
    use super::{
        ALL_MENU_LISTENERS, APP_MENU_QUIT_ACCELERATOR, APP_MENU_QUIT_ID, CaptureQuitGuardView,
        ExitLifecycleAction, ExitRequestKind, HandledExitOutcome, MenuListenerOwner,
        PlannedMenuEntry, TRAY_MENU_QUIT_ID, UserQuitOutcome, decide_exit_lifecycle_action,
        exit_request_kind, handle_exit_requested, is_app_menu_quit_id,
        menu_plan_contains_predefined_quit, menu_plan_has_custom_quit,
        menu_quit_notes_from_all_listeners, perform_user_initiated_quit, planned_macos_app_submenu,
        planned_non_macos_file_submenu, route_app_menu_event_quit, route_menu_event_quit_for_owner,
        route_tray_menu_event_quit,
    };
    use std::cell::Cell;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn macos_app_submenu_uses_custom_quit_not_predefined() {
        let plan = planned_macos_app_submenu();
        assert!(menu_plan_has_custom_quit(plan));
        assert!(!menu_plan_contains_predefined_quit(plan));
        assert!(
            !plan
                .iter()
                .any(|e| matches!(e, PlannedMenuEntry::PredefinedQuit)),
            "predefined Quit must not remain as a terminate: bypass"
        );
        assert!(menu_plan_contains_predefined_quit(&[
            PlannedMenuEntry::PredefinedQuit
        ]));
        assert_eq!(APP_MENU_QUIT_ID, "cryptex-app-menu-quit");
        assert_eq!(APP_MENU_QUIT_ACCELERATOR, "CmdOrCtrl+Q");
        assert!(is_app_menu_quit_id(APP_MENU_QUIT_ID));
        assert_eq!(
            route_app_menu_event_quit(APP_MENU_QUIT_ID),
            Some("clean_quit_from_app_menu")
        );
    }

    #[test]
    fn non_macos_file_submenu_uses_custom_quit_not_predefined() {
        let plan = planned_non_macos_file_submenu();
        assert!(menu_plan_has_custom_quit(plan));
        assert!(!menu_plan_contains_predefined_quit(plan));
    }

    #[test]
    fn menu_quit_ids_have_exactly_one_listener_owner() {
        assert_eq!(
            route_app_menu_event_quit(APP_MENU_QUIT_ID),
            Some("clean_quit_from_app_menu")
        );
        assert_eq!(route_app_menu_event_quit(TRAY_MENU_QUIT_ID), None);
        assert_eq!(
            route_tray_menu_event_quit(TRAY_MENU_QUIT_ID),
            Some("clean_quit_from_tray")
        );
        assert_eq!(route_tray_menu_event_quit(APP_MENU_QUIT_ID), None);
        assert_eq!(route_app_menu_event_quit("open"), None);
        assert_eq!(route_tray_menu_event_quit("open"), None);

        assert_eq!(
            menu_quit_notes_from_all_listeners(APP_MENU_QUIT_ID),
            vec!["clean_quit_from_app_menu"]
        );
        assert_eq!(
            menu_quit_notes_from_all_listeners(TRAY_MENU_QUIT_ID),
            vec!["clean_quit_from_tray"]
        );
        assert!(menu_quit_notes_from_all_listeners("open").is_empty());
    }

    /// Simulate Tauri delivering one menu event to every registered listener.
    fn dispatch_menu_event_through_all_listeners<F>(id: &str, mut on_note: F) -> usize
    where
        F: FnMut(&'static str),
    {
        let mut acted = 0;
        for owner in ALL_MENU_LISTENERS {
            if let Some(note) = route_menu_event_quit_for_owner(*owner, id) {
                acted += 1;
                on_note(note);
            }
        }
        acted
    }

    #[test]
    fn one_tray_quit_event_through_all_listeners_cancels_once_then_can_retry() {
        let asked = Cell::new(0);
        let stopped = Cell::new(false);
        let exited = Cell::new(false);
        let gate = AtomicBool::new(false);

        let run_once = |asked: &Cell<usize>,
                        stopped: &Cell<bool>,
                        exited: &Cell<bool>,
                        gate: &AtomicBool,
                        confirm: bool| {
            if !gate
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return UserQuitOutcome::Cancelled;
            }
            let outcome = perform_user_initiated_quit(
                CaptureQuitGuardView::Dirty,
                |_message| {
                    asked.set(asked.get() + 1);
                    confirm
                },
                || {
                    stopped.set(true);
                    Ok(())
                },
                || exited.set(true),
            );
            match outcome {
                UserQuitOutcome::Cancelled | UserQuitOutcome::StopFailed => {
                    gate.store(false, Ordering::Release);
                }
                UserQuitOutcome::Completed => {}
            }
            outcome
        };

        // First physical tray Quit → Cancel: every listener sees the event, but only tray owns it.
        let mut outcomes = Vec::new();
        let acted = dispatch_menu_event_through_all_listeners(TRAY_MENU_QUIT_ID, |_note| {
            outcomes.push(run_once(&asked, &stopped, &exited, &gate, false));
        });
        assert_eq!(
            acted, 1,
            "tray Quit must not also be owned by the app listener"
        );
        assert_eq!(outcomes, vec![UserQuitOutcome::Cancelled]);
        assert_eq!(
            asked.get(),
            1,
            "Cancel must prompt exactly once for one event"
        );
        assert!(!stopped.get());
        assert!(!exited.get());
        assert!(!gate.load(Ordering::Acquire));

        // A later separate Quit click may prompt again.
        outcomes.clear();
        let acted = dispatch_menu_event_through_all_listeners(TRAY_MENU_QUIT_ID, |_note| {
            outcomes.push(run_once(&asked, &stopped, &exited, &gate, true));
        });
        assert_eq!(acted, 1);
        assert_eq!(outcomes, vec![UserQuitOutcome::Completed]);
        assert_eq!(asked.get(), 2);
        assert!(stopped.get());
        assert!(exited.get());
    }

    #[test]
    fn one_app_quit_event_through_all_listeners_is_handled_exactly_once() {
        let asked = Cell::new(0);
        let stopped = Cell::new(false);
        let exited = Cell::new(false);
        let gate = AtomicBool::new(false);

        let mut outcomes = Vec::new();
        let acted = dispatch_menu_event_through_all_listeners(APP_MENU_QUIT_ID, |_note| {
            if !gate
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return;
            }
            let outcome = perform_user_initiated_quit(
                CaptureQuitGuardView::Unknown,
                |_message| {
                    asked.set(asked.get() + 1);
                    true
                },
                || {
                    stopped.set(true);
                    Ok(())
                },
                || exited.set(true),
            );
            outcomes.push(outcome);
        });
        assert_eq!(acted, 1);
        assert_eq!(outcomes, vec![UserQuitOutcome::Completed]);
        assert_eq!(asked.get(), 1);
        assert!(stopped.get());
        assert!(exited.get());
        // App listener owns app Quit; tray listener must ignore it.
        assert_eq!(
            route_menu_event_quit_for_owner(MenuListenerOwner::Tray, APP_MENU_QUIT_ID),
            None
        );
    }

    #[test]
    fn application_menu_exit_request_is_user_initiated() {
        assert_eq!(exit_request_kind(None), ExitRequestKind::UserInitiated);
        assert_eq!(
            decide_exit_lifecycle_action(ExitRequestKind::UserInitiated, false),
            ExitLifecycleAction::InterceptAndGuard
        );
    }

    #[test]
    fn user_exit_cancel_preserves_session_and_prevents_before_confirm() {
        let in_progress = AtomicBool::new(false);
        let prevented = Cell::new(false);
        let asked = Cell::new(0);
        let stopped = Cell::new(false);
        let exited = Cell::new(false);
        let outcome = handle_exit_requested(
            None,
            &in_progress,
            CaptureQuitGuardView::Dirty,
            || prevented.set(true),
            |_message| {
                assert!(
                    prevented.get(),
                    "prevent_exit must run before blocking confirmation"
                );
                asked.set(asked.get() + 1);
                false
            },
            || {
                stopped.set(true);
                Ok(())
            },
            || exited.set(true),
        );
        assert_eq!(outcome, HandledExitOutcome::Cancelled);
        assert!(prevented.get());
        assert_eq!(asked.get(), 1);
        assert!(!stopped.get());
        assert!(!exited.get());
        assert!(!in_progress.load(Ordering::Acquire));
    }

    #[test]
    fn user_exit_confirm_stops_daemon_then_exits() {
        let in_progress = AtomicBool::new(false);
        let prevented = Cell::new(false);
        let asked = Cell::new(0);
        let stopped = Cell::new(false);
        let exited = Cell::new(false);
        let outcome = handle_exit_requested(
            None,
            &in_progress,
            CaptureQuitGuardView::Unknown,
            || prevented.set(true),
            |_message| {
                assert!(prevented.get());
                asked.set(asked.get() + 1);
                true
            },
            || {
                stopped.set(true);
                Ok(())
            },
            || exited.set(true),
        );
        assert_eq!(outcome, HandledExitOutcome::Completed);
        assert!(prevented.get());
        assert_eq!(asked.get(), 1);
        assert!(stopped.get());
        assert!(exited.get());
        assert!(in_progress.load(Ordering::Acquire));
    }

    #[test]
    fn user_exit_shutdown_failure_retains_desktop() {
        let in_progress = AtomicBool::new(false);
        let prevented = Cell::new(false);
        let exited = Cell::new(false);
        let outcome = handle_exit_requested(
            None,
            &in_progress,
            CaptureQuitGuardView::Clean,
            || prevented.set(true),
            |_message| true,
            || Err("daemon ignored clean stop and had to be force-stopped".into()),
            || exited.set(true),
        );
        assert_eq!(outcome, HandledExitOutcome::StopFailed);
        assert!(prevented.get());
        assert!(!exited.get());
        assert!(!in_progress.load(Ordering::Acquire));
    }

    #[test]
    fn repeated_user_exit_requests_do_not_reprompt() {
        let in_progress = AtomicBool::new(true);
        let prevented = Cell::new(false);
        let asked = Cell::new(0);
        let stopped = Cell::new(false);
        let exited = Cell::new(false);
        let outcome = handle_exit_requested(
            None,
            &in_progress,
            CaptureQuitGuardView::Dirty,
            || prevented.set(true),
            |_message| {
                asked.set(asked.get() + 1);
                true
            },
            || {
                stopped.set(true);
                Ok(())
            },
            || exited.set(true),
        );
        assert_eq!(outcome, HandledExitOutcome::DuplicateIgnored);
        assert!(prevented.get());
        assert_eq!(asked.get(), 0);
        assert!(!stopped.get());
        assert!(!exited.get());
    }

    #[test]
    fn intentional_final_exit_skips_prompt() {
        let in_progress = AtomicBool::new(true);
        let prevented = Cell::new(false);
        let asked = Cell::new(0);
        let stopped = Cell::new(false);
        let exited = Cell::new(false);
        let outcome = handle_exit_requested(
            Some(0),
            &in_progress,
            CaptureQuitGuardView::Dirty,
            || prevented.set(true),
            |_message| {
                asked.set(asked.get() + 1);
                true
            },
            || {
                stopped.set(true);
                Ok(())
            },
            || exited.set(true),
        );
        assert_eq!(outcome, HandledExitOutcome::IntentionalAllowed);
        assert!(!prevented.get());
        assert_eq!(asked.get(), 0);
        assert!(!stopped.get());
        assert!(!exited.get());
        assert_eq!(
            decide_exit_lifecycle_action(ExitRequestKind::Intentional, true),
            ExitLifecycleAction::Allow
        );
    }
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
    stop_supervised_daemon(state.inner())?;
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
fn p0_2_quit_clean(app: AppHandle, _state: State<'_, ProbeState>) -> Result<(), String> {
    // Shared path; StopFailed leaves Desktop up and writes quit.error.
    run_guarded_user_quit(&app, "clean_quit");
    Ok(())
}
fn stop_supervised_daemon(state: &ProbeState) -> Result<(), String> {
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
    let quit = MenuItem::with_id(app, TRAY_MENU_QUIT_ID, "Quit Cryptex", true, None::<&str>)?;
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
            id => {
                if let Some(note) = route_menu_event_quit_for_owner(MenuListenerOwner::Tray, id) {
                    run_guarded_user_quit(app, note);
                }
            }
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
                    let _ = fs::write(
                        runtime.join("window.error"),
                        format!(
                            "{error}
"
                        ),
                    );
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
                                let _ = fs::write(
                                    root.join("start.error"),
                                    format!(
                                        "{error}
"
                                    ),
                                );
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
                    let _ = fs::write(
                        root.join("pilot-quit.error"),
                        format!(
                            "{error}
"
                        ),
                    );
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
                let _ = fs::write(
                    root.join("restart.error"),
                    format!(
                        "{error}
"
                    ),
                );
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
    if let Err(error) = frontend_build_mode::ensure_current() {
        eprintln!(
            "RELEASE_FRONTEND_MODE_REJECTED: {error:?}; use the Tauri packaging command or --features tauri/custom-protocol"
        );
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
        capture_session_dirty: Arc::new(AtomicBool::new(false)),
        capture_guard_sync_ok: Arc::new(AtomicBool::new(false)),
        user_quit_in_progress: Arc::new(AtomicBool::new(false)),
    };

    let builder =
        tauri::Builder::default().plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            let state = app.state::<ProbeState>();
            if let Err(error) = show_main_window(app, "single_instance_launch", &state)
                && let Ok(runtime) = runtime_root_path()
            {
                let _ = fs::write(
                    runtime.join("window.error"),
                    format!(
                        "{error}
"
                    ),
                );
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
        .menu(build_cryptex_app_menu)
        .on_menu_event(|app, event| {
            if let Some(note) =
                route_menu_event_quit_for_owner(MenuListenerOwner::Application, event.id().as_ref())
            {
                run_guarded_user_quit(app, note);
            }
        })
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
            p0_4_guarded_search,
            p0_4_enroll_local_device,
            p0_5_capture_memory,
            p0_5_resolve_capture,
            p0_5_set_capture_session_guard,
            p0_3_wait_for_daemon_restart,
            p0_2_stop_daemon,
            p0_2_show_main_window,
            p0_2_write_snapshot,
            p0_2_quit_clean
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Close-to-tray: preserve the renderer session (draft + unresolved capture).
                // This is not a Quit path and must not call app.exit.
                api.prevent_close();
                let app = window.app_handle();
                record_window_close(app, &app.state::<ProbeState>());
                let _ = window.hide();
            }
        })
        .setup(move |app| {
            build_tray(app.handle())?;
            #[cfg(not(debug_assertions))]
            if let Err(error) = start_daemon_for_desktop_launch(&probe_state)
                && let Ok(root) = runtime_root_path()
            {
                let _ = fs::create_dir_all(&root);
                let _ = fs::write(
                    root.join("start.error"),
                    format!(
                        "{error}
"
                    ),
                );
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
        .run(|app_handle, event| match event {
            RunEvent::ExitRequested { code, api, .. } => {
                let state = app_handle.state::<ProbeState>();
                let mut stop_error: Option<String> = None;
                let outcome = handle_exit_requested(
                    code,
                    &state.user_quit_in_progress,
                    capture_quit_guard_view(state.inner()),
                    || api.prevent_exit(),
                    native_confirm_discard_and_quit,
                    || {
                        let result = stop_daemon_for_user_quit(
                            state.inner(),
                            "clean_quit_from_exit_requested",
                        );
                        if let Err(error) = &result {
                            stop_error = Some(error.clone());
                        }
                        result
                    },
                    || app_handle.exit(0),
                );
                if matches!(outcome, HandledExitOutcome::StopFailed) {
                    let detail =
                        stop_error.unwrap_or_else(|| "failed to stop supervised daemon".into());
                    if let Ok(root) = runtime_root_path() {
                        let _ = fs::write(
                            root.join("quit.error"),
                            format!(
                                "{detail}
"
                            ),
                        );
                    }
                }
            }

            #[cfg(target_os = "macos")]
            RunEvent::Reopen {
                has_visible_windows,
                ..
            } if !has_visible_windows => {
                let _ =
                    show_main_window(app_handle, "dock_reopen", &app_handle.state::<ProbeState>());
            }
            _ => {}
        });
}

#[cfg(test)]
mod sidecar_resolution_tests {
    use super::{resolve_cryptexd_binary, validate_sidecar_request};
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

    #[test]
    fn renderer_accepts_only_backend_selected_sidecar() {
        let root = unique_root();
        let expected = root.join(sidecar_name());
        let other = root.join("other-executable");
        fs::write(&expected, b"fixture").unwrap();
        fs::write(&other, b"fixture").unwrap();
        assert_eq!(
            validate_sidecar_request(&expected, expected.clone()).unwrap(),
            expected
        );
        assert_eq!(
            validate_sidecar_request(&other, expected.clone()).unwrap_err(),
            "SIDECAR_REQUEST_MISMATCH"
        );
        assert_eq!(
            validate_sidecar_request(std::path::Path::new("relative"), expected.clone())
                .unwrap_err(),
            "SIDECAR_REQUEST_INVALID"
        );
        assert_eq!(
            validate_sidecar_request(&root, expected.clone()).unwrap_err(),
            "SIDECAR_REQUEST_INVALID"
        );
        assert_eq!(
            validate_sidecar_request(&root.join("missing"), expected).unwrap_err(),
            "SIDECAR_REQUEST_INVALID"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn renderer_cannot_replace_backend_developer_override() {
        let root = unique_root();
        let desktop = root.join("desktop");
        let sibling = root.join(sidecar_name());
        let developer = root.join("backend-override");
        fs::write(&sibling, b"fixture").unwrap();
        fs::write(&developer, b"fixture").unwrap();
        let expected = resolve_cryptexd_binary(Some(developer.clone()), &desktop).unwrap();
        assert_eq!(
            validate_sidecar_request(&sibling, expected.clone()).unwrap_err(),
            "SIDECAR_REQUEST_MISMATCH"
        );
        assert_eq!(
            validate_sidecar_request(&developer, expected).unwrap(),
            developer
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn renderer_rejects_sidecar_alias() {
        let root = unique_root();
        let expected = root.join(sidecar_name());
        fs::write(&expected, b"fixture").unwrap();
        let linked = root.join("linked-executable");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&expected, &linked).unwrap();
        // A file hard link needs no Windows symlink privilege. It still must
        // not substitute another renderer-selected path for the selected role.
        #[cfg(windows)]
        std::fs::hard_link(&expected, &linked).unwrap();
        #[cfg(unix)]
        assert_eq!(
            validate_sidecar_request(&linked, expected).unwrap_err(),
            "SIDECAR_REQUEST_INVALID"
        );
        #[cfg(windows)]
        assert_eq!(
            validate_sidecar_request(&linked, expected).unwrap_err(),
            "SIDECAR_REQUEST_MISMATCH"
        );
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::{parse_live_daemon_pid, windows_release_probe_runtime_root, windows_runtime_root};
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
    fn release_runtime_root_uses_development_local_app_data() {
        let root =
            windows_release_probe_runtime_root(Some(OsStr::new(r"C:\Users\Tarik\AppData\Local")))
                .expect("release probe runtime root");

        assert_eq!(
            root,
            PathBuf::from(r"C:\Users\Tarik\AppData\Local")
                .join("Cryptex-Dev")
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
        assert_eq!(
            windows_release_probe_runtime_root(None).unwrap_err(),
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
    use super::mac_release_probe_runtime_root;
    use std::ffi::OsStr;
    use std::path::PathBuf;

    #[test]
    fn missing_home_is_reported_for_release_probe_runtime() {
        assert_eq!(
            mac_release_probe_runtime_root(None).unwrap_err(),
            "HOME unset"
        );
    }

    #[test]
    fn release_runtime_root_uses_development_application_support() {
        assert_eq!(
            mac_release_probe_runtime_root(Some(OsStr::new("/Users/tarik")))
                .expect("release probe runtime root"),
            PathBuf::from("/Users/tarik")
                .join("Library")
                .join("Application Support")
                .join("Cryptex-Dev")
                .join("runtime")
                .join("p0-2")
        );
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "windows")))]
mod default_probe_runtime_tests {
    use super::default_runtime_root_path;
    use std::path::PathBuf;

    #[test]
    fn compiled_default_uses_development_namespace_in_debug_and_release() {
        // Resolve only: do not create a runtime or mutate the process environment.
        #[cfg(target_os = "windows")]
        let base = PathBuf::from(std::env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA"));
        #[cfg(target_os = "macos")]
        let base = PathBuf::from(std::env::var_os("HOME").expect("HOME"))
            .join("Library")
            .join("Application Support");
        let expected = base.join("Cryptex-Dev");
        #[cfg(debug_assertions)]
        let expected = expected.join("p0-2").join("compare").join("mode-b-desktop");
        #[cfg(not(debug_assertions))]
        let expected = expected.join("runtime").join("p0-2");
        assert_eq!(default_runtime_root_path().unwrap(), expected);
    }
}
