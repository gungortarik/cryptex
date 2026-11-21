//! Installed QA edition. Only Search and the fixed DOM check are registered.
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[tauri::command]
fn m11_search_observed(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AtomicBool>>,
    visible: bool,
) {
    state.store(visible, Ordering::Release);
    app.exit(if visible { 0 } else { 1 });
}

struct Reader(Child);

impl Drop for Reader {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub(super) fn recipient_folder() -> Option<PathBuf> {
    let work = PathBuf::from(std::env::var_os("CRYPTEX_M11_REOPEN_FIXTURE")?);
    Some(work.join("kit-public"))
}

pub(super) fn confirm_recipient(digest: &str) -> bool {
    std::env::var("CRYPTEX_M11_INSTALLED_EXPORT").as_deref() != Ok("decline")
        && std::env::var("CRYPTEX_M11_EXPECT_PUBLIC_DIGEST").as_deref() == Ok(digest)
}

pub(super) fn selected_folder(checkpoint: bool) -> Option<PathBuf> {
    let work = PathBuf::from(std::env::var_os("CRYPTEX_M11_REOPEN_FIXTURE")?);
    let mode = std::env::var("CRYPTEX_M11_INSTALLED_EXPORT").ok()?;
    if checkpoint && mode == "cancel" {
        return None;
    }
    Some(work.join(if checkpoint && mode != "overlap" {
        "export-checkpoints"
    } else {
        "export-volume"
    }))
}

fn check() -> Result<bool, Box<dyn std::error::Error>> {
    let work = PathBuf::from(
        std::env::var_os("CRYPTEX_M11_REOPEN_FIXTURE").ok_or("missing disposable profile")?,
    )
    .canonicalize()?;
    let mode = std::env::var("CRYPTEX_M11_INSTALLED_EXPORT").ok();
    let expected = match mode.as_deref() {
        None => "replacement/recovery/first-device.json",
        Some("wrong-brain" | "decline" | "cancel" | "write" | "overlap" | "damaged") => {
            "export-source/identity/brain_identity.json"
        }
        _ => return Err("invalid rehearsal mode".into()),
    };
    if work.parent() != Some(std::env::temp_dir().canonicalize()?.as_path())
        || !work
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.starts_with("cryptex-m11-recovery-"))
        || !work.join(expected).is_file()
    {
        return Err("invalid disposable profile".into());
    }
    let desktop = std::env::current_exe()?;
    let reader_path = desktop.with_file_name(format!("cryptexd{}", std::env::consts::EXE_SUFFIX));
    let mut reader = Reader(
        Command::new(reader_path)
            .arg("--rehearse-recovered-search")
            .env("CRYPTEX_M11_REOPEN_FIXTURE", &work)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?,
    );
    let mut output = BufReader::new(reader.0.stdout.take().ok_or("missing reader output")?);
    let mut line = String::new();
    loop {
        line.clear();
        if output.read_line(&mut line)? == 0 {
            return Err("reader exited before startup".into());
        }
        if line.trim() == "fixture-ready" {
            break;
        }
    }
    let requests = match mode.as_deref() {
        Some("decline" | "wrong-brain" | "write" | "overlap") => 2,
        Some("cancel") => 3, // First confirmation, then cancellation at folder selection.
        _ => 1,
    };
    for _ in 0..requests {
        writeln!(
            reader.0.stdin.as_mut().ok_or("missing reader input")?,
            "serve"
        )?;
    }
    let observed = Arc::new(AtomicBool::new(false));
    let app = tauri::Builder::default()
        .manage(observed.clone())
        .invoke_handler(tauri::generate_handler![
            super::p0_4_guarded_search,
            super::m11_export_backup,
            m11_search_observed
        ])
        .on_page_load(move |view, payload| {
            let script = match mode.as_deref() {
                Some(mode) => include_str!("installed_export.js").replace("EXPECTED_MODE", mode),
                None => include_str!("installed_search.js").to_owned(),
            };
            if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished)
                && view.eval(&script).is_err()
            {
                use tauri::Manager;
                view.app_handle().exit(2);
            }
        })
        .build(tauri::generate_context!())?;
    let code = app.run_return(|_, _| {});
    if code != 0 || !observed.load(Ordering::Acquire) {
        return Ok(false);
    }
    writeln!(
        reader.0.stdin.take().ok_or("missing reader input")?,
        "finish"
    )?;
    Ok(reader.0.wait()?.success())
}

pub(super) fn run() -> i32 {
    match check() {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(error) => {
            eprintln!("installed search rehearsal: {error}");
            2
        }
    }
}
