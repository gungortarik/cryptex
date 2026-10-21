//! Test-only OS gate for cooperating native handover/startup callers.
use std::fs::{self, OpenOptions};
use std::io::{BufRead, Read, Write};
use std::path::Path;

use cryptexd::error::LifecycleError;
use fs2::FileExt;
use serde_json::json;

fn hold(root: &Path) -> Result<(), &'static str> {
    super::package_recovery_claim::validate_handover_root(root)?;
    let file = root.join("handover-native.lock");
    match fs::symlink_metadata(&file) {
        Ok(metadata) => {
            let unsafe_file = metadata.file_type().is_symlink() || !metadata.is_file();
            #[cfg(windows)]
            let unsafe_file = {
                use std::os::windows::fs::MetadataExt;
                unsafe_file || metadata.file_attributes() & 0x400 != 0
            };
            if unsafe_file {
                return Err("UNSAFE_HANDOVER_LOCK");
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("HANDOVER_IO_FAILED"),
    }
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(file)
        .map_err(|_| "HANDOVER_IO_FAILED")?;
    lock.try_lock_exclusive().map_err(|error| {
        if error.kind() == std::io::ErrorKind::WouldBlock || error.raw_os_error() == Some(33) {
            "HANDOVER_BUSY"
        } else {
            "HANDOVER_LOCK_FAILED"
        }
    })?;
    println!(
        "{}",
        json!({"schema": 1, "locked": true, "pid": std::process::id()})
    );
    std::io::stdout()
        .flush()
        .map_err(|_| "HANDOVER_IO_FAILED")?;
    // Bounded control message, unbounded lease lifetime. EOF is abnormal, but
    // dropping the file releases the OS lock even when the caller dies.
    let mut command = String::new();
    std::io::stdin()
        .lock()
        .take(64)
        .read_line(&mut command)
        .map_err(|_| "HANDOVER_CONTROL_FAILED")?;
    if command != "release\n" {
        return Err("HANDOVER_CONTROL_FAILED");
    }
    drop(lock);
    println!("{}", json!({"schema": 1, "released": true}));
    Ok(())
}

pub(crate) fn run(args: &[String]) -> Result<(), LifecycleError> {
    if args.len() != 2 || args[0] != "--root" {
        return Err(LifecycleError::Usage(
            "handover requires --root PATH".into(),
        ));
    }
    match hold(Path::new(&args[1])) {
        Ok(()) => Ok(()),
        Err(code) => {
            println!("{}", json!({"schema": 1, "ok": false, "code": code}));
            Err(LifecycleError::Attention(code.into()))
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn options_require_exact_root_pair() {
        for args in [
            vec![],
            vec!["--root".into()],
            vec!["--other".into(), "x".into()],
        ] {
            assert!(super::run(&args).is_err());
        }
    }
}
