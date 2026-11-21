//! Measurement hooks exclusive to the disposable M14 desktop build.
use super::{p0_2_autostart_disable, p0_2_autostart_enable, p0_2_autostart_status};
use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn root() -> Result<PathBuf> {
    let (user, base) = if cfg!(windows) {
        (
            "USERNAME",
            PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("missing LocalAppData")?),
        )
    } else {
        (
            "USER",
            PathBuf::from(std::env::var_os("HOME").ok_or("missing home")?)
                .join("Library/Application Support"),
        )
    };
    let user = std::env::var(user)?;
    let root = base.join("Cryptex-M14-Login");
    let hosted = matches!(user.as_str(), "runner" | "runneradmin") && root.join("hosted").is_file();
    if !(hosted || user.eq_ignore_ascii_case("cryptexm14")) {
        return Err("a disposable login account is required".into());
    }
    if PathBuf::from(fs::read_to_string(root.join("executable"))?).canonicalize()?
        != std::env::current_exe()?.canonicalize()?
    {
        return Err("wrong test executable".into());
    }
    Ok(root)
}

#[cfg(windows)]
fn session() -> Result<String> {
    let system = PathBuf::from(std::env::var_os("SystemRoot").ok_or("missing Windows directory")?);
    let output = super::background_process::command(system.join("System32/whoami.exe"))
        .arg("/logonid")
        .output()?;
    let sid = String::from_utf8(output.stdout)?;
    let sid = sid.trim();
    if !output.status.success() || !sid.starts_with("S-1-5-5-") {
        return Err("missing interactive logon SID".into());
    }
    Ok(sid.into())
}

#[cfg(target_os = "macos")]
fn session() -> Result<String> {
    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        fn SessionGetInfo(caller: u32, session: *mut u32, attributes: *mut u32) -> i32;
    }
    let mut id = 0;
    let mut attributes = 0;
    // Security.framework's callerSecuritySession selector. Output buffers are local.
    if unsafe { SessionGetInfo(u32::MAX, &mut id, &mut attributes) } != 0 || id == 0 {
        return Err("security session unavailable".into());
    }
    Ok(id.to_string())
}

pub(super) fn record_attempt() -> Result<()> {
    let root = root()?;
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args == ["--m14-session"] {
        println!("{}", session()?);
        std::process::exit(0);
    }
    if args == ["--cryptex-background"] {
        // Count attempts before the single-instance plugin can absorb a duplicate.
        let mut record = tempfile::Builder::new()
            .prefix("launch-")
            .tempfile_in(&root)?;
        write!(record, "{}", session()?)?;
        record.keep()?;
    } else if args != ["--m14-arm"] && args != ["--m14-disarm"] {
        return Err("unknown rehearsal action".into());
    }
    Ok(())
}

pub(super) fn observe(app: &AppHandle) -> Result<()> {
    let root = root()?;
    let handle = app.clone();
    let arm = std::env::args().nth(1).as_deref() == Some("--m14-arm");
    let disarm = std::env::args().nth(1).as_deref() == Some("--m14-disarm");
    if arm {
        if p0_2_autostart_status(handle.clone()).map_err(|e| format!("read startup status: {e}"))? {
            return Err("existing registration".into());
        }
        fs::write(root.join("armed-session"), session()?)?;
        p0_2_autostart_enable(handle.clone()).map_err(|e| format!("enable startup: {e}"))?;
        p0_2_autostart_enable(handle.clone()).map_err(|e| format!("enable startup: {e}"))?;
        if !p0_2_autostart_status(handle.clone())? {
            return Err("registration was not enabled".into());
        }
    } else if disarm {
        p0_2_autostart_disable(handle.clone())?;
    } else {
        let window = handle.get_webview_window("main").ok_or("missing window")?;
        if window.is_visible()? || !p0_2_autostart_status(handle.clone())? {
            return Err("unexpected startup window or registration".into());
        }
        fs::write(root.join("ready-session"), session()?)?;
    }
    std::thread::spawn(move || {
        if !arm && !disarm {
            // Keep the first instance alive while observing duplicate startup attempts.
            std::thread::sleep(std::time::Duration::from_secs(60));
            let outcome = p0_2_autostart_disable(handle.clone()).and_then(|_| {
                if p0_2_autostart_status(handle.clone())? {
                    Err("registration still present".into())
                } else {
                    fs::write(root.join("disabled"), "ok").map_err(|e| e.to_string())
                }
            });
            if let Err(e) = outcome {
                eprintln!("Disable failed: {e}");
                handle.exit(1);
                return;
            }
        }
        handle.exit(0);
    });
    Ok(())
}
