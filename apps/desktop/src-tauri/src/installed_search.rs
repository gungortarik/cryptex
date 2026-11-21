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

pub(super) fn writer_folder(create: bool) -> Option<PathBuf> {
    let work = PathBuf::from(std::env::var_os("CRYPTEX_M11_REOPEN_FIXTURE")?);
    let parent = work.join("writer-targets");
    if create {
        return Some(parent);
    }
    if std::env::var("CRYPTEX_M11_INSTALLED_EXPORT").as_deref() == Ok("writer-copy") {
        return Some(work.join("other-target"));
    }
    let mut entries = std::fs::read_dir(parent).ok()?;
    let target = entries.next()?.ok()?.path();
    if entries.next().is_some() {
        return None;
    }
    Some(target)
}

pub(super) fn verification_file(title: &str) -> Option<PathBuf> {
    let work = PathBuf::from(std::env::var_os("CRYPTEX_M11_REOPEN_FIXTURE")?);
    match title {
        "Select the first-device recovery backup" => Some(work.join("first-check/backup.pgp")),
        "Select its checkpoint from your separate trusted folder"
            if std::env::var("CRYPTEX_M11_INSTALLED_EXPORT").is_ok_and(|m| {
                m.starts_with("first-device-check") || m.starts_with("first-device-clean")
            }) =>
        {
            Some(work.join("kit/selected-backup.json"))
        }
        "Select the signed first-device approval" => Some(challenge_folder()?.join(
            if std::env::var("CRYPTEX_M11_INSTALLED_EXPORT").as_deref() == Ok("first-device-wrong")
            {
                "wrong-approval.json"
            } else {
                "owner-approval.json"
            },
        )),
        "Select the scheduled generation to verify" => {
            Some(writer_folder(false)?.join("backup-00000000000000000001.pgp"))
        }
        "Select its checkpoint from your separate trusted folder"
            if std::env::var("CRYPTEX_M11_INSTALLED_EXPORT")
                .is_ok_and(|m| m.starts_with("writer-verify")) =>
        {
            let mut entries = std::fs::read_dir(work.join("schedule-checkpoints")).ok()?;
            let selected = entries.next()?.ok()?.path();
            if entries.next().is_some() {
                return None;
            }
            Some(selected)
        }
        "Select the original encrypted backup used to recover this device" => {
            Some(work.join("pair-input/original.pgp"))
        }
        "Select the independently saved checkpoint for that original backup" => {
            Some(work.join("pair-checkpoints/original.json"))
        }
        "Select the encrypted manual backup"
            if std::env::var("CRYPTEX_M11_INSTALLED_EXPORT")
                .is_ok_and(|m| m.starts_with("verify-pair")) =>
        {
            Some(work.join("pair-volume/selected.pgp"))
        }
        "Select its checkpoint from your separate trusted folder"
            if std::env::var("CRYPTEX_M11_INSTALLED_EXPORT")
                .is_ok_and(|m| m.starts_with("verify-pair")) =>
        {
            Some(work.join("pair-checkpoints/selected.json"))
        }
        "Select the encrypted manual backup" => Some(work.join("export-volume/restore-input.pgp")),
        "Select its checkpoint from your separate trusted folder" => {
            Some(work.join("export-checkpoints/restore-selection.json"))
        }
        "Select your encrypted offline Recovery Kit" => Some(work.join("offline-kit.pgp")),
        "Select the separately held Kit unlock file" => {
            std::env::var_os("CRYPTEX_M11_VERIFY_UNLOCK").map(PathBuf::from)
        }
        _ => None,
    }
}

pub(super) fn challenge_folder() -> Option<PathBuf> {
    let folder = if std::env::var("CRYPTEX_M11_INSTALLED_EXPORT")
        .is_ok_and(|m| m.starts_with("first-device-clean"))
    {
        "clean-challenges"
    } else {
        "public-challenges"
    };
    Some(PathBuf::from(std::env::var_os("CRYPTEX_M11_REOPEN_FIXTURE")?).join(folder))
}

pub(super) fn confirm_recipient(digest: &str) -> bool {
    std::env::var("CRYPTEX_M11_INSTALLED_EXPORT").as_deref() != Ok("decline")
        && std::env::var("CRYPTEX_M11_EXPECT_PUBLIC_DIGEST").as_deref() == Ok(digest)
}

pub(super) fn selected_folder(checkpoint: bool) -> Option<PathBuf> {
    let work = PathBuf::from(std::env::var_os("CRYPTEX_M11_REOPEN_FIXTURE")?);
    let mode = std::env::var("CRYPTEX_M11_INSTALLED_EXPORT").ok()?;
    if checkpoint && mode == "writer-enable" {
        return Some(work.join("schedule-checkpoints"));
    }
    if checkpoint && mode == "cancel" {
        return None;
    }
    if mode == "pair-write" {
        return Some(work.join(if checkpoint {
            "pair-checkpoints"
        } else {
            "pair-volume"
        }));
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
        Some("pair-write") => "replacement/recovery/first-device.json",
        Some("first-device-export" | "first-device-wrong" | "first-device-import") => {
            "replacement/identity/brain_identity.json"
        }
        Some(
            "verify-pair"
            | "verify-pair-missing"
            | "first-device-check"
            | "first-device-clean-prepare"
            | "first-device-clean-reject"
            | "first-device-clean-export"
            | "first-device-clean-import"
            | "first-device-check-damaged",
        ) => "offline-kit.pgp",
        Some(
            "verify"
            | "verify-cancel"
            | "verify-damaged"
            | "verify-wrong-key"
            | "verify-wrong-checkpoint",
        ) => "offline-kit.pgp",
        Some(
            "wrong-brain"
            | "decline"
            | "cancel"
            | "write"
            | "overlap"
            | "damaged"
            | "writer-retain"
            | "writer-verify"
            | "writer-verify-wrong"
            | "writer-enable"
            | "writer-resume"
            | "writer-pause"
            | "writer-decline"
            | "writer-create"
            | "writer-inspect"
            | "writer-copy",
        ) => "export-source/identity/brain_identity.json",
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
        Some("verify-cancel" | "writer-decline") => 0,
        Some("pair-write") => 5,
        Some("writer-enable" | "decline" | "wrong-brain" | "write" | "overlap") => 2,
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
    let wait_for_schedule = mode.as_deref() == Some("writer-resume");
    let app = tauri::Builder::default()
        .manage(observed.clone())
        .invoke_handler(tauri::generate_handler![
            super::p0_4_guarded_search,
            super::m11_export_backup,
            super::m11_backup_writer,
            super::m11_verify_manual_backup,
            super::m11_first_device,
            super::m11_prepare_replacement,
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
    if wait_for_schedule {
        let target = writer_folder(false).ok_or("missing scheduled target")?;
        let end = std::time::Instant::now() + std::time::Duration::from_secs(45);
        while !target.join("backup-00000000000000000001.pgp").is_file() {
            if std::time::Instant::now() >= end {
                return Err("scheduled generation timed out".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
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
