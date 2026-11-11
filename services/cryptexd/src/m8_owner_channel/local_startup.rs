//! Default-off real-binary startup/content experiment. Startup alone never
//! grants owner authority. Only the separate content-bound synthetic mode
//! exposes the private per-message connection used by the fixture domain.
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::Arc;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use super::image_policy::{PublisherDefinition, Role, SigningProfile, inventory_image};
use super::native::{Channel, Listener};
use super::release_inventory::{ReleaseTrust, VerifiedBundle, VerifiedInventory, read_input};

const CERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/m8-local-signer.der"));
const DESKTOP: &str = "Contents/MacOS/cryptex-desktop";
const DAEMON: &str = "Contents/MacOS/cryptexd";
const EPOCH_FLAG: &str = "--m8-local-startup-epoch";
const CONTENT_EPOCH_FLAG: &str = "--m8-local-content-epoch";
const RELEASE: &str = "m8-local-startup-v1";
#[path = "local_client.rs"]
mod client;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Arguments,
    Inventory,
    Image,
    Channel,
    Rendezvous,
    Instance,
    Deadline,
    Spawn,
    Readiness,
    Cleanup,
    StaleAccepted,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "local_signed_startup_{self:?}")
    }
}
impl std::error::Error for Error {}
impl From<super::native::Error> for Error {
    fn from(error: super::native::Error) -> Self {
        match error {
            super::native::Error::Image => Self::Image,
            super::native::Error::Instance => Self::Instance,
            super::native::Error::Rendezvous => Self::Rendezvous,
            super::native::Error::Deadline => Self::Deadline,
            _ => Self::Channel,
        }
    }
}

struct Release {
    root: PathBuf,
    bundle: VerifiedBundle,
    digest: [u8; 32],
}
impl Release {
    fn current(role: Role, deadline: Instant) -> Result<Self, Error> {
        let executable = std::env::current_exe().map_err(|_| Error::Image)?;
        let root = executable
            .ancestors()
            .nth(3)
            .ok_or(Error::Image)?
            .to_path_buf();
        let relative = match role {
            Role::Desktop => DESKTOP,
            Role::Daemon => DAEMON,
        };
        if executable != root.join(relative) {
            return Err(Error::Image);
        }
        let raw = read_input(&root, "Contents/Resources/m8-startup.json", 1024 * 1024)
            .map_err(|_| Error::Inventory)?;
        let cms = read_input(&root, "Contents/Resources/m8-startup.cms", 2 * 1024 * 1024)
            .map_err(|_| Error::Inventory)?;
        let architecture = match std::env::consts::ARCH {
            "aarch64" => "arm64",
            "x86_64" => "x86_64",
            _ => return Err(Error::Image),
        };
        let trust = ReleaseTrust {
            signer_der: CERT,
            release_id: RELEASE,
            architecture,
            publisher: PublisherDefinition {
                signing: SigningProfile::LocalCertificate { der: CERT },
                desktop_identifier: "com.cryptex.m8.local-startup.desktop",
                daemon_identifier: "com.cryptex.m8.local-startup.daemon",
            },
        };
        let bundle = VerifiedInventory::verify(&raw, &cms, trust, deadline)
            .and_then(|inventory| inventory.snapshot(&root, deadline))
            .map_err(|_| Error::Inventory)?;
        if bundle.image_paths() != (DESKTOP, DAEMON) {
            return Err(Error::Image);
        }
        Ok(Self {
            root,
            bundle,
            digest: Sha256::digest(&raw).into(),
        })
    }
}

fn check_time(deadline: Instant) -> Result<(), Error> {
    if Instant::now() >= deadline {
        Err(Error::Deadline)
    } else {
        Ok(())
    }
}
fn step<T>(name: &'static str, action: impl FnOnce() -> Result<T, Error>) -> Result<T, Error> {
    let started = Instant::now();
    let result = action();
    if let Err(error) = &result {
        // Fixed stage and typed error only: no runtime path, content, signer
        // selector, capability or owner snapshot is included.
        eprintln!(
            "{}",
            serde_json::json!({"m8LocalStage": name,
            "elapsedMs": started.elapsed().as_millis(), "error": error.to_string()})
        );
    }
    result
}

// Exact binary shape: magic, version, role-bound phase, epoch, inventory hash.
// Phase 1 = Desktop accept, 2 = daemon hello, 3 = daemon runtime-ready.
// These bytes have no operation/grant fields and cannot enter owner dispatch.
fn frame(role: u8, epoch: &[u8; 16], digest: &[u8; 32]) -> Vec<u8> {
    let mut out = b"CXLS\x01".to_vec();
    out.push(role);
    out.extend_from_slice(epoch);
    out.extend_from_slice(digest);
    out
}
fn epoch_text(epoch: &[u8; 16]) -> String {
    epoch.iter().map(|v| format!("{v:02x}")).collect()
}
fn parse_epoch(text: &str) -> Result<[u8; 16], Error> {
    if text.len() != 32
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Error::Arguments);
    }
    let mut bytes = [0; 16];
    for (i, value) in bytes.iter_mut().enumerate() {
        *value = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).map_err(|_| Error::Arguments)?;
    }
    Ok(bytes)
}

/// Only the feature-enabled Desktop's headless check constructs this.
pub struct Prepared {
    release: Release,
    listener: Listener,
    epoch: [u8; 16],
    deadline: Instant,
    content_mode: bool,
}
impl Prepared {
    pub fn current_desktop(timeout: Duration) -> Result<Self, Error> {
        let deadline = Instant::now().checked_add(timeout).ok_or(Error::Deadline)?;
        let release = step("desktop_inventory", || {
            Release::current(Role::Desktop, deadline)
        })?;
        check_time(deadline)?;
        let (listener, epoch) = Listener::open().map_err(|_| Error::Channel)?;
        Ok(Self {
            release,
            listener,
            epoch,
            deadline,
            content_mode: false,
        })
    }
    pub fn current_content(timeout: Duration) -> Result<Self, Error> {
        let mut prepared = Self::current_desktop(timeout)?;
        prepared
            .release
            .bundle
            .frontend(&prepared.release.digest)
            .map_err(|_| Error::Inventory)?;
        prepared.content_mode = true;
        Ok(prepared)
    }
    pub fn daemon_path(&self) -> PathBuf {
        self.release.root.join(DAEMON)
    }

    /// The path is a launch locator. Only the subsequent native live image and
    /// exact-instance checks establish this isolated startup observation.
    pub fn launch(self, mut command: Command, ready: &Path) -> Result<LocalDaemon, Error> {
        check_time(self.deadline)?;
        if Path::new(command.get_program()) != self.daemon_path() {
            return Err(Error::Arguments);
        }
        command.args([
            if self.content_mode {
                CONTENT_EPOCH_FLAG
            } else {
                EPOCH_FLAG
            },
            &epoch_text(&self.epoch),
        ]);
        let child = command.spawn().map_err(|_| Error::Spawn)?;
        let mut owned = LocalDaemon {
            child: Some(child),
            channel: None,
            release: self.release,
            epoch: self.epoch,
            content_mode: self.content_mode,
            bound: false,
            pending_review: None,
            client_cleanup_failed: false,
        };
        let expected = step("daemon_image", || {
            inventory_image(&owned.release.bundle, Role::Daemon).map_err(|_| Error::Image)
        })?;
        let mut channel = step("native_accept", || {
            self.listener
                .accept_until(
                    owned.child.as_mut().unwrap(),
                    &(&expected).into(),
                    self.deadline,
                )
                .map_err(Error::from)
        })?;
        check_time(self.deadline)?;
        let mut buffer = [0; 54];
        let hello = step("startup_hello", || {
            channel
                .receive(&mut buffer, self.deadline)
                .map_err(Error::from)
                .map(|bytes| bytes.to_vec())
        })?;
        if hello != frame(2, &self.epoch, &owned.release.digest) {
            return Err(Error::Channel);
        }
        step("startup_ack", || {
            channel
                .send(&frame(1, &self.epoch, &owned.release.digest), self.deadline)
                .map_err(Error::from)
        })?;
        owned.channel = Some(channel);
        step("ready_file", || {
            crate::daemon_startup::await_ready_until(
                owned.child.as_mut().unwrap(),
                ready,
                self.deadline,
            )
            .map_err(|_| Error::Readiness)
        })?;
        if step("runtime_ready", || {
            owned
                .channel
                .as_mut()
                .unwrap()
                .receive(&mut buffer, self.deadline)
                .map_err(Error::from)
                .map(|bytes| bytes.to_vec())
        })? != frame(3, &self.epoch, &owned.release.digest)
        {
            return Err(Error::Channel);
        }
        check_time(self.deadline)?;
        Ok(owned)
    }
}

/// One owner retains the snapshot, native rights and exact child. No native
/// handle, privileged grant or signer capability is returned to the renderer.
pub struct LocalDaemon {
    child: Option<Child>,
    channel: Option<Channel>,
    release: Release,
    epoch: [u8; 16],
    content_mode: bool,
    bound: bool,
    pending_review: Option<client::PendingReview>,
    client_cleanup_failed: bool,
}
impl LocalDaemon {
    pub fn terminate_for_restart(&mut self) -> Result<(), Error> {
        if let Some(mut child) = self.child.take() {
            let kill = child.kill();
            let status = child.wait().map_err(|_| Error::Cleanup)?;
            if kill.is_err() && status.success() {
                return Err(Error::Cleanup);
            }
        }
        Ok(())
    }
    pub fn require_stale_rejection(&mut self) -> Result<(), Error> {
        if self.child.is_some() {
            return Err(Error::Arguments);
        }
        let channel = self.channel.as_mut().ok_or(Error::Arguments)?;
        match channel.send(
            b"closed-startup-check",
            Instant::now() + Duration::from_secs(1),
        ) {
            Err(super::native::Error::Instance | super::native::Error::Closed) => Ok(()),
            _ => Err(Error::StaleAccepted),
        }
    }
    pub fn shutdown(mut self, stop: &Path) -> Result<(), Error> {
        if self.content_mode && (!self.bound || self.child.is_none()) {
            return self.close();
        }
        std::fs::write(stop, b"stop\n").map_err(|_| Error::Cleanup)?;
        if self.bound {
            self.channel
                .as_mut()
                .ok_or(Error::Channel)?
                .send(CLOSE, Instant::now() + Duration::from_secs(2))
                .map_err(Error::from)?;
            self.pending_review.take();
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match self.child.as_mut().ok_or(Error::Arguments)?.try_wait() {
                Ok(Some(status)) => {
                    self.child.take();
                    if !status.success() {
                        return Err(Error::Cleanup);
                    }
                    return self.close_channel();
                }
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                _ => return Err(Error::Cleanup),
            }
        }
    }
    pub fn close(mut self) -> Result<(), Error> {
        self.terminate_for_restart()?;
        self.close_channel()
    }
    fn close_channel(&mut self) -> Result<(), Error> {
        if let Some(channel) = self.channel.take()
            && channel.close().is_err()
        {
            self.client_cleanup_failed = true;
        }
        if self.client_cleanup_failed {
            Err(Error::Cleanup)
        } else {
            Ok(())
        }
    }
}
impl Drop for LocalDaemon {
    fn drop(&mut self) {
        let child = self.terminate_for_restart();
        let channel = self.close_channel();
        if child.is_err() || channel.is_err() {
            eprintln!("local_signed_startup_cleanup_failed");
        }
    }
}

/// Held by the actual daemon main across its lifecycle. Startup data alone
/// never creates an authenticated owner request or unlocks Unix owner routes.
pub struct ChildGuard {
    _channel: Channel,
    _release: Release,
    epoch: [u8; 16],
    deadline: Instant,
    content_mode: bool,
    runtime: PathBuf,
}
impl ChildGuard {
    pub fn is_content_mode(&self) -> bool {
        self.content_mode
    }
    pub fn serve_content(
        &mut self,
        root: &Path,
        lock: &crate::instance::InstanceLock,
    ) -> Result<(), Error> {
        if !self.content_mode || root != self.runtime {
            return Err(Error::Arguments);
        }
        let content = self
            ._release
            .bundle
            .frontend(&self._release.digest)
            .map_err(|_| Error::Inventory)?;
        let mut bytes = [0; 54];
        if self
            ._channel
            .receive(&mut bytes, self.deadline)
            .map_err(Error::from)?
            != frame(4, &self.epoch, content.binding())
        {
            return Err(Error::Channel);
        }
        let mut connection = BoundConnection {
            channel: &mut self._channel,
            seal: Arc::new(()),
            ready: frame(5, &self.epoch, content.binding()),
            _content: content,
        };
        crate::ipc::local_native::serve(&mut connection, root, lock).map_err(|_| Error::Channel)
    }
    pub fn runtime_ready(&mut self) -> Result<(), Error> {
        self._channel
            .send(&frame(3, &self.epoch, &self._release.digest), self.deadline)
            .map_err(Error::from)
    }
    pub fn close(self) -> Result<(), Error> {
        self._channel.close().map_err(|_| Error::Cleanup)
    }
}
pub fn child_gate(args: &mut Vec<String>) -> Result<Option<ChildGuard>, Error> {
    if !args
        .iter()
        .any(|arg| arg == EPOCH_FLAG || arg == CONTENT_EPOCH_FLAG)
    {
        return Ok(None);
    }
    if args.len() != 10
        || args[0] != "lifecycle"
        || args[1] != "hold"
        || args[2] != "--runtime-root"
        || args[4] != "--ready-file"
        || args[6] != "--stop-file"
        || !matches!(args[8].as_str(), EPOCH_FLAG | CONTENT_EPOCH_FLAG)
        || !Path::new(&args[3]).is_absolute()
        || Path::new(&args[5]) != Path::new(&args[3]).join("ready")
        || Path::new(&args[7]) != Path::new(&args[3]).join("stop")
    {
        return Err(Error::Arguments);
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    let epoch = parse_epoch(&args[9])?;
    let content_mode = args[8] == CONTENT_EPOCH_FLAG;
    let runtime = PathBuf::from(&args[3]);
    if content_mode {
        require_fresh_content_runtime(&runtime)?;
    }
    let release = step("child_inventory", || {
        Release::current(Role::Daemon, deadline)
    })?;
    let expected = inventory_image(&release.bundle, Role::Desktop).map_err(|_| Error::Image)?;
    check_time(deadline)?;
    let mut channel =
        Channel::connect_until(&epoch, &(&expected).into(), deadline).map_err(Error::from)?;
    channel
        .send(&frame(2, &epoch, &release.digest), deadline)
        .map_err(Error::from)?;
    let mut buffer = [0; 54];
    if channel
        .receive(&mut buffer, deadline)
        .map_err(Error::from)?
        != frame(1, &epoch, &release.digest)
    {
        return Err(Error::Channel);
    }
    check_time(deadline)?;
    args.truncate(8);
    Ok(Some(ChildGuard {
        _channel: channel,
        _release: release,
        epoch,
        deadline,
        content_mode,
        runtime,
    }))
}

pub const CONTENT_MARKER: &str = "m8-isolated-native-content-v1\n";
fn require_fresh_content_runtime(root: &Path) -> Result<(), Error> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::symlink_metadata(root).map_err(|_| Error::Arguments)?;
    let temp = std::env::temp_dir()
        .canonicalize()
        .map_err(|_| Error::Arguments)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != unsafe { libc::getuid() }
        || metadata.mode() & 0o077 != 0
        || !root
            .canonicalize()
            .map_err(|_| Error::Arguments)?
            .starts_with(temp)
        || std::fs::read_dir(root)
            .map_err(|_| Error::Arguments)?
            .count()
            != 1
        || std::fs::read(root.join("m8-content.marker")).map_err(|_| Error::Arguments)?
            != CONTENT_MARKER.as_bytes()
    {
        return Err(Error::Arguments);
    }
    Ok(())
}

const CLOSE: &[u8] = b"CXLC\x01";
const PING: &[u8] = b"CXLP\x01";
const STATUS: &[u8] = b"CXLS-status\x01";
pub(crate) struct BoundConnection<'a> {
    channel: &'a mut Channel,
    seal: Arc<()>,
    ready: Vec<u8>,
    _content: crate::ProtectedContent,
}
pub(crate) struct ReceivedRequest {
    decoded: super::request::DecodedRequest,
    seal: Arc<()>,
}
pub(crate) enum Incoming {
    Request(Box<ReceivedRequest>),
    Close,
    Ping,
    Status,
}
impl BoundConnection<'_> {
    pub(crate) fn ready(&mut self, deadline: Instant) -> std::io::Result<()> {
        self.channel.send(&self.ready, deadline).map_err(native_io)
    }
    pub(crate) fn receive(&mut self, deadline: Instant) -> std::io::Result<Incoming> {
        let mut bytes = vec![0; super::request::MAX_FRAME_BYTES];
        let bytes = self
            .channel
            .receive(&mut bytes, deadline)
            .map_err(native_io)?;
        if bytes == CLOSE {
            return Ok(Incoming::Close);
        }
        if bytes == PING {
            return Ok(Incoming::Ping);
        }
        if bytes == STATUS {
            return Ok(Incoming::Status);
        }
        let decoded = super::request::decode_untrusted(bytes).map_err(std::io::Error::other)?;
        Ok(Incoming::Request(Box::new(ReceivedRequest {
            decoded,
            seal: Arc::clone(&self.seal),
        })))
    }
    pub(crate) fn consume(
        &self,
        request: ReceivedRequest,
    ) -> std::io::Result<super::request::DecodedRequest> {
        if !Arc::ptr_eq(&self.seal, &request.seal) {
            return Err(std::io::Error::other("native_request_connection_mismatch"));
        }
        Ok(request.decoded)
    }
    pub(crate) fn send(&mut self, bytes: &[u8], deadline: Instant) -> std::io::Result<()> {
        self.channel.send(bytes, deadline).map_err(native_io)
    }
}
fn native_io(error: super::native::Error) -> std::io::Error {
    std::io::Error::other(Error::from(error))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_frames_bind_fixed_role_epoch_and_inventory() {
        let a = frame(1, &[1; 16], &[2; 32]);
        assert_eq!(a.len(), 54);
        assert_ne!(a, frame(2, &[1; 16], &[2; 32]));
        assert_ne!(a, frame(3, &[1; 16], &[2; 32]));
        assert_ne!(a, frame(1, &[3; 16], &[2; 32]));
        assert_ne!(a, frame(1, &[1; 16], &[3; 32]));
        assert_eq!(parse_epoch(&epoch_text(&[123; 16])), Ok([123; 16]));
        assert_eq!(parse_epoch(&"A".repeat(32)), Err(Error::Arguments));
        assert_eq!(parse_epoch(&"0".repeat(31)), Err(Error::Arguments));
    }
    #[test]
    fn malformed_child_mode_refuses_before_native_or_runtime_work() {
        for mut args in [
            vec![EPOCH_FLAG.into()],
            vec!["lifecycle".into(), EPOCH_FLAG.into(), "bad".into()],
        ] {
            assert!(matches!(child_gate(&mut args), Err(Error::Arguments)));
        }
        assert!(child_gate(&mut vec!["help".into()]).unwrap().is_none());
    }
}
