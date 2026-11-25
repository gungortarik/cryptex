//! Apply an explicitly staged local package while the user has quit Desktop.
#[cfg(target_os = "macos")]
mod mac_removal;
mod release;
use cryptex_updater::{Health, Layout, PROBE, desktop};
use process_wrap::std::{ChildWrapper, CommandWrap};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env, fs,
    io::{self, Read, Write},
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
    InvalidReply,
}

fn require_connection_check(layout: &Layout, digest: &str) -> io::Result<Health> {
    match probe(layout, digest, PROBE)? {
        Probe::Replied(health) => Ok(health),
        Probe::TimedOut => Err(error("working Desktop update probe timed out")),
        Probe::Failed => Err(error("working Desktop failed its update probe")),
        Probe::InvalidReply => Err(error("invalid working Desktop update response")),
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

fn probe(layout: &Layout, digest: &str, mode: &str) -> io::Result<Probe> {
    release::verify(layout, &layout.active, digest)?;
    let mut command = Command::new(desktop(&layout.active));
    command
        .arg(mode)
        .stdin(Stdio::null())
        .stdout(Stdio::piped());
    let mut command = CommandWrap::from(command);
    #[cfg(windows)]
    command.wrap(process_wrap::std::JobObject);
    #[cfg(unix)]
    command.wrap(process_wrap::std::ProcessGroup::leader());
    let mut child = command.spawn()?;
    let timeout = if mode == cryptex_updater::KEYCHAIN_PROBE {
        Duration::from_secs(150)
    } else {
        cryptex_updater::PROBE_TIMEOUT
    };
    let deadline = Instant::now() + timeout;
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
        Ok(Ok(_)) => {
            terminate_probe(&mut *child)?;
            return Ok(Probe::InvalidReply);
        }
        _ => {
            terminate_probe(&mut *child)?;
            return Err(error("could not read Desktop update probe output"));
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
    let Ok(health) = serde_json::from_slice::<Health>(&bytes) else {
        return Ok(Probe::InvalidReply);
    };
    if !key_reply_matches(mode, &health)
        || health.desktop_pid != child.id()
        || health.daemon_pid == 0
        || !matches!(
            (
                health.status.as_str(),
                health.desktop_protocol == health.daemon_protocol
            ),
            ("connected", true) | ("incompatible", false)
        )
    {
        return Ok(Probe::InvalidReply);
    }
    Ok(Probe::Replied(health))
}

fn key_reply_matches(mode: &str, health: &Health) -> bool {
    let expected_prompt = match mode {
        cryptex_updater::KEYCHAIN_PROBE => Some(true),
        cryptex_updater::KEYCHAIN_REOPEN => Some(false),
        _ => None,
    };
    health.key_check.as_ref().map(|r| r.prompted_access_allowed) == expected_prompt
        && health.key_check.as_ref().is_none_or(|r| {
            r.verifier_pid != 0
                && r.verifier_pid != health.desktop_pid
                && health.requires_existing_key
                && (r.staged_transport.is_none() || mode == cryptex_updater::KEYCHAIN_REOPEN)
        })
}

// Each check is a fresh, independently verified Desktop/daemon pair. The second
// child must reopen the same credential with all Keychain interaction disabled.
fn check_candidate_key(
    working: &Health,
    candidate: &Health,
    mut run: impl FnMut(&str) -> io::Result<Probe>,
) -> io::Result<bool> {
    if candidate.status != "connected" {
        return Ok(true);
    }
    if !candidate.requires_existing_key {
        return Ok(!working.requires_existing_key);
    }
    for mode in [
        cryptex_updater::KEYCHAIN_PROBE,
        cryptex_updater::KEYCHAIN_REOPEN,
    ] {
        if !matches!(run(mode)?, Probe::Replied(reply) if reply.requires_existing_key
            && reply.status == "connected" && reply.desktop_protocol == candidate.desktop_protocol)
        {
            return Ok(false);
        }
    }
    Ok(true)
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
    #[cfg(all(target_os = "macos", feature = "mac-owner-transport-update"))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    transport: Option<cryptexd::managed_profile::owner_transport_update::TransportTransition>,
}

impl Intent {
    fn write_intent(&self, file: &Path) -> io::Result<tempfile::NamedTempFile> {
        let temp = tempfile::NamedTempFile::new_in(
            file.parent()
                .ok_or_else(|| error("missing updater state directory"))?,
        )?;
        serde_json::to_writer(temp.as_file(), self)?;
        temp.as_file().sync_all()?;
        Ok(temp)
    }

    #[cfg(all(target_os = "macos", feature = "mac-owner-transport-update"))]
    fn select_transport(&mut self, layout: &Layout, reported: [u8; 32]) -> io::Result<()> {
        use cryptexd::managed_profile::owner_transport_update::TransportTransition;
        if self.transport.is_some() {
            return Err(error("transport generation already selected"));
        }
        let root = cryptexd::local_signed_startup::sealed_trial_root()
            .map_err(|_| error("sealed profile unavailable"))?;
        let stopped = layout.stopped_desktop()?;
        let transition = TransportTransition::capture(&root)
            .map_err(|_| error("staged transport unavailable"))?;
        if transition.fingerprint() != reported {
            return Err(error(
                "staged transport differs from the authenticated daemon reply",
            ));
        }
        self.transport = Some(transition);
        let path = layout.intent();
        self.write_intent(&path)?.persist(&path)?;
        fs::File::open(
            path.parent()
                .ok_or_else(|| error("missing transaction directory"))?,
        )?
        .sync_all()?;
        self.transport
            .as_ref()
            .ok_or_else(|| error("missing transport intent"))?
            .activate(&root)
            .map_err(|_| error("transport activation failed"))?;
        drop(stopped);
        Ok(())
    }

    #[cfg(not(all(target_os = "macos", feature = "mac-owner-transport-update")))]
    fn select_transport(&mut self, _: &Layout, _: [u8; 32]) -> io::Result<()> {
        Err(error(
            "updater does not support this enrolled transport transition",
        ))
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
        let stopped = layout.stopped_desktop()?;
        let old = working.ok_or_else(|| error("working image missing during recovery"))?;
        let new = staged.ok_or_else(|| error("staged image missing during recovery"))?;
        // Authenticate the retained image without launching its daemon against
        // any state left behind by the interrupted candidate.
        release::verify(layout, old, &self.working)?;
        #[cfg(all(target_os = "macos", feature = "mac-owner-transport-update"))]
        if let Some(transport) = &self.transport {
            let root = cryptexd::local_signed_startup::sealed_trial_root()
                .map_err(|_| error("sealed profile unavailable"))?;
            transport
                .restore(&root)
                .map_err(|_| error("could not restore retained native transport"))?;
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
        drop(stopped);
        let verified = require_connection_check(layout, &self.working)?;
        if verified.status != "connected" {
            return Err(error("retained Desktop could not reconnect"));
        }
        // Reopen independently after the first owned daemon has drained.
        let restored = require_connection_check(layout, &self.working)?;
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
        let intent: Intent = serde_json::from_reader(fs::File::open(&journal)?.take(2048))?;
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
    let before = require_connection_check(layout, &original)?;
    if before.status != "connected" {
        return Err(error("current Desktop does not have a working daemon"));
    }
    let mut intent = Intent {
        working: original.clone(),
        staged: approved.to_owned(),
        #[cfg(all(target_os = "macos", feature = "mac-owner-transport-update"))]
        transport: None,
    };
    intent.write_intent(&journal)?.persist_noclobber(&journal)?;
    fs::rename(&layout.active, &layout.previous)?;
    if let Err(cause) = fs::rename(&layout.pending, &layout.active) {
        fs::rename(&layout.previous, &layout.active)?;
        return Err(cause);
    }
    if package_digest(&layout.active)? != approved {
        return Err(error("activated package changed"));
    }
    let response = candidate_health(
        &before,
        |mode| probe(layout, approved, mode),
        |reported| intent.select_transport(layout, reported),
    );
    let candidate = match response {
        Ok(health) => health,
        Err(outcome) => {
            let mut checks = vec![before];
            checks.extend(intent.restore(layout)?);
            return reopen(layout, transaction, &original, outcome, checks);
        }
    };
    let mut checks = vec![before, candidate];
    let outcome = if checks[1].status == "incompatible" {
        fs::rename(&layout.active, &layout.pending)?;
        fs::rename(&layout.previous, &layout.active)?;
        if package_digest(&layout.active)? != original {
            return Err(error("previous package changed"));
        }
        let restored = require_connection_check(layout, &original)?;
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

fn candidate_health(
    before: &Health,
    mut run: impl FnMut(&str) -> io::Result<Probe>,
    mut select: impl FnMut([u8; 32]) -> io::Result<()>,
) -> Result<Health, &'static str> {
    match run(PROBE) {
        Ok(Probe::Replied(health)) => match check_candidate_key(before, &health, |mode| {
            let reply = run(mode)?;
            if let Probe::Replied(ref checked) = reply
                && let Some(prepared) = checked
                    .key_check
                    .as_ref()
                    .and_then(|check| check.staged_transport)
            {
                if mode != cryptex_updater::KEYCHAIN_REOPEN
                    || checked.status != "connected"
                    || checked.desktop_protocol != health.desktop_protocol
                    || !checked.requires_existing_key
                {
                    return Ok(Probe::InvalidReply);
                }
                select(prepared)?;
                let reopened = run(mode)?;
                if matches!(&reopened, Probe::Replied(check) if check.key_check.as_ref().is_some_and(|key| key.staged_transport.is_some()))
                {
                    return Ok(Probe::InvalidReply);
                }
                return Ok(reopened);
            }
            Ok(reply)
        }) {
            Ok(true) => Ok(health),
            Ok(false) => Err("rolled_back_keychain_check"),
            Err(_) => Err("rolled_back_keychain_io"),
        },
        Ok(Probe::TimedOut) => Err("rolled_back_timeout"),
        Ok(Probe::Failed) => Err("rolled_back_failed_probe"),
        Ok(Probe::InvalidReply) => Err("rolled_back_invalid_reply"),
        Err(_) => Err("rolled_back_probe_io"),
    }
}

fn main() -> io::Result<()> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [command] if command == "signer-fingerprint" => {
            println!("{}", release::signer_fingerprint()?);
            Ok(())
        }
        [command, directory] if command == "fingerprint" => {
            println!("{}", package_digest(Path::new(directory))?);
            Ok(())
        }
        [command, directory, approved] if command == "statement" => {
            let digest = package_digest(Path::new(directory))?;
            if approved.to_str() != Some(&digest) {
                return Err(error("package differs from approved fingerprint"));
            }
            io::stdout()
                .lock()
                .write_all(release::content(&digest).as_bytes())
        }
        [command, package, receipt] if command == "admit" => {
            release::admit(&Layout::current()?, Path::new(package), Path::new(receipt))
        }
        #[cfg(target_os = "macos")]
        [command, digest] if command == "remove-installed-app" => mac_removal::remove(
            &Layout::current()?,
            digest
                .to_str()
                .ok_or_else(|| error("expected approved package SHA-256"))?,
        ),
        [command, digest] if command == "apply" => {
            let hash = digest
                .to_str()
                .filter(|s| s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit()))
                .ok_or_else(|| error("expected SHA-256 package fingerprint"))?;
            apply(&Layout::current()?, &hash.to_ascii_lowercase())
        }
        _ => Err(error(
            "use fingerprint PATH, statement PATH APPROVED_SHA256, admit PATH RECEIPT or apply APPROVED_SHA256",
        )),
    }
}

#[cfg(test)]
mod keychain_update_tests {
    use super::*;
    use cryptex_updater::{KEYCHAIN_PROBE, KEYCHAIN_REOPEN};

    fn health(requires_existing_key: bool) -> Health {
        Health {
            status: "connected".into(),
            desktop_pid: 1,
            daemon_pid: 2,
            desktop_protocol: 1,
            daemon_protocol: 1,
            requires_existing_key,
            key_check: None,
        }
    }

    #[test]
    fn a_health_reply_or_interactive_receipt_cannot_satisfy_the_silent_probe() {
        let mut reply = health(true);
        assert!(key_reply_matches(PROBE, &reply));
        assert!(!key_reply_matches(KEYCHAIN_REOPEN, &reply));
        reply.key_check = Some(cryptex_updater::KeyCheck {
            prompted_access_allowed: true,
            verifier_pid: 3,
            staged_transport: None,
        });
        assert!(key_reply_matches(KEYCHAIN_PROBE, &reply));
        assert!(!key_reply_matches(PROBE, &reply));
        assert!(!key_reply_matches(KEYCHAIN_REOPEN, &reply));
        reply.key_check.as_mut().unwrap().prompted_access_allowed = false;
        assert!(key_reply_matches(KEYCHAIN_REOPEN, &reply));
        reply.key_check.as_mut().unwrap().verifier_pid = reply.desktop_pid;
        assert!(!key_reply_matches(KEYCHAIN_REOPEN, &reply));
    }

    #[test]
    fn staged_transport_requires_a_fresh_silent_check_after_selection() {
        for repeated_stage in [true, false] {
            let mut modes = vec![];
            let mut selected = vec![];
            let result = candidate_health(
                &health(true),
                |mode| {
                    modes.push(mode.to_owned());
                    let mut reply = health(true);
                    if mode != PROBE {
                        reply.key_check = Some(cryptex_updater::KeyCheck {
                            prompted_access_allowed: mode == KEYCHAIN_PROBE,
                            verifier_pid: 3,
                            staged_transport: (mode == KEYCHAIN_REOPEN
                                && (modes.len() == 3 || repeated_stage))
                                .then_some([42; 32]),
                        });
                    }
                    Ok(Probe::Replied(reply))
                },
                |fingerprint| {
                    selected.push(fingerprint);
                    Ok(())
                },
            );
            assert_eq!(
                modes,
                [PROBE, KEYCHAIN_PROBE, KEYCHAIN_REOPEN, KEYCHAIN_REOPEN]
            );
            assert_eq!(selected, [[42; 32]]);
            assert_eq!(result.is_ok(), !repeated_stage);
        }
    }

    #[test]
    fn failed_probe_io_selects_recovery_without_asking_again() {
        for (failed, outcome) in [
            (PROBE, "rolled_back_probe_io"),
            (KEYCHAIN_PROBE, "rolled_back_keychain_io"),
            (KEYCHAIN_REOPEN, "rolled_back_keychain_io"),
        ] {
            let mut seen = Vec::new();
            let result = candidate_health(
                &health(true),
                |mode| {
                    seen.push(mode.to_owned());
                    if mode == failed {
                        return Err(error("fixture pipe closed"));
                    }
                    Ok(Probe::Replied(health(true)))
                },
                |_| panic!("transport selection was not requested"),
            );
            assert_eq!(result.unwrap_err(), outcome);
            assert_eq!(seen.last().unwrap(), failed);
            assert!(
                seen.iter()
                    .filter(|mode| mode.as_str() == KEYCHAIN_PROBE)
                    .count()
                    <= 1
            );
        }
    }

    #[test]
    fn rejection_stops_after_one_prompt_and_one_time_access_cannot_commit() {
        for modes in [vec![KEYCHAIN_PROBE], vec![KEYCHAIN_PROBE, KEYCHAIN_REOPEN]] {
            let mut seen = Vec::new();
            let accepted = check_candidate_key(&health(true), &health(true), |mode| {
                seen.push(mode.to_owned());
                Ok(if seen.len() == modes.len() {
                    Probe::Failed
                } else {
                    Probe::Replied(health(true))
                })
            })
            .unwrap();
            assert!(!accepted);
            assert_eq!(seen, modes);
        }
    }

    #[test]
    fn persistent_upgrade_requires_a_second_silent_success_and_cannot_drop_capability() {
        let mut seen = Vec::new();
        assert!(
            check_candidate_key(&health(false), &health(true), |mode| {
                seen.push(mode.to_owned());
                Ok(Probe::Replied(health(true)))
            })
            .unwrap()
        );
        assert_eq!(seen, [KEYCHAIN_PROBE, KEYCHAIN_REOPEN]);
        for old in [true, false] {
            assert_eq!(
                check_candidate_key(&health(old), &health(false), |_| {
                    panic!("ordinary health should determine this outcome")
                })
                .unwrap(),
                !old
            );
        }
    }
}
