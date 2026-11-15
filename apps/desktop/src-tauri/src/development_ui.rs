//! Normal local-certificate Mac shell. No webview exists before native binding.
use super::*;
use crate::development_launch::{Lifecycle, Quit};
use std::sync::Mutex;
use tauri::Manager;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

type OwnerSlot = Arc<Mutex<Option<Owner>>>;

pub(super) fn stage<T>(name: &'static str, result: Result<T, String>) -> Result<T, String> {
    result.inspect_err(|_| {
        eprintln!(r#"{{"developmentStage":"{name}"}}"#);
    })
}

fn explain_access() -> bool {
    rfd::MessageDialog::new()
        .set_title("Open your local memory")
        .set_description(
            "Cryptex needs access to its existing signing key in macOS Keychain. \
             On first opening, or when access changes, macOS may ask for permission. \
             Enter your Keychain password only in the macOS permission window; \
             Cryptex never asks you to type it here.\n\n\
             If you decline or access cannot be verified, Cryptex will stop opening. \
             You can open it again after resolving access.\n\n\
             Closing the main window keeps Cryptex running in the menu bar. \
             Choose Quit Cryptex there, or press Command-Q, to exit.",
        )
        .set_buttons(rfd::MessageButtons::OkCancelCustom(
            "Open local memory".into(),
            "Quit".into(),
        ))
        .show()
        == rfd::MessageDialogResult::Custom("Open local memory".into())
}

fn show_failure(cleanup: bool) {
    let (title, message) = if cleanup {
        (
            "Cryptex could not confirm a complete shutdown",
            "The local service did not confirm that it stopped safely. \
             Cryptex will close with an error. Please report this shutdown error before reopening Cryptex.",
        )
    } else {
        (
            "Cryptex could not open safely",
            "The existing profile or secure connection could not be verified. \
             If macOS asked for Keychain access, check that permission before opening Cryptex again. \
             This message does not mean that your password was incorrect.\n\n\
             This development app requires an existing initialized profile. \
             It will not create a replacement identity or bypass verification.",
        )
    };
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_level(rfd::MessageLevel::Error)
        .show();
}

fn close_owner(slot: &OwnerSlot) -> Result<(), String> {
    // Never hold this lock while waiting for the worker/native shutdown.
    let owner = slot.lock().map_err(|_| "development_owner_lock")?.take();
    match owner {
        Some(owner) => stage("close", owner.close()),
        None => Ok(()),
    }
}

fn reopen(app: &tauri::AppHandle) {
    // A second launch during the explanation/startup has no window yet. The
    // primary setup will show it; forwarded argv/cwd never select a profile.
    if let Some(window) = app.get_webview_window("main")
        && (window.unminimize().is_err() || window.show().is_err() || window.set_focus().is_err())
    {
        eprintln!(r#"{{"developmentStage":"window_reopen"}}"#);
    }
}

fn tray(app: &tauri::AppHandle) -> Result<(), String> {
    let build = || -> Result<(), Box<dyn std::error::Error>> {
        let open = MenuItem::with_id(app, "open", "Open Cryptex", true, None::<&str>)?;
        let quit = MenuItem::with_id(
            app,
            crate::TRAY_MENU_QUIT_ID,
            "Quit Cryptex",
            true,
            None::<&str>,
        )?;
        let menu = Menu::with_items(app, &[&open, &quit])?;
        let icon = app
            .default_window_icon()
            .cloned()
            .ok_or("development_tray_icon")?;
        TrayIconBuilder::new()
            .icon(icon)
            .tooltip("Cryptex")
            .menu(&menu)
            .show_menu_on_left_click(false)
            .on_menu_event(|app, event| match event.id.as_ref() {
                "open" => reopen(app),
                crate::TRAY_MENU_QUIT_ID => app.exit(0),
                _ => {}
            })
            .on_tray_icon_event(|tray, event| {
                if matches!(
                    event,
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    }
                ) {
                    reopen(tray.app_handle());
                }
            })
            .build(app)?;
        Ok(())
    };
    build().map_err(|_| "development_tray".into())
}

pub(super) fn run(runtime: PathBuf) -> Result<(), String> {
    let result = run_inner(runtime);
    if result.is_err() {
        show_failure(false);
    }
    result
}

fn run_inner(runtime: PathBuf) -> Result<(), String> {
    // Snapshot only: no listener, child, credentials or grant. Tauri needs the
    // authenticated context to select the singleton before application setup.
    let content = stage(
        "content_preflight",
        cryptexd::local_signed_startup::verified_desktop_content(Duration::from_secs(15))
            .map_err(|error| error.to_string()),
    )?;
    let (mut context, binding) = stage("content", install_context(content, &runtime))?;
    let mut window = context
        .config()
        .app
        .windows
        .first()
        .cloned()
        .ok_or("development_window")?;
    window.title = "Cryptex".into();
    context.config_mut().app.windows.clear();
    let owner: OwnerSlot = Arc::new(Mutex::new(None));
    let setup_owner = Arc::clone(&owner);
    let app = tauri::Builder::default()
        // This is window convenience, never an authentication/ownership check.
        // The native daemon still enforces its independent instance/profile pins.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| reopen(app)))
        .plugin(tauri_plugin_opener::init())
        .manage(Mutex::new(Lifecycle::default()))
        .manage(DevelopmentDraft::default())
        .menu(crate::build_cryptex_app_menu)
        .on_menu_event(|app, event| {
            if crate::is_app_menu_quit_id(event.id().as_ref()) {
                app.exit(0);
            }
        })
        .invoke_handler(|invoke| {
            let app = invoke.message.webview_ref().app_handle().clone();
            let state = app.state::<Mutex<Lifecycle>>();
            let lifecycle = state.lock();
            // Hold the lifecycle lock through dispatch. Work accepted earlier
            // belongs to the owner's existing bounded shutdown/drain contract.
            // Guard synchronization is bookkeeping, not a domain operation.
            if invoke.message.command() != "p0_5_set_capture_session_guard"
                && !lifecycle.as_ref().is_ok_and(|state| state.accepts_work())
            {
                invoke.resolver.reject("development_not_ready");
                return true;
            }
            let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
                m8_native_profile_status,
                m8_new_bootstrap_transaction_id,
                m8_read_bootstrap_brain_identity,
                m8_read_transport_device_id,
                m8_create_bootstrap_offer,
                m8_create_bootstrap_proof,
                m8_prepare_peer_approval,
                m8_confirm_peer_approval,
                m8_cancel_peer_approval,
                m8_complete_peer_approval,
                m8_prepare_owner_security_operation,
                m8_confirm_owner_security_operation,
                m8_cancel_owner_security_operation,
                p0_5_set_capture_session_guard,
                crate::p0_3_ipc_health,
                crate::p0_4_guarded_search,
                crate::p0_5_guarded_timeline,
                crate::p0_5_guarded_event_detail,
                crate::p0_5_capture_memory,
                crate::p0_5_resolve_capture,
                crate::p0_6_materialize_and_open_vault,
            ];
            handler(invoke)
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if window.hide().is_err() {
                    eprintln!(r#"{{"developmentStage":"window_hide"}}"#);
                }
            }
        })
        .setup(move |app| {
            // This is information, not Keychain authorization. Do not cache it
            // as proof of access. Each primary launch explains possible OS prompts.
            if !explain_access() {
                app.handle().exit(0);
                return Ok(());
            }
            let setup = (|| -> Result<(), String> {
                let state = app.state::<Mutex<Lifecycle>>();
                if *state.lock().map_err(|_| "development_lifecycle_lock")? != Lifecycle::Opening {
                    return Ok(());
                }
                // Fresh native deadline starts after the human explanation.
                let (started, content) = stage(
                    "start",
                    Owner::start_profile(&runtime, ContentProfile::ExistingProfile),
                )?;
                let client = started.client.clone();
                *setup_owner.lock().map_err(|_| "development_owner_lock")? = Some(started);
                if content.binding() != &binding {
                    return Err("development_content_changed".into());
                }
                stage("bind", client.call(|reply| Work::Bind(binding, reply)))?;
                stage("ping", client.call(Work::Ping))?;
                app.manage(client);
                // Do not create a hideable window unless its reopen path exists.
                tray(app.handle())?;
                if !state
                    .lock()
                    .map_err(|_| "development_lifecycle_lock")?
                    .ready()
                {
                    return Err("development_startup_cancelled".into());
                }
                tauri::WebviewWindowBuilder::from_config(app, &window)
                    .map_err(|_| "development_window")?
                    .incognito(true)
                    .on_navigation(|url| {
                        url.scheme() == "tauri" && url.host_str() == Some("localhost")
                    })
                    .build()
                    .map_err(|_| "development_window")?;
                Ok(())
            })();
            if let Err(error) = setup {
                eprintln!("development_startup: {error}");
                // Tauri panics on a returned setup error. Close explicitly and
                // present a native error instead of leaving an invisible child.
                let cleanup_failed = close_owner(&setup_owner).is_err();
                if let Ok(mut state) = app.state::<Mutex<Lifecycle>>().lock() {
                    *state = Lifecycle::Stopping;
                }
                show_failure(cleanup_failed);
                if let Ok(mut state) = app.state::<Mutex<Lifecycle>>().lock() {
                    state.shutdown_complete();
                }
                app.handle().exit(2);
            }
            Ok(())
        })
        .build(context)
        .map_err(|_| "development_window")?;
    app.run(move |app, event| match event {
        tauri::RunEvent::ExitRequested { api, code, .. } => {
            let state = app.state::<Mutex<Lifecycle>>();
            if state
                .lock()
                .is_ok_and(|state| *state == Lifecycle::ExitReady)
            {
                return;
            }
            // Every initial request is held. Only a second, explicit request
            // after successful cleanup can terminate the event loop.
            api.prevent_exit();
            let dirty = app.state::<DevelopmentDraft>().0.load(Ordering::Acquire);
            let mut decision = match state.lock() {
                Ok(mut state) => state.request_quit(dirty),
                Err(_) => Quit::Prevent,
            };
            if decision == Quit::Confirm {
                // Release the state lock before the modal nested event loop.
                let discard = crate::native_confirm_discard_and_quit(crate::CAPTURE_QUIT_WARNING);
                decision = match state.lock() {
                    Ok(mut state) => state.confirm_quit(discard),
                    Err(_) => Quit::Prevent,
                };
            }
            if decision != Quit::Proceed {
                return;
            }
            if close_owner(&owner).is_err() {
                show_failure(true);
                std::process::exit(2);
            }
            if let Ok(mut state) = state.lock() {
                state.shutdown_complete();
            }
            app.exit(code.unwrap_or(0));
        }
        tauri::RunEvent::Reopen { .. } => reopen(app),
        // Emergency fallback; ordinary Quit already observed shutdown.
        tauri::RunEvent::Exit if close_owner(&owner).is_err() => {
            eprintln!(r#"{{"developmentStage":"exit_cleanup"}}"#);
            std::process::exit(2);
        }
        _ => {}
    });
    Ok(())
}
