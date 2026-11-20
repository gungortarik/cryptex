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

fn check() -> Result<bool, Box<dyn std::error::Error>> {
    let work = PathBuf::from(
        std::env::var_os("CRYPTEX_M11_REOPEN_FIXTURE").ok_or("missing disposable profile")?,
    )
    .canonicalize()?;
    if work.parent() != Some(std::env::temp_dir().canonicalize()?.as_path())
        || !work
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.starts_with("cryptex-m11-recovery-"))
        || !work.join("public-test-context.json").is_file()
    {
        return Err("invalid disposable profile".into());
    }
    let desktop = std::env::current_exe()?;
    let reader_path = desktop.with_file_name(format!(
        "cryptex-m11-reader{}",
        std::env::consts::EXE_SUFFIX
    ));
    let mut reader = Reader(Command::new(reader_path)
        .args(["--exact", "knowledge_materialization::pairing_recovery_tests::m11_backup_read::restored_note_and_attachment_pass_guarded_reads", "--ignored", "--nocapture"])
        .env("CRYPTEX_M11_REOPEN_CASE", "installed")
        .env("CRYPTEX_M11_REOPEN_FIXTURE", &work)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()?);
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
    writeln!(
        reader.0.stdin.as_mut().ok_or("missing reader input")?,
        "serve"
    )?;
    let observed = Arc::new(AtomicBool::new(false));
    let app = tauri::Builder::default()
        .manage(observed.clone())
        .invoke_handler(tauri::generate_handler![
            super::p0_4_guarded_search,
            m11_search_observed
        ])
        .on_page_load(|view, payload| {
            if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished)
                && view.eval(include_str!("installed_search.js")).is_err()
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
