//! Run the package's encrypt-only helper with a public certificate and a
//! daemon-prepared archive. The caller supplies independent release digests.
use super::{Error, Prepared};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::NamedTempFile;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CodecPins {
    runtime: String,
    entry: String,
    openpgp: String,
}

impl CodecPins {
    fn packaged() -> Result<Self, Error> {
        let manifest = option_env!("CRYPTEX_M11_PACKAGE_CODEC_DIGESTS").ok_or(Error::Codec)?;
        serde_json::from_str(manifest).map_err(|_| Error::Codec)
    }
}

// Verify the very bytes that will run. Package files can be owned by the OS.
// A name change in the source directory cannot substitute code after this copy.
fn copy_pinned(source: &Path, destination: &Path, expected: &str) -> Result<(), Error> {
    let mut file = File::open(source).map_err(|_| Error::Codec)?;
    let mut out = File::create_new(destination).map_err(|_| Error::Codec)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut bytes = 0u64;
    loop {
        let n = file.read(&mut buffer).map_err(|_| Error::Codec)?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        if bytes > 256 * 1024 * 1024 {
            return Err(Error::Codec);
        }
        out.write_all(&buffer[..n]).map_err(|_| Error::Codec)?;
        digest.update(&buffer[..n]);
    }
    if format!("{:x}", digest.finalize()) != expected {
        return Err(Error::CodecIntegrity);
    }
    Ok(())
}

impl Prepared {
    /// The directory must be the OS-protected resource directory of the
    /// owner's selected installation. No renderer-supplied command is accepted.
    pub fn encrypt(&self, directory: &Path, recipient: &[u8]) -> Result<NamedTempFile, Error> {
        let assembly = self.encryption_input()?;
        run_codec(
            directory,
            recipient,
            Some(assembly.reopen().map_err(|_| Error::Archive)?),
        )
    }
}

/// Parse and reserialize a single encryption-capable public certificate before
/// its bytes may be retained by the daemon or displayed to the owner.
pub(crate) fn public_recipient(directory: &Path, recipient: &[u8]) -> Result<Vec<u8>, Error> {
    let file = run_codec(directory, recipient, None)?;
    let mut bytes = Vec::new();
    file.reopen()
        .map_err(|_| Error::Codec)?
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Codec)?;
    if bytes.is_empty() || bytes.len() > 65536 {
        return Err(Error::Codec);
    }
    Ok(bytes)
}

fn run_codec(
    directory: &Path,
    recipient: &[u8],
    mut payload: Option<File>,
) -> Result<NamedTempFile, Error> {
    let pins = CodecPins::packaged()?;
    if !directory.is_absolute() || recipient.is_empty() || recipient.len() > 64 * 1024 {
        return Err(Error::Codec);
    }
    let mut staging = tempfile::Builder::new();
    staging.prefix("cryptex-backup-codec-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        staging.permissions(std::fs::Permissions::from_mode(0o700));
    }
    let staging = staging.tempdir().map_err(|_| Error::Codec)?;
    // Node resolves module paths before checking its filesystem permission.
    // macOS exposes the private temp folder via both /var and /private/var.
    #[cfg(target_os = "macos")]
    let path = staging.path().canonicalize().map_err(|_| Error::Codec)?;
    #[cfg(not(target_os = "macos"))]
    let path = staging.path();
    let runtime = if cfg!(windows) { "node.exe" } else { "node" };
    for (name, pin) in [
        (runtime, &pins.runtime),
        ("encrypt.cjs", &pins.entry),
        ("openpgp.min.cjs", &pins.openpgp),
    ] {
        copy_pinned(&directory.join(name), &path.join(name), pin)?;
    }
    let node = path.join(runtime);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&node, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| Error::Codec)?;
    }
    let script = path.join("encrypt.cjs");
    let library = path.join("openpgp.min.cjs");
    let output = NamedTempFile::new().map_err(|_| Error::Archive)?;
    let mut read_library = std::ffi::OsString::from("--allow-fs-read=");
    read_library.push(&library);
    let mut command = Command::new(node);
    command
        .env_clear()
        .current_dir(path)
        .args(["--permission", "--no-addons"])
        .arg(read_library)
        .arg(script)
        .stdin(Stdio::piped())
        .stderr(Stdio::null())
        .stdout(output.as_file().try_clone().map_err(|_| Error::Archive)?);
    if payload.is_none() {
        command.arg("public-recipient");
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
        command.creation_flags(CREATE_NO_WINDOW);
        let system = std::env::var_os("SystemRoot").ok_or(Error::Codec)?;
        command.env("SystemRoot", system);
    }
    let mut child = command.spawn().map_err(|_| Error::CodecStart)?;
    let mut pipe = child.stdin.take().expect("piped child input");
    let status = std::thread::scope(|scope| {
        let feeder = scope.spawn(move || -> io::Result<()> {
            pipe.write_all(&(recipient.len() as u32).to_be_bytes())?;
            pipe.write_all(recipient)?;
            if let Some(input) = payload.as_mut() {
                io::copy(input, &mut pipe)?;
            }
            Ok(())
        });
        let deadline = Instant::now() + Duration::from_secs(300);
        let success = loop {
            match child.try_wait() {
                Ok(Some(code)) => break code.success(),
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(50))
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break false;
                }
            }
        };
        success && matches!(feeder.join(), Ok(Ok(())))
    });
    if !status
        || output
            .as_file()
            .metadata()
            .map_err(|_| Error::Archive)?
            .len()
            == 0
    {
        return Err(Error::CodecExit);
    }
    output.as_file().sync_all().map_err(|_| Error::Archive)?;
    Ok(output)
}
