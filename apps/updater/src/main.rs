//! Apply an explicitly staged local package while the user has quit Desktop.
mod release;
use cryptex_updater::{Health, Layout, PROBE, desktop};
use process_wrap::std::{ChildWrapper, CommandWrap};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env, fs,
    io::{self, Read},
    path::Path,
    process::{Command, ExitStatus, Stdio},
    sync::mpsc,
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

enum Probe {
    Replied(Health),
    TimedOut,
    Failed,
}

fn require_connection_check(layout: &Layout, package: &Path, digest: &str) -> io::Result<Health> {
    match probe(layout, package, digest)? {
        Probe::Replied(health) => Ok(health),
        Probe::TimedOut => Err(error("working Desktop update probe timed out")),
        Probe::Failed => Err(error("working Desktop failed its update probe")),
    }
}

fn wait_for_exit(
    child: &mut dyn ChildWrapper,
    timeout: Duration,
) -> io::Result<Option<ExitStatus>> {
    let deadline = Instant::now() + timeout;
    loop {
        let status = child.try_wait()?;
        if status.is_some() || Instant::now() >= deadline {
            return Ok(status);
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn terminate_probe(child: &mut dyn ChildWrapper) -> io::Result<()> {
    // Signal only this probe's job/process group, then reap the child before recovery.
    child.start_kill()?;
    wait_for_exit(child, Duration::from_secs(5))?
        .ok_or_else(|| error("update probe did not exit after termination"))?;
    Ok(())
}

fn probe(layout: &Layout, package: &Path, digest: &str) -> io::Result<Probe> {
    release::verify(layout, package, digest)?;
    let mut command = Command::new(desktop(package));
    command
        .arg(PROBE)
        .stdin(Stdio::null())
        .stdout(Stdio::piped());
    let mut command = CommandWrap::from(command);
    #[cfg(windows)]
    command.wrap(process_wrap::std::JobObject);
    #[cfg(unix)]
    command.wrap(process_wrap::std::ProcessGroup::leader());
    let mut child = command.spawn()?;
    let deadline = Instant::now() + Duration::from_secs(30);
    let stdout = child
        .stdout()
        .take()
        .ok_or_else(|| error("missing probe output"))?;
    let (send, receive) = mpsc::sync_channel(1);
    // Keep the pipe and retained child under one deadline. A descendant may still
    // hold stdout after the probe exits. Keep the leader unreaped until the read ends.
    if let Err(cause) = thread::Builder::new().spawn(move || {
        let mut bytes = Vec::new();
        let reply = stdout.take(4097).read_to_end(&mut bytes).map(|_| bytes);
        let _ = send.send(reply);
    }) {
        terminate_probe(&mut *child)?;
        return Err(cause);
    }
    let bytes = match receive.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(Ok(bytes)) if bytes.len() <= 4096 => bytes,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            terminate_probe(&mut *child)?;
            return Ok(Probe::TimedOut);
        }
        _ => {
            terminate_probe(&mut *child)?;
            return Err(error("invalid Desktop update probe output"));
        }
    };
    let Some(exit) = wait_for_exit(
        &mut *child,
        deadline.saturating_duration_since(Instant::now()),
    )?
    else {
        terminate_probe(&mut *child)?;
        return Ok(Probe::TimedOut);
    };
    if !exit.success() {
        return Ok(Probe::Failed);
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
    Ok(Probe::Replied(health))
}

fn vacant(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(false),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(true),
        Err(e) => Err(e),
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    working: String,
    staged: String,
}

impl Intent {
    fn save(&self, file: &Path) -> io::Result<()> {
        let temp = tempfile::NamedTempFile::new_in(
            file.parent()
                .ok_or_else(|| error("missing updater state directory"))?,
        )?;
        serde_json::to_writer(temp.as_file(), self)?;
        temp.as_file().sync_all()?;
        temp.persist_noclobber(file)?;
        Ok(())
    }

    fn restore(&self, layout: &Layout) -> io::Result<Vec<Health>> {
        // Find both recorded images before moving anything. A crash may have
        // happened between any two renames, including during an earlier retry.
        let mut working = None;
        let mut staged = None;
        for path in [&layout.active, &layout.previous, &layout.pending] {
            if vacant(path)? {
                continue;
            }
            let hash = package_digest(path)?;
            if hash == self.working && working.is_none() {
                working = Some(path);
            } else if hash == self.staged && staged.is_none() {
                staged = Some(path);
            } else {
                return Err(error("unexpected image during recovery"));
            }
        }
        let old = working.ok_or_else(|| error("working image missing during recovery"))?;
        let new = staged.ok_or_else(|| error("staged image missing during recovery"))?;
        let verified = require_connection_check(layout, old, &self.working)?;
        if verified.status != "connected" {
            return Err(error("retained Desktop could not reconnect"));
        }
        if old == &layout.pending && new == &layout.active {
            fs::rename(old, &layout.previous)?;
        } else if !((old == &layout.active && new == &layout.pending)
            || (old == &layout.previous && (new == &layout.active || new == &layout.pending)))
        {
            return Err(error("unexpected package positions during recovery"));
        }
        if new == &layout.active {
            fs::rename(new, &layout.pending)?;
        }
        if old != &layout.active {
            fs::rename(&layout.previous, &layout.active)?;
        }
        let restored = require_connection_check(layout, &layout.active, &self.working)?;
        if restored.status != "connected" || restored.desktop_protocol != verified.desktop_protocol
        {
            return Err(error("recovered Desktop could not reconnect"));
        }
        Ok(vec![verified, restored])
    }
}

fn reopen(
    layout: &Layout,
    lock: fs::File,
    digest: &str,
    outcome: &str,
    checks: Vec<Health>,
) -> io::Result<()> {
    release::verify(layout, &layout.active, digest)?;
    fs::remove_file(layout.intent())?;
    drop(lock);
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

fn apply(layout: &Layout, approved: &str) -> io::Result<()> {
    let transaction = layout.begin_update()?;
    let journal = layout.intent();
    if !vacant(&journal)? {
        let intent: Intent = serde_json::from_reader(fs::File::open(&journal)?.take(1024))?;
        let checks = intent.restore(layout)?;
        return reopen(layout, transaction, &intent.working, "recovered", checks);
    }
    if !vacant(&layout.previous)? {
        return Err(error("an unfinished update retains the previous package"));
    }
    let original = package_digest(&layout.active)?;
    if package_digest(&layout.pending)? != approved {
        return Err(error("staged package differs from the approved digest"));
    }
    if original == approved {
        return Err(error("working and staged packages are identical"));
    }
    if env::current_exe()?
        .canonicalize()?
        .starts_with(layout.active.canonicalize()?)
    {
        return Err(error(
            "run the standalone updater from outside Desktop's install directory",
        ));
    }
    release::verify(layout, &layout.pending, approved)?;
    let before = require_connection_check(layout, &layout.active, &original)?;
    if before.status != "connected" {
        return Err(error("current Desktop does not have a working daemon"));
    }
    let intent = Intent {
        working: original.clone(),
        staged: approved.to_owned(),
    };
    intent.save(&journal)?;
    fs::rename(&layout.active, &layout.previous)?;
    if let Err(cause) = fs::rename(&layout.pending, &layout.active) {
        fs::rename(&layout.previous, &layout.active)?;
        return Err(cause);
    }
    if package_digest(&layout.active)? != approved {
        return Err(error("activated package changed"));
    }
    let candidate = match probe(layout, &layout.active, approved)? {
        Probe::Replied(health) => health,
        failure @ (Probe::TimedOut | Probe::Failed) => {
            let mut checks = vec![before];
            checks.extend(intent.restore(layout)?);
            return reopen(
                layout,
                transaction,
                &original,
                if matches!(failure, Probe::TimedOut) {
                    "rolled_back_timeout"
                } else {
                    "rolled_back_failed_probe"
                },
                checks,
            );
        }
    };
    let mut checks = vec![before, candidate];
    let outcome = if checks[1].status == "incompatible" {
        fs::rename(&layout.active, &layout.pending)?;
        fs::rename(&layout.previous, &layout.active)?;
        if package_digest(&layout.active)? != original {
            return Err(error("previous package changed"));
        }
        let restored = require_connection_check(layout, &layout.active, &original)?;
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
    reopen(
        layout,
        transaction,
        if outcome == "updated" {
            approved
        } else {
            &original
        },
        outcome,
        checks,
    )
}

fn main() -> io::Result<()> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [command, directory] if command == "fingerprint" => {
            println!("{}", package_digest(Path::new(directory))?);
            Ok(())
        }
        [command, package, receipt] if command == "admit" => {
            release::admit(&Layout::current()?, Path::new(package), Path::new(receipt))
        }
        [command, digest] if command == "apply" => {
            let hash = digest
                .to_str()
                .filter(|s| s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit()))
                .ok_or_else(|| error("expected SHA-256 package fingerprint"))?;
            apply(&Layout::current()?, &hash.to_ascii_lowercase())
        }
        _ => Err(error(
            "use fingerprint PATH, admit PATH RECEIPT or apply APPROVED_SHA256",
        )),
    }
}
