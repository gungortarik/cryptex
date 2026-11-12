//! Default-off transport-only role of the same signed daemon. No canonical API.
use super::image_policy::{Role, inventory_image};
use super::local_startup::Release;
use super::native::{Channel, Listener, PeerLiveness};
use crate::managed_profile::{ManagedProfile, native::NativePorts};
use crate::secure_store::{MacosKeychainStore, SecureStore, SecureStoreEnvironment};
use crate::syncthing_package::PinnedSyncthingPackage;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::time::{Duration, Instant};

const FLAG: &str = "--m8-local-transport-helper";
const MARKER: &[u8] = b"m8-native-transport-helper-v1\n";
const HINT: &[u8] = b"cryptex-m8-transport-verified-v1\n";
const PROFILE: &str = "00000000000000000000000000000001";
const BUDGET: Duration = Duration::from_secs(4);
const REFUSAL_ACK: &[u8] = b"m8-transport-refusal-ack-v1";
const STOP_ACK: &[u8] = b"m8-transport-stop-ack-v1";
fn channel_error(stage: &'static str, error: super::native::Error) -> Error {
    eprintln!(
        "component=mac_transport operation=native_channel stage={stage} status=failed error={error:?}"
    );
    Error::Channel
}
thread_local! {static PARENT:RefCell<Option<Rc<PeerLiveness>>>=const{RefCell::new(None)};}
pub(crate) fn role_entered() -> bool {
    PARENT.with(|p| p.borrow().is_some())
}
pub(crate) fn role_active() -> bool {
    PARENT.with(|p| p.borrow().as_ref().is_some_and(|pin| pin.live()))
}
struct Scope;
impl Scope {
    fn enter(channel: &Channel) -> Result<Self, Error> {
        if role_entered()
            || unsafe { libc::getpid() != libc::getpgrp() || libc::getpid() != libc::getsid(0) }
        {
            return Err(Error::Policy);
        }
        let pin = Rc::new(channel.peer_liveness().map_err(|_| Error::Channel)?);
        PARENT.with(|p| *p.borrow_mut() = Some(pin));
        Ok(Self)
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        let _ = PARENT.with(|p| p.borrow_mut().take());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    Arguments,
    Policy,
    Profile,
    Package,
    Engine,
    Channel,
    Deadline,
    Cleanup,
    Unavailable,
    CredentialUnavailable,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "mac_transport_{self:?}")
    }
}
impl std::error::Error for Error {}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    sequence: u64,
    operation: Operation,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Operation {
    Observe,
    Stop,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    sequence: u64,
    identity: Option<String>,
    credential_profile: Option<String>,
    error: Option<Error>,
}

fn private_root(root: &Path) -> Result<(), Error> {
    crate::paths::assert_outside_repository(root).map_err(|_| Error::Arguments)?;
    let temp = std::env::temp_dir()
        .canonicalize()
        .map_err(|_| Error::Arguments)?;
    if !root.is_absolute()
        || root.canonicalize().map_err(|_| Error::Arguments)? != root
        || !root.starts_with(temp)
    {
        return Err(Error::Arguments);
    }
    let m = std::fs::symlink_metadata(root).map_err(|_| Error::Profile)?;
    if !m.is_dir() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0 {
        return Err(Error::Profile);
    }
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(root)
        .map_err(|_| Error::Profile)?;
    if !crate::owned_process::macos::private_acl(&directory) {
        return Err(Error::Profile);
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(root.join("transport.marker"))
        .map_err(|_| Error::Profile)?;
    let m = file.metadata().map_err(|_| Error::Profile)?;
    if !crate::owned_process::macos::private_acl(&file) {
        return Err(Error::Profile);
    }
    let mut bytes = Vec::new();
    file.take(128)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Profile)?;
    if !m.is_file()
        || m.nlink() != 1
        || m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o077 != 0
        || bytes != MARKER
    {
        return Err(Error::Profile);
    }
    Ok(())
}
fn create_root(root: &Path) -> Result<PathBuf, Error> {
    let parent = root
        .parent()
        .ok_or(Error::Arguments)?
        .canonicalize()
        .map_err(|_| Error::Arguments)?;
    let temp = std::env::temp_dir()
        .canonicalize()
        .map_err(|_| Error::Arguments)?;
    if !root.is_absolute()
        || !parent.starts_with(temp)
        || crate::paths::assert_outside_repository(&parent).is_err()
    {
        return Err(Error::Arguments);
    }
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(root)
        .map_err(|_| Error::Profile)?;
    let root = root.canonicalize().map_err(|_| Error::Profile)?;
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(root.join("transport.marker"))
        .map_err(|_| Error::Profile)?;
    f.write_all(MARKER)
        .and_then(|_| f.sync_all())
        .map_err(|_| Error::Profile)?;
    private_root(&root)?;
    Ok(root)
}
fn epoch_text(epoch: &[u8; 16]) -> String {
    epoch.iter().map(|b| format!("{b:02x}")).collect()
}
fn epoch(text: &str) -> Result<[u8; 16], Error> {
    if text.len() != 32
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Error::Arguments);
    }
    let mut result = [0; 16];
    for (i, slot) in result.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).map_err(|_| Error::Arguments)?;
    }
    Ok(result)
}
fn hello(role: u8, epoch: &[u8; 16], release: &Release, root: &Path) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    use std::os::unix::ffi::OsStrExt;
    let mut bytes = b"m8-transport-only-v1\0".to_vec();
    bytes.push(role);
    bytes.extend_from_slice(epoch);
    bytes.extend_from_slice(&release.digest);
    bytes.extend_from_slice(&Sha256::digest(root.as_os_str().as_bytes()));
    bytes
}
fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    if bytes.len() > 1024 {
        return Err(Error::Channel);
    }
    cryptex_events::reject_duplicate_json_keys(
        std::str::from_utf8(bytes).map_err(|_| Error::Channel)?,
    )
    .map_err(|_| Error::Channel)?;
    serde_json::from_slice(bytes).map_err(|_| Error::Channel)
}
fn await_hint(child: &mut Child, deadline: Instant) -> Result<(), Error> {
    use std::os::fd::AsRawFd;
    let pipe = child.stdout.as_mut().ok_or(Error::Channel)?;
    let fd = pipe.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(Error::Channel);
    }
    let mut raw = Vec::new();
    let mut buffer = [0u8; 64];
    loop {
        if Instant::now() >= deadline {
            return Err(Error::Deadline);
        }
        match pipe.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                raw.extend_from_slice(&buffer[..n]);
                if raw.len() > HINT.len() {
                    return Err(Error::Channel);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5))
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Err(Error::Channel),
        }
    }
    if raw != HINT {
        return Err(Error::Channel);
    }
    Ok(())
}
fn package(release: &Release, root: &Path) -> Result<PinnedSyncthingPackage, Error> {
    let directory = root.join("package");
    if !directory.try_exists().map_err(|_| Error::Package)? {
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .map_err(|_| Error::Package)?;
        for name in ["syncthing", "LICENSE.txt", "AUTHORS.txt"] {
            let bytes = release
                .bundle
                .content(&format!("Contents/Resources/transport/{name}"))
                .ok_or(Error::Package)?;
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(if name == "syncthing" { 0o500 } else { 0o400 })
                .open(directory.join(name))
                .map_err(|_| Error::Package)?;
            file.write_all(bytes)
                .and_then(|_| file.sync_all())
                .map_err(|_| Error::Package)?;
        }
    }
    PinnedSyncthingPackage::open(&directory).map_err(|_| Error::Package)
}
fn ports(root: &Path) -> Result<NativePorts, Error> {
    let file = root.join("ports.json");
    if file.try_exists().map_err(|_| Error::Profile)? {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(file)
            .map_err(|_| Error::Profile)?;
        let m = file.metadata().map_err(|_| Error::Profile)?;
        if !m.is_file()
            || m.nlink() != 1
            || m.uid() != unsafe { libc::geteuid() }
            || m.mode() & 0o077 != 0
            || !crate::owned_process::macos::private_acl(&file)
        {
            return Err(Error::Profile);
        }
        let mut raw = Vec::new();
        file.take(1025)
            .read_to_end(&mut raw)
            .map_err(|_| Error::Profile)?;
        return decode(&raw);
    }
    let a = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .map_err(|_| Error::Engine)?;
    let b = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .map_err(|_| Error::Engine)?;
    let ports = NativePorts::new(
        a.local_addr().map_err(|_| Error::Engine)?.port(),
        b.local_addr().map_err(|_| Error::Engine)?.port(),
    )
    .map_err(|_| Error::Engine)?;
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(file)
        .map_err(|_| Error::Profile)?;
    f.write_all(&serde_json::to_vec(&ports).map_err(|_| Error::Profile)?)
        .and_then(|_| f.sync_all())
        .map_err(|_| Error::Profile)?;
    File::open(root)
        .and_then(|f| f.sync_all())
        .map_err(|_| Error::Profile)?;
    Ok(ports)
}

fn serve(root: &Path, release: &Release, channel: &mut Channel) -> Result<(), Error> {
    let _scope = Scope::enter(channel)?;
    let _package = package(release, root)?;
    let ports = ports(root)?;
    let path = root.join(PROFILE);
    let store = MacosKeychainStore::new();
    let mut profile = if path.try_exists().map_err(|_| Error::Profile)? {
        ManagedProfile::open(&path, SecureStoreEnvironment::Dev)
    } else {
        ManagedProfile::create(&path, SecureStoreEnvironment::Dev)
    }
    .map_err(|_| Error::Profile)?;
    let credential = profile.credential_profile_id().to_owned();
    let mut prepared = profile
        .prepare_native(&_package, &store, ports)
        .inspect_err(|e| eprintln!("component=mac_transport stage=prepare error={e}"))
        .map_err(|error| match error {
            crate::managed_profile::native::NativeConfigError::Profile(
                crate::managed_profile::ProfileError::CredentialUnavailable,
            ) => Error::CredentialUnavailable,
            _ => Error::Profile,
        })?;
    let identity = prepared.configured_transport_device_id().to_owned();
    let mut session = prepared
        .start(&_package, &store)
        .inspect_err(|e| eprintln!("component=mac_transport stage=start error={e}"))
        .map_err(|_| Error::Engine)?;
    eprintln!("component=mac_transport stage=owned_rest_ready status=passed");
    session
        .exercise_foreign_socket_refusal(&store)
        .inspect_err(|error| {
            eprintln!(
                "component=mac_transport stage=foreign_socket_check status=failed error={error}"
            )
        })
        .map_err(|_| Error::Engine)?;
    let ready = Reply {
        sequence: 0,
        identity: Some(identity.clone()),
        credential_profile: Some(credential),
        error: None,
    };
    channel
        .send(
            &serde_json::to_vec(&ready).map_err(|_| Error::Channel)?,
            Instant::now() + BUDGET,
        )
        .map_err(|_| Error::Channel)?;
    let mut sequence = 0;
    loop {
        let mut buffer = [0; 1024];
        let bytes = channel
            .receive(&mut buffer, Instant::now() + Duration::from_secs(60))
            .map_err(|_| Error::Channel)?;
        let request: Request = decode(bytes)?;
        sequence += 1;
        if request.sequence != sequence {
            return Err(Error::Channel);
        }
        match request.operation {
            Operation::Observe => {
                let _before_package = PinnedSyncthingPackage::open(&root.join("package"))
                    .map_err(|_| Error::Package)?;
                session
                    .observe(&store)
                    .inspect_err(|e| eprintln!("component=mac_transport stage=observe error={e}"))
                    .map_err(|_| Error::Engine)?;
                let _after_package = PinnedSyncthingPackage::open(&root.join("package"))
                    .map_err(|_| Error::Package)?;
                if !role_active() {
                    crate::owned_process::macos::kill_helper_group();
                }
                let reply = Reply {
                    sequence,
                    identity: Some(identity.clone()),
                    credential_profile: None,
                    error: None,
                };
                channel
                    .send(
                        &serde_json::to_vec(&reply).map_err(|_| Error::Channel)?,
                        Instant::now() + BUDGET,
                    )
                    .map_err(|_| Error::Channel)?;
            }
            Operation::Stop => {
                session.stop(&store).map_err(|_| Error::Engine)?;
                let reply = Reply {
                    sequence,
                    identity: None,
                    credential_profile: None,
                    error: None,
                };
                channel
                    .send(
                        &serde_json::to_vec(&reply).map_err(|_| Error::Channel)?,
                        Instant::now() + BUDGET,
                    )
                    .map_err(|_| Error::Channel)?;
                // Keep the authenticated helper live until its supervisor has
                // actually consumed the stop result. A queued reply from an
                // exited sender must remain invalid under the native policy.
                let mut ack = [0; 128];
                if channel
                    .receive(&mut ack, Instant::now() + BUDGET)
                    .map_err(|error| channel_error("stop_ack", error))?
                    != STOP_ACK
                {
                    return Err(Error::Channel);
                }
                return Ok(());
            }
        }
    }
}

pub fn entry(args: &[String]) -> Option<Result<(), Error>> {
    if args.first().is_some_and(|a| {
        a == "--m8-local-transport-loss-driver" || a == "--m8-local-transport-blocked-check"
    }) {
        return Some(if args.len() != 2 {
            Err(Error::Arguments)
        } else if args[0] == "--m8-local-transport-loss-driver" {
            loss_driver(Path::new(&args[1]))
        } else {
            (|| {
                let worker = Worker::spawn(PathBuf::from(&args[1]), false)?;
                if wait_identity(worker.status_reader()).is_ok()
                    || worker.close() != Err(Error::Profile)
                {
                    return Err(Error::Policy);
                }
                println!("mac_transport_unresolved_restart_refused=passed");
                Ok(())
            })()
        });
    }
    if args
        .first()
        .is_some_and(|a| a == "--m8-local-transport-driver")
    {
        return Some(if args.len() == 2 {
            check(Path::new(&args[1]))
        } else {
            Err(Error::Arguments)
        });
    }
    if args.first().is_none_or(|a| a != FLAG) {
        return None;
    }
    Some((|| {
        if args.len() != 3 {
            return Err(Error::Arguments);
        }
        let epoch = epoch(&args[1])?;
        let root = Path::new(&args[2]);
        private_root(root)?;
        let deadline = Instant::now() + Duration::from_secs(15);
        let release = Release::current(Role::Daemon, deadline).map_err(|_| Error::Policy)?;
        let expected = inventory_image(&release.bundle, Role::Daemon).map_err(|_| Error::Policy)?;
        std::io::stdout()
            .write_all(HINT)
            .and_then(|_| std::io::stdout().flush())
            .map_err(|_| Error::Channel)?;
        use std::os::fd::AsRawFd;
        let discard = OpenOptions::new()
            .write(true)
            .open("/dev/null")
            .map_err(|_| Error::Channel)?;
        if unsafe { libc::dup2(discard.as_raw_fd(), libc::STDOUT_FILENO) } < 0 {
            return Err(Error::Channel);
        }
        let mut channel = Channel::connect_until(&epoch, &(&expected).into(), deadline)
            .map_err(|_| Error::Channel)?;
        channel
            .send(&hello(2, &epoch, &release, root), deadline)
            .map_err(|_| Error::Channel)?;
        let mut buffer = [0; 128];
        if channel
            .receive(&mut buffer, deadline)
            .map_err(|_| Error::Channel)?
            != hello(1, &epoch, &release, root)
        {
            return Err(Error::Channel);
        }
        let result = serve(root, &release, &mut channel);
        if let Err(error) = result {
            let refusal = Reply {
                sequence: 0,
                identity: None,
                credential_profile: None,
                error: Some(error),
            };
            let bytes = serde_json::to_vec(&refusal).map_err(|_| Error::Channel)?;
            channel
                .send(&bytes, Instant::now() + BUDGET)
                .map_err(|_| Error::Channel)?;
            let mut ack = [0; 128];
            if channel
                .receive(&mut ack, Instant::now() + BUDGET)
                .map_err(|_| Error::Channel)?
                != REFUSAL_ACK
            {
                return Err(Error::Channel);
            }
        }
        let closed = channel.close().map_err(|_| Error::Cleanup);
        result.and(closed)
    })())
}

struct Helper {
    child: Option<Child>,
    channel: Option<Channel>,
    _release: Release,
    sequence: u64,
    identity: String,
    credential: String,
    root: PathBuf,
    active: File,
}
impl Helper {
    fn start(
        root: &Path,
        credential_record: &std::sync::Mutex<Option<String>>,
    ) -> Result<Self, Error> {
        use std::os::unix::process::CommandExt;
        private_root(root)?;
        if root
            .join("transport.active")
            .try_exists()
            .map_err(|_| Error::Profile)?
        {
            return Err(Error::Profile);
        }
        let profile_path = root.join(PROFILE);
        let profile = if profile_path.try_exists().map_err(|_| Error::Profile)? {
            ManagedProfile::open(&profile_path, SecureStoreEnvironment::Dev)
        } else {
            ManagedProfile::create(&profile_path, SecureStoreEnvironment::Dev)
        }
        .map_err(|_| Error::Profile)?;
        let credential = profile.credential_profile_id().to_owned();
        drop(profile);
        *credential_record.lock().map_err(|_| Error::Unavailable)? = Some(credential.clone());
        let deadline = Instant::now() + Duration::from_secs(15);
        let release = Release::current(Role::Daemon, deadline).map_err(|_| Error::Policy)?;
        let expected = inventory_image(&release.bundle, Role::Daemon).map_err(|_| Error::Policy)?;
        let (listener, epoch) = Listener::open().map_err(|_| Error::Channel)?;
        let mut active = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(root.join("transport.active"))
            .map_err(|_| Error::Profile)?;
        active
            .write_all(epoch_text(&epoch).as_bytes())
            .and_then(|_| active.sync_all())
            .map_err(|_| Error::Profile)?;
        File::open(root)
            .and_then(|f| f.sync_all())
            .map_err(|_| Error::Profile)?;
        let mut command = Command::new(release.root.join("Contents/MacOS/cryptexd"));
        command
            .args([
                FLAG,
                &epoch_text(&epoch),
                root.to_str().ok_or(Error::Arguments)?,
            ])
            .env_clear()
            // Root validation must use the same backend-selected temporary
            // directory in both roles; no inherited command/search variables.
            .env(
                "TMPDIR",
                std::env::temp_dir()
                    .canonicalize()
                    .map_err(|_| Error::Profile)?,
            )
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        // SAFETY: setsid is async-signal-safe; no allocations/locks run in child.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
        let child = command.spawn().map_err(|_| Error::Engine)?;
        let mut result = Self {
            child: Some(child),
            channel: None,
            _release: release,
            sequence: 0,
            identity: String::new(),
            credential: credential.clone(),
            root: root.to_owned(),
            active,
        };
        await_hint(result.child.as_mut().unwrap(), deadline)?;
        let mut channel = listener
            .accept_until(
                result.child.as_mut().unwrap(),
                &(&expected).into(),
                deadline,
            )
            .map_err(|error| channel_error("helper_accept", error))?;
        let mut buffer = [0; 128];
        if channel
            .receive(&mut buffer, deadline)
            .map_err(|_| Error::Channel)?
            != hello(2, &epoch, &result._release, root)
        {
            return Err(Error::Channel);
        }
        channel
            .send(&hello(1, &epoch, &result._release, root), deadline)
            .map_err(|_| Error::Channel)?;
        let mut buffer = [0; 1024];
        let ready: Reply = decode(
            channel
                .receive(&mut buffer, Instant::now() + Duration::from_secs(35))
                .map_err(|error| channel_error("helper_ready", error))?,
        )?;
        if let Some(error) = ready.error {
            if ready.sequence != 0 || ready.identity.is_some() || ready.credential_profile.is_some()
            {
                return Err(Error::Channel);
            }
            channel
                .send(REFUSAL_ACK, Instant::now() + BUDGET)
                .map_err(|_| Error::Channel)?;
            return Err(error);
        }
        let id = ready
            .identity
            .filter(|s| crate::syncthing_rest::native_id_shape(s))
            .ok_or(Error::Engine)?;
        let returned_credential = ready
            .credential_profile
            .filter(|s| {
                s.len() == 32
                    && s.bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
            .ok_or(Error::Profile)?;
        if ready.sequence != 0 || ready.error.is_some() {
            return Err(Error::Channel);
        }
        if returned_credential != credential {
            return Err(Error::Profile);
        }
        result.identity = id;
        result.channel = Some(channel);
        Ok(result)
    }
    fn request(&mut self, op: Operation) -> Result<(), Error> {
        self.sequence = self.sequence.checked_add(1).ok_or(Error::Channel)?;
        let request = Request {
            sequence: self.sequence,
            operation: op,
        };
        let deadline = Instant::now()
            + if matches!(op, Operation::Stop) {
                Duration::from_secs(30)
            } else {
                BUDGET
            };
        let channel = self.channel.as_mut().ok_or(Error::Channel)?;
        channel
            .send(
                &serde_json::to_vec(&request).map_err(|_| Error::Channel)?,
                deadline,
            )
            .map_err(|error| channel_error("request_send", error))?;
        let mut buffer = [0; 1024];
        let reply: Reply = decode(
            channel
                .receive(&mut buffer, deadline)
                .map_err(|error| channel_error("request_receive", error))?,
        )?;
        if reply.sequence != self.sequence
            || reply.error.is_some()
            || reply.credential_profile.is_some()
            || match op {
                Operation::Observe => reply.identity.as_deref() != Some(self.identity.as_str()),
                Operation::Stop => reply.identity.is_some(),
            }
        {
            return Err(Error::Channel);
        }
        Ok(())
    }
    fn close(&mut self) -> Result<(), Error> {
        self.request(Operation::Stop)?;
        self.channel
            .as_mut()
            .ok_or(Error::Channel)?
            .send(STOP_ACK, Instant::now() + BUDGET)
            .map_err(|error| channel_error("stop_ack_send", error))?;
        // Retain the parent's endpoint while the live helper consumes the ACK
        // and exits; closing it first would invalidate even an already queued
        // ACK. Reap only after verified group death, then release the channel.
        self.cleanup(false)?;
        if let Some(channel) = self.channel.take() {
            channel.close().map_err(|_| Error::Cleanup)?;
        }
        Ok(())
    }
    fn cleanup(&mut self, kill: bool) -> Result<(), Error> {
        let Some(child) = self.child.as_mut() else {
            return Ok(());
        };
        // Keep the exact group leader unreaped until every member is dead.
        let group = child.id() as i32;
        if kill {
            unsafe { libc::killpg(group, libc::SIGKILL) };
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
            let seen = unsafe {
                libc::waitid(
                    libc::P_PID,
                    child.id(),
                    info.as_mut_ptr(),
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            };
            if seen != 0 {
                return Err(Error::Cleanup);
            }
            if unsafe { info.assume_init().si_pid() } != 0
                && unsafe { cx_transport_group_empty(group, 0, group) } == 1
            {
                let status = child.wait().map_err(|_| Error::Cleanup)?;
                self.child.take();
                let retained = self.active.metadata().map_err(|_| Error::Cleanup)?;
                let current = std::fs::symlink_metadata(self.root.join("transport.active"))
                    .map_err(|_| Error::Cleanup)?;
                if retained.dev() != current.dev() || retained.ino() != current.ino() {
                    return Err(Error::Cleanup);
                }
                std::fs::remove_file(self.root.join("transport.active"))
                    .map_err(|_| Error::Cleanup)?;
                File::open(&self.root)
                    .and_then(|f| f.sync_all())
                    .map_err(|_| Error::Cleanup)?;
                return if kill || status.success() {
                    Ok(())
                } else {
                    Err(Error::Cleanup)
                };
            }
            if Instant::now() >= deadline {
                return Err(Error::Cleanup);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
unsafe extern "C" {
    fn cx_transport_group_empty(group: i32, except: i32, exited_child: i32) -> i32;
}
impl Drop for Helper {
    fn drop(&mut self) {
        self.channel.take();
        if self.child.is_some() && self.cleanup(true).is_err() {
            eprintln!("mac_transport_group_cleanup_unresolved");
        }
    }
}

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU8, Ordering},
    mpsc,
};
const OPENING: u8 = 0;
const READY: u8 = 1;
const FAILED: u8 = 2;
const FINISHED: u8 = 3;
enum Work {
    Observe,
    CrashHelper,
    FixtureGroup,
}
struct Query {
    work: Work,
    deadline: Instant,
    interest: Arc<AtomicBool>,
    reply: mpsc::SyncSender<Result<String, Error>>,
}
#[derive(Clone)]
pub(crate) struct StatusReader {
    sender: mpsc::SyncSender<Query>,
    state: Arc<AtomicU8>,
}
impl StatusReader {
    pub(crate) fn try_transport_device_id(&self) -> Result<String, Error> {
        self.call(Work::Observe)
    }
    fn call(&self, work: Work) -> Result<String, Error> {
        if self.state.load(Ordering::Acquire) != READY {
            return Err(Error::Unavailable);
        }
        let (reply, receive) = mpsc::sync_channel(1);
        let interest = Arc::new(AtomicBool::new(true));
        let deadline = Instant::now() + BUDGET;
        self.sender
            .try_send(Query {
                work,
                deadline,
                interest: interest.clone(),
                reply,
            })
            .map_err(|_| Error::Unavailable)?;
        let result = receive
            .recv_timeout(BUDGET)
            .map_err(|_| Error::Deadline)
            .and_then(|v| v);
        interest.store(false, Ordering::Release);
        result
    }
}
pub(crate) struct Worker {
    reader: StatusReader,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<Result<(), Error>>>,
    credential: Arc<Mutex<Option<String>>>,
}
impl Worker {
    pub(crate) fn spawn(root: PathBuf, create: bool) -> Result<Self, Error> {
        let root = if create {
            create_root(&root)?
        } else {
            let root = root.canonicalize().map_err(|_| Error::Profile)?;
            private_root(&root)?;
            root
        };
        let (sender, receive) = mpsc::sync_channel::<Query>(8);
        let state = Arc::new(AtomicU8::new(OPENING));
        let stop = Arc::new(AtomicBool::new(false));
        let credential = Arc::new(Mutex::new(None));
        let read = StatusReader {
            sender,
            state: state.clone(),
        };
        let worker_stop = stop.clone();
        let record = credential.clone();
        let thread = std::thread::Builder::new()
            .name("mac-transport-native-owner".into())
            .spawn(move || {
                let result = (|| {
                    let mut helper = Helper::start(&root, &record)?;
                    state.store(READY, Ordering::Release);
                    while !worker_stop.load(Ordering::Acquire) {
                        match receive.recv_timeout(Duration::from_millis(250)) {
                            Ok(query) => {
                                if !query.interest.load(Ordering::Acquire)
                                    || Instant::now() >= query.deadline
                                {
                                    continue;
                                }
                                if matches!(query.work, Work::CrashHelper) {
                                    helper
                                        .child
                                        .as_mut()
                                        .ok_or(Error::Engine)?
                                        .kill()
                                        .map_err(|_| Error::Engine)?;
                                    state.store(FAILED, Ordering::Release);
                                    let _ = query.reply.try_send(Err(Error::Engine));
                                    return Err(Error::Engine);
                                }
                                if matches!(query.work, Work::FixtureGroup) {
                                    let group = helper.child.as_ref().ok_or(Error::Engine)?.id();
                                    let _ = query.reply.try_send(Ok(group.to_string()));
                                    continue;
                                }
                                let result = helper
                                    .request(Operation::Observe)
                                    .map(|_| helper.identity.clone());
                                if result.is_err() {
                                    state.store(FAILED, Ordering::Release);
                                }
                                if query.interest.load(Ordering::Acquire)
                                    && Instant::now() < query.deadline
                                {
                                    let _ = query.reply.try_send(result.clone());
                                }
                                result?;
                            }
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                helper.request(Operation::Observe)?
                            }
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    }
                    helper.close()?;
                    Ok(())
                })();
                state.store(
                    if result.is_ok() { FINISHED } else { FAILED },
                    Ordering::Release,
                );
                result
            })
            .map_err(|_| Error::Unavailable)?;
        Ok(Self {
            reader: read,
            stop,
            thread: Some(thread),
            credential,
        })
    }
    pub(crate) fn status_reader(&self) -> StatusReader {
        self.reader.clone()
    }
    fn join(&mut self) -> Result<(), Error> {
        self.stop.store(true, Ordering::Release);
        self.thread
            .take()
            .ok_or(Error::Cleanup)?
            .join()
            .map_err(|_| Error::Cleanup)?
    }
    pub(crate) fn close(mut self) -> Result<(), Error> {
        self.join()
    }
    fn fixture_credential(&self) -> Result<String, Error> {
        self.credential
            .lock()
            .map_err(|_| Error::Unavailable)?
            .clone()
            .ok_or(Error::Profile)
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        if self.thread.is_some()
            && let Err(error) = self.join()
        {
            eprintln!("component=mac_transport operation=worker_exit status=failed error={error}");
        }
    }
}

/// Exact signed daemon driver; no canonical profile, approval or UI route.
fn wait_identity(reader: StatusReader) -> Result<String, Error> {
    let until = Instant::now() + Duration::from_secs(45);
    loop {
        match reader.try_transport_device_id() {
            Ok(id) => return Ok(id),
            Err(_) if reader.state.load(Ordering::Acquire) == OPENING && Instant::now() < until => {
                std::thread::sleep(Duration::from_millis(50))
            }
            Err(e) => return Err(e),
        }
    }
}

fn loss_driver(root: &Path) -> Result<(), Error> {
    let worker = Worker::spawn(root.to_owned(), true)?;
    wait_identity(worker.status_reader())?;
    let group = worker.reader.call(Work::FixtureGroup)?;
    println!("mac_transport_loss_ready={group}");
    std::io::stdout().flush().map_err(|_| Error::Channel)?;
    // The external bounded fixture kills this exact captured driver child.
    // There is no request/renderer route for this pause or the group number.
    std::thread::sleep(Duration::from_secs(60));
    worker.close()?;
    Err(Error::Deadline)
}

pub fn check(root: &Path) -> Result<(), Error> {
    let first = Worker::spawn(root.to_owned(), true)?;
    let cleanup = first.credential.clone();
    let result = (|| {
        let identity = wait_identity(first.status_reader())?;
        let credential = first.fixture_credential()?;
        if first.reader.try_transport_device_id()? != identity {
            return Err(Error::Engine);
        }
        first.close()?;
        eprintln!("mac_transport_clean_stop_ack=passed");
        let second = Worker::spawn(root.to_owned(), false)?;
        if wait_identity(second.status_reader())? != identity
            || second.fixture_credential()? != credential
        {
            return Err(Error::Engine);
        }
        second
            .reader
            .call(Work::CrashHelper)
            .err()
            .ok_or(Error::Engine)?;
        if second.close() != Err(Error::Engine) {
            return Err(Error::Cleanup);
        }
        let third = Worker::spawn(root.to_owned(), false)?;
        if wait_identity(third.status_reader())? != identity {
            return Err(Error::Engine);
        }
        third.close()?;
        // Out-of-band fixture corruption must invalidate a running identity
        // reader. The observer must leave the changed bytes intact, and only
        // this explicit isolated fixture restores them after verified cleanup.
        for leaf in [
            "native/config.xml",
            "native/cert.pem",
            "native/key.pem",
            "profile.json",
            "native-init.json",
            "native-ready.json",
            "native-generation-started",
            "native-generation-complete",
            "api-ready",
            "api-binding.json",
            "package/AUTHORS.txt",
        ] {
            use std::os::unix::fs::PermissionsExt;
            let worker = Worker::spawn(root.to_owned(), false)?;
            if wait_identity(worker.status_reader())? != identity {
                return Err(Error::Engine);
            }
            let path = if leaf.starts_with("package/") {
                root.join(leaf)
            } else {
                root.join(PROFILE).join(leaf)
            };
            let original = std::fs::read(&path).map_err(|_| Error::Profile)?;
            let permissions = std::fs::metadata(&path)
                .map_err(|_| Error::Profile)?
                .permissions();
            let mut changed = original.clone();
            if leaf == "native/config.xml" {
                let text = std::str::from_utf8(&original).map_err(|_| Error::Profile)?;
                let bad = text.replace(
                    "<globalAnnounceEnabled>false</globalAnnounceEnabled>",
                    "<globalAnnounceEnabled>true</globalAnnounceEnabled>",
                );
                if bad.as_bytes() == original {
                    return Err(Error::Profile);
                }
                changed = bad.into_bytes();
            } else if leaf.ends_with(".json") {
                let mut json: serde_json::Value =
                    serde_json::from_slice(&original).map_err(|_| Error::Profile)?;
                json["schema"] = 2.into();
                changed = serde_json::to_vec(&json).map_err(|_| Error::Profile)?;
            } else {
                changed.extend_from_slice(b"\nchanged-fixture\n");
            }
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .map_err(|_| Error::Profile)?;
            std::fs::write(&path, &changed).map_err(|_| Error::Profile)?;
            let refused = worker.reader.try_transport_device_id().is_err();
            let failed = worker.close().is_err();
            if !refused
                || !failed
                || root.join("transport.active").exists()
                || std::fs::read(&path).map_err(|_| Error::Profile)? != changed
            {
                return Err(Error::Policy);
            }
            std::fs::write(&path, &original).map_err(|_| Error::Profile)?;
            std::fs::set_permissions(&path, permissions).map_err(|_| Error::Profile)?;
        }
        eprintln!("mac_transport_live_profile_package_changes_refused=passed");
        // Missing existing credentials must not regenerate native identity or
        // rewrite the bound configuration. Keep private snapshot bytes in RAM.
        let snapshot = || -> Result<Vec<Vec<u8>>, Error> {
            let base = root.join(PROFILE);
            let mut bytes = Vec::new();
            for leaf in [
                "profile.json",
                "api-binding.json",
                "native-init.json",
                "native-ready.json",
                "native-generation-started",
                "native-generation-complete",
                "native/config.xml",
                "native/cert.pem",
                "native/key.pem",
            ] {
                let path = base.join(leaf);
                let metadata = std::fs::symlink_metadata(&path).map_err(|_| Error::Profile)?;
                if !metadata.is_file() || metadata.nlink() != 1 || metadata.len() > 256 * 1024 {
                    return Err(Error::Profile);
                }
                bytes.push(std::fs::read(path).map_err(|_| Error::Profile)?);
            }
            Ok(bytes)
        };
        let before = snapshot()?;
        let (service, account) =
            crate::syncthing_rest::credential_labels(SecureStoreEnvironment::Dev, &credential)
                .map_err(|_| Error::Profile)?;
        let store = MacosKeychainStore::new();
        store
            .delete(&service, &account)
            .map_err(|_| Error::Cleanup)?;
        let missing = Worker::spawn(root.to_owned(), false)?;
        if wait_identity(missing.status_reader()).is_ok()
            || missing.close() != Err(Error::CredentialUnavailable)
            || snapshot()? != before
            || !matches!(
                store.get(&service, &account),
                Err(crate::SecureStoreError::Missing)
            )
        {
            return Err(Error::Profile);
        }
        eprintln!("mac_transport_owned_identity_restart=passed");
        eprintln!("mac_transport_helper_loss_cleanup=passed");
        eprintln!("mac_transport_missing_key_no_regeneration=passed");
        Ok(())
    })();
    let deleted = (|| {
        let id = cleanup
            .lock()
            .map_err(|_| Error::Cleanup)?
            .clone()
            .ok_or(Error::Cleanup)?;
        let (service, account) =
            crate::syncthing_rest::credential_labels(SecureStoreEnvironment::Dev, &id)
                .map_err(|_| Error::Cleanup)?;
        let store = MacosKeychainStore::new();
        match store.get(&service, &account) {
            Ok(_) => store
                .delete(&service, &account)
                .map_err(|_| Error::Cleanup)?,
            Err(crate::SecureStoreError::Missing) => {}
            Err(_) => return Err(Error::Cleanup),
        }
        if !matches!(
            store.get(&service, &account),
            Err(crate::SecureStoreError::Missing)
        ) {
            return Err(Error::Cleanup);
        }
        Ok(())
    })();
    result.and(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn helper_protocol_has_no_owner_commands_or_ambient_role() {
        assert!(!role_entered());
        assert!(!role_active());
        assert!(decode::<Request>(br#"{"sequence":1,"operation":"observe"}"#).is_ok());
        for raw in [
            br#"{"sequence":1,"operation":"bootstrap.approval.confirm"}"#.as_slice(),
            br#"{"sequence":1,"operation":"observe","capability":"forged"}"#.as_slice(),
            br#"{"sequence":1,"sequence":2,"operation":"observe"}"#.as_slice(),
        ] {
            assert!(decode::<Request>(raw).is_err());
        }
        assert!(decode::<Request>(&vec![b' '; 1025]).is_err());
    }
    #[test]
    fn helper_arguments_cannot_enter_ordinary_lifecycle_or_supply_a_pid() {
        for args in [
            vec![FLAG.into()],
            vec![FLAG.into(), "aa".repeat(16), "relative".into()],
            vec![
                FLAG.into(),
                "aa".repeat(16),
                "/tmp/unused".into(),
                "lifecycle".into(),
            ],
        ] {
            assert!(entry(&args).unwrap().is_err());
        }
        let spec = crate::owned_process::LaunchSpec::syncthing(
            Path::new("/tmp/unused-engine"),
            Path::new("/tmp/unused-home"),
        )
        .unwrap();
        assert!(matches!(
            crate::owned_process::OwnedProcess::spawn(&spec),
            Err(crate::owned_process::OwnedProcessError::InvalidLaunch)
        ));
    }
    #[test]
    fn helper_root_rejects_shared_marker_without_rewriting_it() {
        let base = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "cx-helper-root-{}",
            epoch_text(&{
                let mut b = [0; 16];
                getrandom::getrandom(&mut b).unwrap();
                b
            })
        ));
        let root = create_root(&base).unwrap();
        let link = root.join("marker-alias");
        std::fs::hard_link(root.join("transport.marker"), &link).unwrap();
        assert!(private_root(&root).is_err());
        assert_eq!(std::fs::read(&link).unwrap(), MARKER);
        std::fs::remove_dir_all(root).unwrap();
    }
}
