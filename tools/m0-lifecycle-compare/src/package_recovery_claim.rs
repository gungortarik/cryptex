//! Test-only stale claim cleanup, serialized by a native filesystem lock.
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use cryptexd::error::LifecycleError;
use fs2::FileExt;
use serde_json::{Value, json};

#[derive(Debug)]
struct ClaimError(&'static str);
type Result<T> = std::result::Result<T, ClaimError>;

fn ordinary(path: &Path, directory: bool) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| ClaimError("CLAIM_IO_FAILED"))?;
    let unsafe_path = metadata.file_type().is_symlink();
    #[cfg(windows)]
    let unsafe_path = {
        use std::os::windows::fs::MetadataExt;
        unsafe_path || metadata.file_attributes() & 0x400 != 0
    };
    if unsafe_path || if directory { !metadata.is_dir() } else { !metadata.is_file() } {
        return Err(ClaimError("UNSAFE_ROOT"));
    }
    Ok(())
}

fn validate_root(root: &Path) -> Result<()> {
    if !root.is_absolute() || root.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err(ClaimError("UNSAFE_ROOT"));
    }
    for ancestor in root.ancestors() {
        ordinary(ancestor, true)?;
    }
    let repository = fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .map_err(|_| ClaimError("UNSAFE_ROOT"))?;
    let resolved = fs::canonicalize(root).map_err(|_| ClaimError("UNSAFE_ROOT"))?;
    if resolved.starts_with(repository) {
        return Err(ClaimError("UNSAFE_ROOT"));
    }
    ordinary(&root.join("experiment.json"), false)?;
    let marker: Value = serde_json::from_slice(&fs::read(root.join("experiment.json"))
        .map_err(|_| ClaimError("CLAIM_IO_FAILED"))?).map_err(|_| ClaimError("UNOWNED_ROOT"))?;
    if marker["schema"] != 1 || marker["scope"] != "m0-package-filesystem-candidate" {
        return Err(ClaimError("UNOWNED_ROOT"));
    }
    Ok(())
}

fn owner(bytes: &[u8]) -> Result<u32> {
    if bytes.len() > 4096 { return Err(ClaimError("INVALID_RECOVERY_CLAIM")); }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| ClaimError("INVALID_RECOVERY_CLAIM"))?;
    let pid = value["pid"].as_u64().and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v > 0).ok_or(ClaimError("INVALID_RECOVERY_CLAIM"))?;
    let token = value["token"].as_str().ok_or(ClaimError("INVALID_RECOVERY_CLAIM"))?;
    let valid_token = token.len() == 36 && token.bytes().enumerate().all(|(i, b)| {
        if [8, 13, 18, 23].contains(&i) { b == b'-' } else { b.is_ascii_digit() || (b'a'..=b'f').contains(&b) }
    });
    if value["schema"] != 1 || !valid_token { return Err(ClaimError("INVALID_RECOVERY_CLAIM")); }
    Ok(pid)
}

fn prove_owner_exited(node: &Path, pid: u32) -> Result<()> {
    if !node.is_absolute() { return Err(ClaimError("INVALID_NODE_PATH")); }
    ordinary(node, false)?;
    let script = "try { process.kill(Number(process.argv[1]), 0); process.exit(2); } catch (e) { process.exit(e.code === 'ESRCH' ? 0 : 3); }";
    let mut command = Command::new(node);
    command.args(["-e", script, &pid.to_string()]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().map_err(|_| ClaimError("OWNER_PROBE_FAILED"))?;
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return match status.code() {
                Some(0) => Ok(()), Some(2) => Err(ClaimError("RECOVERY_OWNER_ACTIVE")),
                _ => Err(ClaimError("RECOVERY_OWNER_UNKNOWN")),
            },
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            _ => {
                child.kill().map_err(|_| ClaimError("OWNER_PROBE_CLEANUP_FAILED"))?;
                child.wait().map_err(|_| ClaimError("OWNER_PROBE_CLEANUP_FAILED"))?;
                return Err(ClaimError("OWNER_PROBE_FAILED"));
            }
        }
    }
}

fn clear(root: &Path, node: &Path, hold_ms: u64) -> Result<Value> {
    validate_root(root)?;
    let lock_path = root.join("recovery-native.lock");
    if lock_path.try_exists().map_err(|_| ClaimError("CLAIM_IO_FAILED"))? { ordinary(&lock_path, false)?; }
    let lock = OpenOptions::new().read(true).write(true).create(true).truncate(false)
        .open(&lock_path).map_err(|_| ClaimError("CLAIM_IO_FAILED"))?;
    lock.try_lock_exclusive().map_err(|e| {
        if e.kind() == std::io::ErrorKind::WouldBlock || e.raw_os_error() == Some(33) { ClaimError("NATIVE_RECOVERY_BUSY") }
        else { ClaimError("NATIVE_LOCK_FAILED") }
    })?;
    // Test checkpoint only. The lock file is never unlinked: all cleaners use
    // the same OS lock object, released automatically when this process exits.
    if hold_ms > 0 {
        println!("{}", json!({"schema": 1, "locked": true}));
        std::io::stdout().flush().map_err(|_| ClaimError("CLAIM_IO_FAILED"))?;
        thread::sleep(Duration::from_millis(hold_ms));
    }
    let claim = root.join("recovery-claim.json");
    if !claim.try_exists().map_err(|_| ClaimError("CLAIM_IO_FAILED"))? {
        return Ok(json!({"schema": 1, "ok": true, "reclaimed": false}));
    }
    ordinary(&claim, false)?;
    let bytes = fs::read(&claim).map_err(|_| ClaimError("CLAIM_IO_FAILED"))?;
    let pid = owner(&bytes)?;
    prove_owner_exited(node, pid)?;
    // Legacy recovery callers cannot replace an existing claim. Competing
    // native cleaners are excluded by the held OS lock during check + unlink.
    ordinary(&claim, false)?;
    if fs::read(&claim).map_err(|_| ClaimError("CLAIM_IO_FAILED"))? != bytes {
        return Err(ClaimError("RECOVERY_CLAIM_CHANGED"));
    }
    fs::remove_file(&claim).map_err(|_| ClaimError("CLAIM_IO_FAILED"))?;
    #[cfg(not(windows))]
    fs::File::open(root).and_then(|f| f.sync_all()).map_err(|_| ClaimError("CLAIM_IO_FAILED"))?;
    Ok(json!({"schema": 1, "ok": true, "reclaimed": true, "owner_pid": pid}))
}

pub(crate) fn run(args: &[String]) -> std::result::Result<(), LifecycleError> {
    let mut root = None; let mut node = None; let mut hold_ms = None;
    for pair in args.chunks(2) {
        if pair.len() != 2 { return Err(LifecycleError::Usage("claim option needs a value".into())); }
        match pair[0].as_str() {
            "--root" if root.is_none() => root = Some(PathBuf::from(&pair[1])),
            "--node" if node.is_none() => node = Some(PathBuf::from(&pair[1])),
            "--hold-ms" if hold_ms.is_none() => hold_ms = Some(pair[1].parse::<u64>().ok().filter(|n| *n <= 10000)
                .ok_or_else(|| LifecycleError::Usage("hold-ms must be 0..10000".into()))?),
            _ => return Err(LifecycleError::Usage("unknown or duplicate claim option".into())),
        }
    }
    let root = root.ok_or_else(|| LifecycleError::Usage("root required".into()))?;
    let node = node.ok_or_else(|| LifecycleError::Usage("node required".into()))?;
    match clear(&root, &node, hold_ms.unwrap_or(0)) {
        Ok(result) => { println!("{result}"); Ok(()) }
        Err(error) => {
            println!("{}", json!({"schema": 1, "ok": false, "code": error.0}));
            Err(LifecycleError::Attention(error.0.into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn valid_owner() {
        assert_eq!(owner(br#"{"schema":1,"pid":123,"token":"12345678-1234-1234-1234-123456789abc"}"#).unwrap(), 123);
    }
    #[test] fn invalid_owner() {
        for value in [br#"{"schema":1,"pid":0,"token":"bad"}"#.as_slice(), b"{", br#"{"schema":2,"pid":12,"token":"12345678-1234-1234-1234-123456789abc"}"#] {
            assert_eq!(owner(value).unwrap_err().0, "INVALID_RECOVERY_CLAIM");
        }
    }
    #[test] fn repository_refused() {
        let repository = fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.." )).unwrap();
        assert_eq!(validate_root(&repository).unwrap_err().0, "UNSAFE_ROOT");
    }
}
