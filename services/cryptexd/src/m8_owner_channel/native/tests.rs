use super::*;
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const PREFIX: &str = "m8_owner_channel::native::tests::";
const SIGNING_ID: &str = "com.cryptex.m8.channel-tests";
const REQUIREMENT: &CStr = c"identifier \"com.cryptex.m8.channel-tests\"";

// This selector exists only in the Rust test binary. The signed fixture is
// temporary, has no entitlements and never opens a real Cryptex profile.
fn helper(binary: &Path, test: &str) -> Command {
    let mut cmd = Command::new(binary);
    cmd.args([
        "--exact",
        &format!("{PREFIX}{test}"),
        "--ignored",
        "--nocapture",
    ]);
    cmd
}

// Keep the group leader waitable until its group is cleaned up. This prevents
// PID reuse between observing its exit and signalling leftover helpers.
struct TestChild {
    child: Child,
    group_leader: bool,
    reaped: bool,
}

impl TestChild {
    fn spawn(cmd: &mut Command, group_leader: bool) -> Self {
        use std::os::unix::process::CommandExt;
        if group_leader {
            cmd.process_group(0);
        }
        Self {
            child: cmd.spawn().unwrap(),
            group_leader,
            reaped: false,
        }
    }

    fn stop_group(&self) -> std::io::Result<()> {
        if self.group_leader {
            // SAFETY: this private group was created at spawn and its leader
            // has not been reaped. We never signal our caller's group.
            let result = unsafe { libc::killpg(self.child.id() as i32, libc::SIGKILL) };
            if result != 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    fn finish(&mut self, timeout: Duration) {
        let status = self.wait_status(timeout);
        assert!(status.success(), "native test child exited with {status}");
    }

    fn wait_status(&mut self, timeout: Duration) -> std::process::ExitStatus {
        let deadline = Instant::now() + timeout;
        loop {
            let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
            // SAFETY: a writable siginfo and the PID of our unreaped child.
            let result = unsafe {
                libc::waitid(
                    libc::P_PID,
                    self.child.id() as libc::id_t,
                    info.as_mut_ptr(),
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            };
            if result == 0 && unsafe { info.assume_init().si_pid() } != 0 {
                if let Err(error) = self.stop_group() {
                    // Darwin may return EPERM when the only remaining group
                    // members are zombies. Check while the PGID is reserved.
                    assert_eq!(error.raw_os_error(), Some(libc::EPERM));
                    assert!(all_group_members_dead(self.child.id()));
                }
                let status = self.child.wait().unwrap();
                self.reaped = true;
                return status;
            }
            if result != 0 {
                assert_eq!(
                    std::io::Error::last_os_error().kind(),
                    std::io::ErrorKind::Interrupted
                );
            }
            assert!(
                Instant::now() < deadline,
                "native test child exceeded its deadline"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for TestChild {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.stop_group();
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn all_group_members_dead(pgid: u32) -> bool {
    let pgid = pgid.to_string();
    let mut ps = TestChild::spawn(
        Command::new("/bin/ps")
            .args(["-x", "-g", &pgid, "-o", "pgid=,stat="])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
        false,
    );
    let status = ps.wait_status(Duration::from_secs(2));
    let mut states = String::new();
    let mut unexpected = [0u8; 1];
    matches!(status.code(), Some(0 | 1))
        && ps
            .child
            .stderr
            .take()
            .unwrap()
            .read(&mut unexpected)
            .unwrap()
            == 0
        && ps
            .child
            .stdout
            .take()
            .unwrap()
            .take(16385)
            .read_to_string(&mut states)
            .is_ok()
        && states.len() <= 16384
        && states.lines().all(|line| {
            let mut fields = line.split_whitespace();
            fields.next() == Some(pgid.as_str())
                && fields.next().is_some_and(|s| s.starts_with('Z'))
                && fields.next().is_none()
        })
}

fn unhex(text: &str) -> Vec<u8> {
    assert!(text.is_ascii() && text.len().is_multiple_of(2));
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_millis(2500)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|v| format!("{v:02x}")).collect()
}

#[test]
fn native_supervised_channel_roundtrip_and_refusals() {
    let mut nonce = [0; 16];
    getrandom::getrandom(&mut nonce).unwrap();
    let dir = std::env::temp_dir().join(format!("cryptex-channel-{}", hex(&nonce)));
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_file(self.0.join("worker-restarting"));
            let _ = fs::remove_file(self.0.join("worker-restarted"));
            let _ = fs::remove_file(self.0.join("supervisor.ok"));
            let _ = fs::remove_file(self.0.join("native-tests"));
            let _ = fs::remove_dir(&self.0);
        }
    }
    let _cleanup = Cleanup(dir.clone());
    let binary = dir.join("native-tests");
    fs::copy(std::env::current_exe().unwrap(), &binary).unwrap();
    // These controls exist only in the test binary. The runner provisions a
    // disposable identity or explicitly selects the owner's existing identity.
    let signer = std::env::var("CX_NATIVE_TEST_SIGNER").unwrap_or_else(|_| "-".into());
    let mut signing = Command::new("/usr/bin/codesign");
    signing.args([
        "--force",
        "--sign",
        &signer,
        "--timestamp=none",
        "--identifier",
        SIGNING_ID,
    ]);
    if let Some(keychain) = std::env::var_os("CX_NATIVE_TEST_KEYCHAIN") {
        signing.arg("--keychain").arg(keychain);
    }
    signing.arg(&binary);
    TestChild::spawn(&mut signing, true).finish(Duration::from_secs(30));
    let mut inspect = TestChild::spawn(
        Command::new("/usr/bin/codesign")
            .args(["-d", "--verbose=4"])
            .arg(&binary)
            .stderr(Stdio::piped()),
        true,
    );
    inspect.finish(Duration::from_secs(10));
    let mut info = String::new();
    inspect
        .child
        .stderr
        .take()
        .unwrap()
        .take(16385)
        .read_to_string(&mut info)
        .unwrap();
    assert!(info.len() <= 16384);
    let hash = info
        .lines()
        .find_map(|s| s.strip_prefix("CDHash="))
        .unwrap();
    assert_eq!(unhex(hash).len(), 20);
    let mut supervisor = helper(&binary, "supervisor");
    supervisor.env("CX_TEST_CDHASH", hash);
    if signer != "-" {
        let cert = fs::read(std::env::var_os("CX_NATIVE_TEST_CERT_DER").unwrap()).unwrap();
        assert!(!cert.is_empty() && cert.len() <= 65536);
        supervisor.env("CX_TEST_LEAF_DER", hex(&cert));
    }
    TestChild::spawn(&mut supervisor, true).finish(Duration::from_secs(60));
    // An exact test filter with no matching test also exits zero.
    assert_eq!(fs::read(dir.join("supervisor.ok")).unwrap(), b"finished");
}

#[test]
#[ignore = "invoked in an independently ad-hoc signed test executable"]
fn supervisor() {
    let binary = std::env::current_exe().unwrap();
    let hash = unhex(&std::env::var("CX_TEST_CDHASH").unwrap());
    let leaf = std::env::var("CX_TEST_LEAF_DER")
        .ok()
        .map(|value| unhex(&value));
    let mut previous = None;
    let mut old_channel: Option<Channel> = None;
    for case in [
        "roundtrip",
        "wrong_hash",
        "wrong_requirement",
        "wrong_leaf",
        "short_handshake",
        "short_connect",
        "wrong_child",
        "parent_image",
        "queued_after_exit",
        "same_pid_restart",
        "expired",
        "queue_timeout",
        "restart",
    ] {
        let (listener, epoch) = Listener::open().unwrap();
        assert_ne!(Some(epoch), previous);
        previous = Some(epoch);
        if case == "short_handshake" {
            let mut child = TestChild::spawn(
                helper(&binary, "worker")
                    .env("CX_TEST_CASE", "idle")
                    .stdin(Stdio::piped()),
                false,
            );
            let policy = ImagePolicy {
                requirement: REQUIREMENT,
                cdhash: &hash,
                signer_der: leaf.as_deref(),
            };
            assert!(matches!(
                Channel::connect_until(&epoch, &policy, Instant::now()),
                Err(Error::Deadline)
            ));
            let start = Instant::now();
            assert!(matches!(
                listener.accept_until(
                    &mut child.child,
                    &policy,
                    start + Duration::from_millis(100)
                ),
                Err(Error::Deadline)
            ));
            // The default handshake budget is 2.5s. No HELLO is ever sent.
            assert!(start.elapsed() < Duration::from_secs(2));
            drop(child.child.stdin.take());
            child.finish(Duration::from_secs(8));
            continue;
        }
        if case == "short_connect" {
            let mut child = TestChild::spawn(
                helper(&binary, "worker")
                    .env("CX_TEST_EPOCH", hex(&epoch))
                    .env("CX_TEST_CASE", case)
                    .stdin(Stdio::piped()),
                false,
            );
            // The real listener remains live but never accepts or sends ACK.
            child.finish(Duration::from_secs(8));
            drop(listener);
            continue;
        }
        let mut child = TestChild::spawn(
            helper(&binary, "worker")
                .env("CX_TEST_EPOCH", hex(&epoch))
                .env("CX_TEST_CASE", case)
                .stdin(Stdio::piped()),
            false,
        );
        let policy = ImagePolicy {
            signer_der: if case == "wrong_leaf" {
                Some(include_bytes!("../release_inventory/fixtures/signer.der"))
            } else {
                leaf.as_deref()
            },
            requirement: if case == "wrong_requirement" {
                c"identifier \"com.cryptex.other\""
            } else {
                REQUIREMENT
            },
            cdhash: if case == "wrong_hash" {
                &[0; 20]
            } else {
                &hash
            },
        };
        if case == "wrong_child" {
            // Both processes use the accepted signed image. Only the handle
            // given to accept is eligible to send this session's HELLO.
            let mut expected = TestChild::spawn(
                helper(&binary, "worker")
                    .env("CX_TEST_CASE", "idle")
                    .stdin(Stdio::piped()),
                false,
            );
            assert!(matches!(
                listener.accept(&mut expected.child, &policy),
                Err(Error::Instance)
            ));
            drop(expected.child.stdin.take());
            expected.finish(Duration::from_secs(8));
            child.finish(Duration::from_secs(8));
            continue;
        }
        let accepted = listener.accept(&mut child.child, &policy);
        if case.starts_with("wrong_") {
            assert!(
                matches!(accepted, Err(Error::Image)),
                "expected peer image rejection"
            );
            child.finish(Duration::from_secs(8));
            continue;
        }
        let mut channel = accepted.unwrap();
        if case == "expired" {
            assert_eq!(
                channel.send(b"expired", Instant::now()),
                Err(Error::Deadline)
            );
            child.finish(Duration::from_secs(8));
            continue;
        }
        if case == "parent_image" {
            child.finish(Duration::from_secs(8));
            assert_eq!(
                channel.send(b"old session", deadline()),
                Err(Error::Instance)
            );
            continue;
        }
        if case == "queued_after_exit" {
            child.finish(Duration::from_secs(8));
            let mut buffer = [0; 32];
            assert_eq!(
                channel.receive(&mut buffer, deadline()),
                Err(Error::Instance)
            );
            assert_eq!(buffer, [0; 32]);
            continue;
        }
        if case == "same_pid_restart" {
            let receipt = binary.with_file_name("worker-restarted");
            let until = Instant::now() + Duration::from_secs(8);
            while !receipt.exists() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(10));
            }
            assert_eq!(
                fs::read_to_string(receipt).unwrap(),
                child.child.id().to_string()
            );
            let mut bytes = [0; 64];
            assert_eq!(
                channel.receive(&mut bytes, deadline()),
                Err(Error::Instance)
            );
            assert_eq!(bytes, [0; 64]);
            drop(child.child.stdin.take());
            child.finish(Duration::from_secs(8));
            continue;
        }
        if case == "queue_timeout" {
            let mut result = Ok(());
            for _ in 0..64 {
                result = channel.send(b"bounded queue", deadline());
                if result.is_err() {
                    break;
                }
            }
            assert_eq!(result, Err(Error::Deadline));
            assert_eq!(channel.send(b"closed", deadline()), Err(Error::Closed));
        } else {
            if case == "restart" {
                let mut stale = old_channel.take().unwrap();
                assert_eq!(stale.send(b"old session", deadline()), Err(Error::Instance));
                assert_eq!(stale.receive(&mut [0; 16], deadline()), Err(Error::Closed));
            }
            let mut buffer = [0; 128];
            assert_eq!(
                channel.receive(&mut buffer, deadline()).unwrap(),
                b"worker to supervisor"
            );
            channel.send(b"supervisor to worker", deadline()).unwrap();
            assert_eq!(channel.receive(&mut buffer, deadline()).unwrap(), b"ack");
        }
        drop(child.child.stdin.take());
        child.finish(Duration::from_secs(8));
        if case == "roundtrip" {
            old_channel = Some(channel);
        } else if case != "queue_timeout" {
            assert_eq!(
                channel.send(b"old session", deadline()),
                Err(Error::Instance)
            );
            assert_eq!(
                channel.receive(&mut [0; 16], deadline()),
                Err(Error::Closed)
            );
        }
    }
    fs::write(binary.with_file_name("supervisor.ok"), b"finished").unwrap();
}

#[test]
#[ignore = "invoked as the supervised child of the signed test executable"]
fn worker() {
    let case = std::env::var_os("CX_TEST_CASE").unwrap();
    if case == OsStr::new("restarted") {
        let bin = std::env::current_exe().unwrap();
        // Publish the marker atomically; the parent reads only a complete PID.
        let marker = bin.with_file_name("worker-restarting");
        fs::write(&marker, std::process::id().to_string()).unwrap();
        fs::rename(marker, bin.with_file_name("worker-restarted")).unwrap();
    }
    if case == OsStr::new("idle") || case == OsStr::new("restarted") {
        assert_eq!(std::io::stdin().read(&mut [0; 1]).unwrap(), 0);
        return;
    }
    let epoch: Epoch = unhex(&std::env::var("CX_TEST_EPOCH").unwrap())
        .try_into()
        .unwrap();
    let hash = unhex(&std::env::var("CX_TEST_CDHASH").unwrap());
    let leaf = std::env::var("CX_TEST_LEAF_DER")
        .ok()
        .map(|value| unhex(&value));
    if case == OsStr::new("short_connect") {
        let start = Instant::now();
        assert!(matches!(
            Channel::connect_until(
                &epoch,
                &ImagePolicy {
                    requirement: REQUIREMENT,
                    cdhash: &hash,
                    signer_der: leaf.as_deref(),
                },
                start + Duration::from_millis(100)
            ),
            Err(Error::Deadline)
        ));
        assert!(start.elapsed() < Duration::from_secs(2));
        return;
    }
    let connected = Channel::connect(
        &epoch,
        &ImagePolicy {
            signer_der: leaf.as_deref(),
            requirement: REQUIREMENT,
            cdhash: if case == OsStr::new("parent_image") {
                &[0; 20]
            } else {
                &hash
            },
        },
    );
    if case.to_str().unwrap().starts_with("wrong_") {
        assert!(matches!(connected, Err(Error::Deadline)));
        return;
    }
    if case == OsStr::new("parent_image") {
        assert!(matches!(connected, Err(Error::Image)));
        return;
    }
    let mut channel = connected.unwrap();
    if case == OsStr::new("expired") {
        assert_eq!(
            channel.receive(&mut [0; 32], deadline()),
            Err(Error::Deadline)
        );
        return;
    }
    if case == OsStr::new("queued_after_exit") {
        channel.send(b"queued before exit", deadline()).unwrap();
        return;
    }
    if case == OsStr::new("same_pid_restart") {
        use std::os::unix::process::CommandExt;
        channel
            .send(b"from the old generation", deadline())
            .unwrap();
        channel.close().unwrap();
        let error = helper(&std::env::current_exe().unwrap(), "worker")
            .env("CX_TEST_CASE", "restarted")
            .exec();
        panic!("test worker could not restart: {error}");
    }
    if case != OsStr::new("queue_timeout") {
        channel.send(b"worker to supervisor", deadline()).unwrap();
        assert_eq!(
            channel.receive(&mut [0; 128], deadline()).unwrap(),
            b"supervisor to worker"
        );
        channel.send(b"ack", deadline()).unwrap();
    }
    // The parent explicitly releases us only after it has checked the last
    // received message. A queued packet from an exited sender must fail closed.
    assert_eq!(std::io::stdin().read(&mut [0; 1]).unwrap(), 0);
}
