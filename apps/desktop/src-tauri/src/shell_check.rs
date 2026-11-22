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

fn check_scaled_appearance(window: &WebviewWindow) -> Result<()> {
    let (tx, rx) = mpsc::channel();
    window.eval_with_callback("window.innerWidth", move |value| {
        let _ = tx.send(value);
    })?;
    let initial_width: f64 = rx.recv_timeout(Duration::from_secs(5))?.parse()?;
    for (theme, dark, background) in [
        (Theme::Light, false, "rgb(246, 245, 241)"),
        (Theme::Dark, true, "rgb(29, 29, 28)"),
    ] {
        window.set_theme(Some(theme))?;
        for zoom in [1.0, 1.5, 2.0] {
            window.set_zoom(zoom)?;
            let expected_width = initial_width / zoom;
            let expression = format!(
                r#"(() => {{
                const width = document.documentElement.clientWidth;
                const controls = document.querySelectorAll('.side-rail a, .side-rail button, #memory-search, .search-button');
                return Math.abs(innerWidth - {expected_width}) <= 2
                    && matchMedia('(prefers-color-scheme: dark)').matches === {dark}
                    && getComputedStyle(document.documentElement).backgroundColor === '{background}'
                    && document.documentElement.scrollWidth <= width + 1
                    && controls.length >= 4 && [...controls].every(el => {{
                        const r = el.getBoundingClientRect();
                        return r.width > 0 && r.height > 0 && r.left >= -1 && r.right <= width + 1;
                    }});
            }})()"#
            );
            if let Err(cause) = until(
                &format!("{theme:?} at {zoom}x (expected viewport {expected_width})"),
                || webview_matches(window, &expression),
            ) {
                let (tx, rx) = mpsc::channel();
                window.eval_with_callback(
                    "({ width: innerWidth, client: document.documentElement.clientWidth, scroll: document.documentElement.scrollWidth, density: devicePixelRatio, dark: matchMedia('(prefers-color-scheme: dark)').matches, outside: [...document.querySelectorAll('.side-rail a, .side-rail button, #memory-search, .search-button')].filter(el => el.getBoundingClientRect().right > document.documentElement.clientWidth + 1).map(el => el.tagName) })",
                    move |value| { let _ = tx.send(value); },
                )?;
                return Err(
                    format!("{cause}: {}", rx.recv_timeout(Duration::from_secs(5))?).into(),
                );
            }
            println!("M14 native layout {theme:?} {zoom}x");
        }
    }
    window.set_zoom(1.0)?;
    window.set_theme(None)?;
    Ok(())
}

fn check_engine(app: &AppHandle, window: &WebviewWindow) -> Result<()> {
    let state = app.state::<super::ProbeState>();
    let runtime = super::pilot_runtime::root().ok_or("missing temporary engine profile")?;
    let original = super::check_local_service()?.daemon_pid;
    if state
        .child
        .lock()
        .map_err(|_| "engine lock")?
        .as_ref()
        .map(|child| child.id())
        != Some(original)
    {
        return Err("health check did not identify the owned pilot engine".into());
    }
    let click = |label: &str| -> Result<bool> {
        let expression = format!(
            "(() => {{ const b = Array.from(document.querySelectorAll('.engine-controls button')).find(b => b.textContent === '{label}' && !b.disabled && b.offsetWidth); if (!b) return false; b.click(); return true; }})()"
        );
        webview_matches(window, &expression)
    };
    until("restart control", || click("Restart engine"))?;
    until("replacement engine health", || {
        Ok(super::check_local_service().is_ok_and(|r| r.daemon_pid != original))
    })?;
    if !webview_matches(
        window,
        "(() => { document.querySelector('.engine-controls summary').click(); return true; })()",
    )? {
        return Err("advanced controls unavailable".into());
    }
    until("stop review", || click("Stop engine…"))?;
    until("cancel focused", || {
        webview_matches(window, "document.activeElement?.textContent === 'Cancel'")
    })?;
    if !click("Cancel")? || super::check_local_service().is_err() {
        return Err("cancel interrupted engine service".into());
    }
    until("reopen stop review", || click("Stop engine…"))?;
    until("confirm stop", || click("Stop engine"))?;
    until("observed engine stop", || {
        Ok(webview_matches(
            window,
            "document.querySelector('.engine-controls [role=status]')?.textContent.includes('Local engine stopped.') === true",
        )? && super::current_daemon_pid(&runtime, &state)?.is_none())
    })?;
    super::restart_exited_daemon(&state, &runtime)?;
    if super::current_daemon_pid(&runtime, &state)?.is_some() {
        return Err("supervisor restarted an intentionally stopped engine".into());
    }
    Ok(())
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
    check_scaled_appearance(&window)?;
    if !webview_matches(&window, include_str!("shell_shortcut_check.js"))? {
        return Err("global Search intercepted a modal keypress".into());
    }
    until("timeline navigation", || {
        webview_matches(
            &window,
            r#"document.querySelector('button[aria-label="Open timeline"]')?.getAttribute('aria-pressed') === 'true'"#,
        )
    })?;
    let shortcut = if cfg!(target_os = "macos") {
        "metaKey"
    } else {
        "ctrlKey"
    };
    let press = format!(
        "!window.dispatchEvent(new KeyboardEvent('keydown', {{key:'k', {shortcut}:true, bubbles:true, cancelable:true}}))"
    );
    if !webview_matches(&window, &press)? {
        return Err("global Search shortcut was not handled".into());
    }
    until("global Search focus", || {
        webview_matches(
            &window,
            r#"document.activeElement?.id === 'memory-search' && document.querySelector('button[aria-label="Search memory"]')?.getAttribute('aria-pressed') === 'true'"#,
        )
    })?;
    if !webview_matches(
        &window,
        "(() => { const input = document.querySelector('#memory-search'); if (!input) return false; Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(input, 'shell-reopen'); input.dispatchEvent(new Event('input', { bubbles: true })); return true; })()",
    )? {
        return Err("search field unavailable before close".into());
    }
    let original_daemon = super::check_local_service()?.daemon_pid;
    window.minimize()?;
    until("native window minimized", || Ok(window.is_minimized()?))?;
    super::show_main_window(
        app,
        "m14_minimized_reopen",
        &app.state::<super::ProbeState>(),
    )?;
    until("restored minimized window", || {
        Ok(!window.is_minimized()? && window.is_visible()?)
    })?;
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
    if super::check_local_service()?.daemon_pid != original_daemon {
        return Err("restoring the window replaced the running engine".into());
    }
    check_engine(app, &window)?;
    Ok(())
}

pub(super) fn start(app: AppHandle) {
    thread::spawn(move || {
        let result = check(&app);
        if let Err(error) = result {
            eprintln!("M13 shell check failed: {error}");
            if let Err(error) = super::stop_supervised_daemon(&app.state::<super::ProbeState>()) {
                eprintln!("M13 pilot engine cleanup failed: {error}");
            }
            app.exit(1);
        } else {
            println!("M13 native shell verified");
            app.exit(0);
        }
    });
}
