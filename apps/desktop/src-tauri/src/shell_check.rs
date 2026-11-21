//! Native shell acceptance, compiled only into the disposable rehearsal binary.
use std::error::Error;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, Theme, WebviewWindow};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn until(label: &str, mut ready: impl FnMut() -> Result<bool>) -> Result<()> {
    let end = Instant::now() + Duration::from_secs(20);
    while Instant::now() < end {
        if ready()? {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(150));
    }
    Err(format!("timed out checking {label}").into())
}

fn webview_matches(window: &WebviewWindow, expression: &str) -> Result<bool> {
    let (tx, rx) = mpsc::channel();
    window.eval_with_callback(expression, move |value| {
        let _ = tx.send(value);
    })?;
    Ok(rx.recv_timeout(Duration::from_secs(5))? == "true")
}

fn check(app: &AppHandle) -> Result<()> {
    let window = app
        .get_webview_window("main")
        .ok_or("missing main window")?;
    let menu = app.menu().ok_or("missing application menu")?;
    if menu.get(tauri::menu::WINDOW_SUBMENU_ID).is_none()
        || app.tray_by_id("cryptex-main-tray").is_none()
    {
        return Err("missing native Window menu or tray".into());
    }
    if window.title()? != "Cryptex" || !window.is_decorated()? || !window.is_resizable()? {
        return Err("main window lost its native title bar or resize controls".into());
    }
    window.show()?;
    until("mounted search and system typography", || {
        webview_matches(
            &window,
            "Boolean(document.querySelector('#memory-search') && getComputedStyle(document.body).fontFamily.split(',')[0].trim() === 'system-ui')",
        )
    })?;
    for (theme, dark, background) in [
        (Theme::Light, false, "rgb(246, 245, 241)"),
        (Theme::Dark, true, "rgb(29, 29, 28)"),
    ] {
        window.set_theme(Some(theme))?;
        let expression = format!(
            "matchMedia('(prefers-color-scheme: dark)').matches === {dark} && getComputedStyle(document.documentElement).backgroundColor === '{background}'"
        );
        until(&format!("{theme:?} appearance"), || {
            webview_matches(&window, &expression)
        })?;
    }
    window.set_theme(None)?;
    if !webview_matches(
        &window,
        "(() => { const input = document.querySelector('#memory-search'); if (!input) return false; input.value = 'shell-reopen'; return true; })()",
    )? {
        return Err("search field unavailable before close".into());
    }
    window.close()?;
    until("close to tray", || Ok(!window.is_visible()?))?;
    super::show_main_window(app, "m13_rehearsal", &app.state::<super::ProbeState>())?;
    until("restored WebView session", || {
        Ok(window.is_visible()?
            && webview_matches(
                &window,
                "document.querySelector('#memory-search')?.value === 'shell-reopen'",
            )?)
    })?;
    Ok(())
}

pub(super) fn start(app: AppHandle) {
    thread::spawn(move || {
        let result = check(&app);
        if let Err(error) = result {
            eprintln!("M13 shell check failed: {error}");
            app.exit(1);
        } else {
            println!("M13 native shell verified");
            app.exit(0);
        }
    });
}
