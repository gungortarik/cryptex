//! Apply an explicitly staged local package while the user has quit Desktop.
use cryptex_updater::{Health, Layout, PROBE, desktop};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env, fs,
    io::{self, Read},
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn error(message: &str) -> io::Error {
    io::Error::other(message)
}

fn package_digest(path: &Path) -> io::Result<String> {
    let mut manifest = BTreeMap::new();
    let mut paths = vec![path.to_path_buf()];
    while let Some(entry) = paths.pop() {
        let metadata = fs::symlink_metadata(&entry)?;
        if !metadata.is_dir() && !metadata.is_file() {
            return Err(error("package contains an indirect or special entry"));
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(error("package contains a reparse point"));
            }
        }
        if metadata.is_dir() {
            for child in fs::read_dir(entry)? {
                paths.push(child?.path());
            }
        } else {
            let name = entry
                .strip_prefix(path)
                .map_err(io::Error::other)?
                .to_str()
                .ok_or_else(|| error("package filename is not UTF-8"))?;
            let mut hash = Sha256::new();
            io::copy(&mut fs::File::open(&entry)?, &mut hash)?;
            manifest.insert(
                name.replace(std::path::MAIN_SEPARATOR, "/"),
                format!("{:x}", hash.finalize()),
            );
        }
    }
    let exe = desktop(path);
    let daemon = exe.with_file_name(if cfg!(windows) {
        "cryptexd.exe"
    } else {
        "cryptexd"
    });
    if !exe.is_file() || !daemon.is_file() {
        return Err(error("incomplete Desktop package"));
    }
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&manifest)?)
    ))
}

fn probe(package: &Path) -> io::Result<Health> {
    let mut child = Command::new(desktop(package))
        .arg(PROBE)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(30);
    let exit = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            return Err(error(
                "update probe timed out; quit Desktop before recovery",
            ));
        }
        thread::sleep(Duration::from_millis(50));
    };
    let mut bytes = Vec::new();
    child
        .stdout
        .take()
        .ok_or_else(|| error("missing probe output"))?
        .take(4097)
        .read_to_end(&mut bytes)?;
    if !exit.success() || bytes.len() > 4096 {
        return Err(error("Desktop failed its update probe"));
    }
    let health: Health = serde_json::from_slice(&bytes)?;
    if health.desktop_pid != child.id()
        || health.daemon_pid == 0
        || !matches!(
            (
                health.status.as_str(),
                health.desktop_protocol == health.daemon_protocol
            ),
            ("connected", true) | ("incompatible", false)
        )
    {
        return Err(error("invalid Desktop update response"));
    }
    Ok(health)
}

fn vacant(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(false),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(true),
        Err(e) => Err(e),
    }
}

fn apply(layout: &Layout, approved: &str) -> io::Result<()> {
    let transaction = layout.begin_update()?;
    if !vacant(&layout.previous)? {
        return Err(error("an unfinished update retains the previous package"));
    }
    let original = package_digest(&layout.active)?;
    if package_digest(&layout.pending)? != approved {
        return Err(error("staged package differs from the approved digest"));
    }
    if env::current_exe()?
        .canonicalize()?
        .starts_with(layout.active.canonicalize()?)
    {
        return Err(error(
            "run the standalone updater from outside Desktop's install directory",
        ));
    }
    let before = probe(&layout.active)?;
    if before.status != "connected" {
        return Err(error("current Desktop does not have a working daemon"));
    }
    fs::rename(&layout.active, &layout.previous)?;
    if let Err(cause) = fs::rename(&layout.pending, &layout.active) {
        fs::rename(&layout.previous, &layout.active)?;
        return Err(cause);
    }
    if package_digest(&layout.active)? != approved {
        return Err(error("activated package changed"));
    }
    let candidate = probe(&layout.active)?;
    let mut checks = vec![before, candidate];
    let outcome = if checks[1].status == "incompatible" {
        fs::rename(&layout.active, &layout.pending)?;
        fs::rename(&layout.previous, &layout.active)?;
        if package_digest(&layout.active)? != original {
            return Err(error("previous package changed"));
        }
        let restored = probe(&layout.active)?;
        if restored.status != "connected" || restored.desktop_protocol != checks[0].desktop_protocol
        {
            return Err(error("restored Desktop could not reconnect"));
        }
        checks.push(restored);
        "rolled_back"
    } else {
        fs::rename(&layout.previous, &layout.pending)?;
        "updated"
    };
    drop(transaction);
    let reopened = Command::new(desktop(&layout.active))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    println!(
        "{}",
        serde_json::json!({"outcome":outcome,"checks":checks,"reopened_pid":reopened.id()})
    );
    Ok(())
}

fn main() -> io::Result<()> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [command, directory] if command == "fingerprint" => {
            println!("{}", package_digest(Path::new(directory))?);
            Ok(())
        }
        [command, digest] if command == "apply" => {
            let hash = digest
                .to_str()
                .filter(|s| s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit()))
                .ok_or_else(|| error("expected SHA-256 package fingerprint"))?;
            apply(&Layout::current()?, &hash.to_ascii_lowercase())
        }
        _ => Err(error("use fingerprint PATH or apply APPROVED_SHA256")),
    }
}
