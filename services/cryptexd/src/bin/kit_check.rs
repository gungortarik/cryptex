//! Offline Kit companion. Reconstruct selected archives only in disposable storage.
#[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
fn main() -> std::process::ExitCode {
    use cryptexd::ipc::backup_verify::Selection;
    use std::io::Read;
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Review {
        current: Selection,
        candidate: Option<Selection>,
    }
    let check = || -> Option<()> {
        let mut bytes = Vec::new();
        std::io::stdin().take(65537).read_to_end(&mut bytes).ok()?;
        if bytes.len() > 65536 {
            return None;
        }
        let files: Review = serde_json::from_slice(&bytes).ok()?;
        let executable = std::env::current_exe().ok()?;
        let hashes = cryptexd::knowledge_materialization::backup::review_kit_backups(
            executable.parent()?,
            &files.current,
            files.candidate.as_ref(),
        )
        .ok()?;
        serde_json::to_writer(std::io::stdout(), &hashes).ok()
    };
    if check().is_some() {
        std::process::ExitCode::SUCCESS
    } else {
        eprintln!("Recovery backup verification failed.");
        std::process::ExitCode::FAILURE
    }
}

#[cfg(not(any(windows, all(target_os = "macos", feature = "m8-local-startup-check"))))]
fn main() -> std::process::ExitCode {
    std::process::ExitCode::FAILURE
}
