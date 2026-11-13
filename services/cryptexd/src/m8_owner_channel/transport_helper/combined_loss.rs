//! Fixture-only containment for losing BOTH actual supervisors. No engine API.
//! A guardian in the helper's own group cleans that group via kill(0), never a
//! stored/recyclable PID or PGID. It is not a production supervisor/resilience.
use super::*;
use std::os::unix::fs::FileTypeExt;

const GUARD: &str = "--m8-local-transport-fixture-guardian";
const DRIVER: &str = "--m8-local-transport-combined-loss-driver";
const PURPOSE: &[u8] = b"m8-fixture-group-guardian-v1\0";
const ARMED: &[u8] = b"m8-guardian-armed-v1";

fn guard_hello(role: u8, epoch: &[u8; 16], release: &Release, root: &Path) -> Vec<u8> {
    let mut bytes = PURPOSE.to_vec();
    bytes.extend(hello(role, epoch, release, root));
    bytes
}
pub(super) fn helper_hello(
    combined: bool,
    role: u8,
    epoch: &[u8; 16],
    release: &Release,
    root: &Path,
) -> Vec<u8> {
    let mut bytes = if combined {
        b"m8-joint-loss-v1\0".to_vec()
    } else {
        Vec::new()
    };
    bytes.extend(hello(role, epoch, release, root));
    bytes
}

pub(super) fn input_pipe() -> Result<Stdio, Error> {
    use std::os::fd::{AsRawFd, FromRawFd};
    let fd = unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_DUPFD_CLOEXEC, 3) };
    if fd < 0 {
        return Err(Error::Arguments);
    }
    // SAFETY: fresh owned duplicate, not the caller's stdin handle.
    let file = unsafe { File::from_raw_fd(fd) };
    if !file
        .metadata()
        .map_err(|_| Error::Arguments)?
        .file_type()
        .is_fifo()
        || file.as_raw_fd() <= 2
    {
        return Err(Error::Arguments);
    }
    Ok(Stdio::from(file))
}

pub(super) fn await_exit_unreaped(child: &Child, budget: Duration) -> Result<(), Error> {
    let until = Instant::now() + budget;
    loop {
        let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
        if unsafe {
            libc::waitid(
                libc::P_PID,
                child.id(),
                info.as_mut_ptr(),
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        } != 0
        {
            return Err(Error::Cleanup);
        }
        if unsafe { info.assume_init().si_pid() } == child.id() as i32 {
            return Ok(());
        }
        if Instant::now() >= until {
            return Err(Error::Deadline);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub(super) struct Guardian(Child);
impl Guardian {
    pub(super) fn start(root: &Path, release: &Release) -> Result<Self, Error> {
        if !role_active()
            || unsafe { libc::getpid() != libc::getpgrp() || libc::getsid(0) != libc::getpid() }
        {
            return Err(Error::Policy);
        }
        let (listener, epoch) = Listener::open().map_err(|_| Error::Channel)?;
        let child = Command::new(release.root.join("Contents/MacOS/cryptexd"))
            .args([
                GUARD,
                &epoch_text(&epoch),
                root.to_str().ok_or(Error::Arguments)?,
            ])
            .env_clear()
            .env(
                "TMPDIR",
                std::env::temp_dir()
                    .canonicalize()
                    .map_err(|_| Error::Profile)?,
            )
            .stdin(input_pipe()?)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|_| Error::Engine)?;
        let mut guard = Self(child);
        let deadline = Instant::now() + Duration::from_secs(15);
        await_hint(&mut guard.0, deadline)?;
        let expected = inventory_image(&release.bundle, Role::Daemon).map_err(|_| Error::Policy)?;
        let mut channel = listener
            .accept_until(&mut guard.0, &(&expected).into(), deadline)
            .map_err(|_| Error::Channel)?;
        let mut buffer = [0; 160];
        if channel
            .receive(&mut buffer, deadline)
            .map_err(|_| Error::Channel)?
            != guard_hello(3, &epoch, release, root)
        {
            return Err(Error::Channel);
        }
        channel
            .send(&guard_hello(4, &epoch, release, root), deadline)
            .map_err(|_| Error::Channel)?;
        if channel
            .receive(&mut buffer, deadline)
            .map_err(|_| Error::Channel)?
            != ARMED
        {
            return Err(Error::Channel);
        }
        channel.close().map_err(|_| Error::Cleanup)?;
        Ok(guard)
    }
}
impl Drop for Guardian {
    fn drop(&mut self) {
        // This exact child is still unreaped. Never target a ps-derived PID.
        if self.0.kill().is_err()
            || await_exit_unreaped(&self.0, Duration::from_secs(2)).is_err()
            || self.0.wait().is_err()
        {
            crate::owned_process::macos::kill_helper_group();
        }
    }
}

fn record(root: &Path, leaf: &str, bytes: &[u8]) -> Result<(), Error> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(root.join(leaf))
        .map_err(|_| Error::Profile)?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| Error::Profile)
}

fn guard(root: &Path, epoch: &[u8; 16]) -> Result<(), Error> {
    private_root(root)?;
    let parent = unsafe { libc::getppid() };
    if parent <= 1 || unsafe { libc::getpgrp() != parent || libc::getsid(0) != parent } {
        return Err(Error::Policy); // Unauthenticated direct invocation never arms a kill.
    }
    let _pipe_check = input_pipe()?;
    let deadline = Instant::now() + Duration::from_secs(15);
    let release = Release::current(Role::Daemon, deadline).map_err(|_| Error::Policy)?;
    let expected = inventory_image(&release.bundle, Role::Daemon).map_err(|_| Error::Policy)?;
    // Non-authoritative scheduling hint; the parent independently verifies us.
    std::io::stdout()
        .write_all(HINT)
        .and_then(|_| std::io::stdout().flush())
        .map_err(|_| Error::Channel)?;
    use std::os::fd::AsRawFd;
    let null = OpenOptions::new()
        .write(true)
        .open("/dev/null")
        .map_err(|_| Error::Channel)?;
    if unsafe { libc::dup2(null.as_raw_fd(), libc::STDOUT_FILENO) } < 0 {
        return Err(Error::Channel);
    }
    let mut channel =
        Channel::connect_until(epoch, &(&expected).into(), deadline).map_err(|_| Error::Channel)?;
    channel
        .send(&guard_hello(3, epoch, &release, root), deadline)
        .map_err(|_| Error::Channel)?;
    let mut buffer = [0; 160];
    if channel
        .receive(&mut buffer, deadline)
        .map_err(|_| Error::Channel)?
        != guard_hello(4, epoch, &release, root)
    {
        return Err(Error::Channel);
    }
    let pin = channel.peer_liveness().map_err(|_| Error::Channel)?;
    if unsafe {
        libc::getppid() != parent || libc::getpgrp() != parent || libc::getsid(0) != parent
    } {
        return Err(Error::Policy);
    }
    // No setsid/setpgid after authentication. Our own membership reserves the
    // cleanup group even after both supervisors exit and their PIDs are reaped.
    let result = (|| {
        let until = Instant::now() + Duration::from_secs(120);
        record(
            root,
            "guardian.ready",
            format!("{} {}", parent, unsafe { libc::getpid() }).as_bytes(),
        )?;
        channel
            .send(ARMED, Instant::now() + BUDGET)
            .map_err(|_| Error::Channel)?;
        // Retain our inbox through the entire armed lifetime. Native receive
        // must not accept queued ARMED bytes after its sender endpoint closes.
        while pin.live() {
            if Instant::now() >= until {
                return Err(Error::Deadline);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        record(root, "guardian.parent-lost", b"lost\n")?;
        loop {
            if Instant::now() >= until {
                return Err(Error::Deadline);
            }
            let mut fd = libc::pollfd {
                fd: libc::STDIN_FILENO,
                events: libc::POLLIN | libc::POLLHUP,
                revents: 0,
            };
            let count = unsafe { libc::poll(&mut fd, 1, 50) };
            if count < 0 {
                return Err(Error::Channel);
            }
            if count > 0 {
                let mut byte = [0];
                return match std::io::stdin().read(&mut byte) {
                    Ok(0) => Ok(()), // Collector loss still cleans our own group.
                    Ok(1) if byte == [b'K'] => Ok(()),
                    _ => Err(Error::Channel),
                };
            }
        }
    })();
    if let Err(error) = result {
        eprintln!("component=combined_loss_guard status=failed error={error}");
    }
    // Apple kill(2): pid 0 addresses the sender's CURRENT process group.
    // We cannot accidentally signal a recycled numeric group identifier.
    unsafe {
        libc::kill(0, libc::SIGKILL);
        libc::_exit(87);
    }
}

fn driver(root: &Path) -> Result<(), Error> {
    let _pipe_check = input_pipe()?;
    let worker = Worker::spawn_inner(root.to_owned(), true, true)?;
    wait_identity(worker.status_reader())?;
    let group = worker.reader.call(Work::FixtureCombinedLoss)?;
    println!("mac_transport_combined_ready={group}");
    std::io::stdout().flush().map_err(|_| Error::Channel)?;
    // Worker owns the exact exited helper unreaped and parks without Drop.
    // Collector now kills only its own Popen daemon; guardian/engines stay live.
    std::thread::sleep(Duration::from_secs(60));
    worker.close()?;
    Err(Error::Deadline)
}

pub(super) fn entry(args: &[String]) -> Option<Result<(), Error>> {
    match args.first().map(String::as_str) {
        Some(GUARD) => Some((|| {
            if args.len() != 3 {
                return Err(Error::Arguments);
            }
            guard(Path::new(&args[2]), &epoch(&args[1])?)
        })()),
        Some(DRIVER) => Some(if args.len() == 2 {
            driver(Path::new(&args[1]))
        } else {
            Err(Error::Arguments)
        }),
        Some("--m8-local-transport-combined-integrity-check") => Some((|| {
            if args.len() != 2 {
                return Err(Error::Arguments);
            }
            let root = Path::new(&args[1]);
            private_root(root)?;
            let _release = Release::current(Role::Daemon, Instant::now() + Duration::from_secs(15))
                .map_err(|_| Error::Policy)?;
            let profile = ManagedProfile::open(&root.join(PROFILE), SecureStoreEnvironment::Dev)
                .map_err(|_| Error::Profile)?;
            // Read/validate the EXISTING OS key and binding only. No create,
            // delete, regeneration or credential material in the output.
            let _credential = profile
                .load_api_credential(&MacosKeychainStore::new())
                .map_err(|_| Error::CredentialUnavailable)?;
            println!("mac_transport_combined_credential_continuity=passed");
            Ok(())
        })()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixture_purposes_and_roles_never_match_the_transport_protocol() {
        // Independent of a Release object, the fixed purpose prefixes cannot
        // alias ordinary transport frames or each other.
        assert_ne!(PURPOSE, b"m8-transport-only-v1\0");
        assert_ne!(PURPOSE, b"m8-joint-loss-v1\0");
        assert_ne!(ARMED, STOP_ACK);
        for flag in [
            GUARD,
            DRIVER,
            "--m8-local-transport-combined-integrity-check",
        ] {
            assert_eq!(entry(&[flag.into()]), Some(Err(Error::Arguments)));
        }
        assert!(entry(&[FLAG.into()]).is_none());
    }
}
