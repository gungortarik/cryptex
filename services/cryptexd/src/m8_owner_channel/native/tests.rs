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

fn finish(child: &mut Child, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "native test child exited with {status}");
            return;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            child.wait().unwrap();
            panic!("native test child exceeded its deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn unhex(text: &str) -> Vec<u8> {
    assert!(text.is_ascii() && text.len().is_multiple_of(2));
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
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
            let _ = fs::remove_file(self.0.join("native-tests"));
            let _ = fs::remove_dir(&self.0);
        }
    }
    let _cleanup = Cleanup(dir.clone());
    let binary = dir.join("native-tests");
    fs::copy(std::env::current_exe().unwrap(), &binary).unwrap();
    assert!(
        Command::new("/usr/bin/codesign")
            .args([
                "--force",
                "--sign",
                "-",
                "--timestamp=none",
                "--identifier",
                SIGNING_ID
            ])
            .arg(&binary)
            .status()
            .unwrap()
            .success()
    );
    let output = Command::new("/usr/bin/codesign")
        .args(["-d", "--verbose=4"])
        .arg(&binary)
        .output()
        .unwrap();
    assert!(output.status.success());
    let info = String::from_utf8(output.stderr).unwrap();
    let hash = info
        .lines()
        .find_map(|s| s.strip_prefix("CDHash="))
        .unwrap();
    assert_eq!(unhex(hash).len(), 20);
    let mut supervisor = helper(&binary, "supervisor")
        .env("CX_TEST_CDHASH", hash)
        .spawn()
        .unwrap();
    finish(&mut supervisor, Duration::from_secs(60));
}

#[test]
#[ignore = "invoked in an independently ad-hoc signed test executable"]
fn supervisor() {
    let binary = std::env::current_exe().unwrap();
    let hash = unhex(&std::env::var("CX_TEST_CDHASH").unwrap());
    let mut previous = None;
    for case in [
        "roundtrip",
        "wrong_hash",
        "wrong_requirement",
        "queue_timeout",
        "restart",
    ] {
        let (listener, epoch) = Listener::open().unwrap();
        assert_ne!(Some(epoch), previous);
        previous = Some(epoch);
        let mut child = helper(&binary, "worker")
            .env("CX_TEST_EPOCH", hex(&epoch))
            .env("CX_TEST_CASE", case)
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let policy = ImagePolicy {
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
        let accepted = listener.accept(&mut child, &policy);
        if case.starts_with("wrong_") {
            assert!(
                matches!(accepted, Err(Error::Image)),
                "expected peer image rejection"
            );
            finish(&mut child, Duration::from_secs(8));
            continue;
        }
        let mut channel = accepted.unwrap();
        if case == "queue_timeout" {
            let mut result = Ok(());
            for _ in 0..64 {
                result = channel.send(b"bounded queue");
                if result.is_err() {
                    break;
                }
            }
            assert_eq!(result, Err(Error::Deadline));
            assert_eq!(channel.send(b"closed"), Err(Error::Closed));
        } else {
            let mut buffer = [0; 128];
            assert_eq!(
                channel.receive(&mut buffer).unwrap(),
                b"worker to supervisor"
            );
            channel.send(b"supervisor to worker").unwrap();
            assert_eq!(channel.receive(&mut buffer).unwrap(), b"ack");
        }
        drop(child.stdin.take());
        finish(&mut child, Duration::from_secs(8));
        if case != "queue_timeout" {
            assert_eq!(channel.send(b"old session"), Err(Error::Instance));
            assert_eq!(channel.receive(&mut [0; 16]), Err(Error::Closed));
        }
    }
}

#[test]
#[ignore = "invoked as the supervised child of the signed test executable"]
fn worker() {
    let epoch: Epoch = unhex(&std::env::var("CX_TEST_EPOCH").unwrap())
        .try_into()
        .unwrap();
    let case = std::env::var_os("CX_TEST_CASE").unwrap();
    let hash = unhex(&std::env::var("CX_TEST_CDHASH").unwrap());
    let connected = Channel::connect(
        &epoch,
        &ImagePolicy {
            requirement: REQUIREMENT,
            cdhash: &hash,
        },
    );
    if case.to_str().unwrap().starts_with("wrong_") {
        assert!(matches!(connected, Err(Error::Deadline)));
        return;
    }
    let mut channel = connected.unwrap();
    if case != OsStr::new("queue_timeout") {
        channel.send(b"worker to supervisor").unwrap();
        assert_eq!(
            channel.receive(&mut [0; 128]).unwrap(),
            b"supervisor to worker"
        );
        channel.send(b"ack").unwrap();
    }
    // The parent explicitly releases us only after it has checked the last
    // received message. A queued packet from an exited sender must fail closed.
    assert_eq!(std::io::stdin().read(&mut [0; 1]).unwrap(), 0);
}
