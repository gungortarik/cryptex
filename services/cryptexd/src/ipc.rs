//! Local daemon IPC. Health and guarded Desktop retrieval are the only exposed
//! operations; the daemon owns canonical verification and SQLite access.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::compat::DAEMON_PROTOCOL_VERSION;
use crate::knowledge_materialization::{
    GuardedSearchError, RetrievalSafetyHold, search_events_with_retrieval_safety,
};
use crate::local_device_identity::LocalDeviceKeyResolver;
use crate::secure_store::{SecureStore, SecureStoreEnvironment};
use crate::{
    identity_runtime::load_brain_identity, local_device_identity::load_local_device_identity,
};
use cryptex_core::DeviceId;
use cryptex_projection::{ProjectionDb, SearchHit, SearchQuery};
#[cfg(windows)]
use interprocess::local_socket::GenericNamespaced;
use interprocess::local_socket::{ListenerOptions, Stream, prelude::*};
use serde::{Deserialize, Serialize};

const PROTOCOL_VERSION: u32 = DAEMON_PROTOCOL_VERSION;
const MAX_FRAME_BYTES: usize = 64 * 1024;
const CAPABILITY_STATUS_READ: &str = "status.read";
const CAPABILITY_CONTENT_SEARCH: &str = "content.search";
const CAPABILITY_IDENTITY_SETUP: &str = "identity.setup";
const MAX_SEARCH_TEXT_BYTES: usize = 1024;
const MAX_SEARCH_FILTER_BYTES: usize = 128;
const MAX_SEARCH_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = MAX_SEARCH_RESPONSE_BYTES;
const IPC_REQUEST_DEADLINE: Duration = Duration::from_secs(2);
const IPC_RESPONSE_DEADLINE: Duration = Duration::from_secs(15);
const MAX_UNAUTHENTICATED_IPC_WORKERS: usize = 4;
#[cfg(windows)]
const ENDPOINT_ID: &str = "cryptexd-v1";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    protocol_version: u32,
    request_id: String,
    client_role: String,
    capabilities: Vec<String>,
    operation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    query: Option<DesktopSearchQuery>,
}

/// Bounded read-only search request accepted from the Desktop role only.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopSearchQuery {
    pub text: Option<String>,
    pub classification: Option<String>,
    pub event_type: Option<String>,
    pub device_id: Option<String>,
    pub created_at_from: Option<String>,
    pub created_at_to: Option<String>,
    pub limit: Option<u32>,
}

impl DesktopSearchQuery {
    fn validate_and_convert(&self) -> Result<SearchQuery, &'static str> {
        if self
            .text
            .as_ref()
            .is_some_and(|value| value.is_empty() || value.len() > MAX_SEARCH_TEXT_BYTES)
        {
            return Err("invalid_search_text");
        }
        for value in [
            self.classification.as_ref(),
            self.event_type.as_ref(),
            self.device_id.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            if value.is_empty() || value.len() > MAX_SEARCH_FILTER_BYTES {
                return Err("invalid_search_filter");
            }
        }
        for value in [self.created_at_from.as_ref(), self.created_at_to.as_ref()]
            .into_iter()
            .flatten()
        {
            if value.is_empty() || value.len() > 64 {
                return Err("invalid_search_time_bound");
            }
        }
        if self.limit.is_some_and(|limit| !(1..=20).contains(&limit)) {
            return Err("invalid_search_limit");
        }
        Ok(SearchQuery {
            text: self.text.clone(),
            classification: self.classification.clone(),
            event_type: self.event_type.clone(),
            device_id: self.device_id.clone(),
            brain_id: None,
            created_at_from: self.created_at_from.clone(),
            created_at_to: self.created_at_to.clone(),
            limit: Some(self.limit.unwrap_or(10)),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSearchHit {
    pub event_id: String,
    pub brain_id: String,
    pub device_id: String,
    pub device_seq: String,
    pub created_at: String,
    pub event_type: String,
    pub classification: String,
    pub text: String,
}

impl From<SearchHit> for DesktopSearchHit {
    fn from(hit: SearchHit) -> Self {
        Self {
            event_id: hit.event_id,
            brain_id: hit.brain_id,
            device_id: hit.device_id,
            device_seq: hit.device_seq,
            created_at: hit.created_at,
            event_type: hit.event_type,
            classification: hit.classification,
            text: hit.text,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSearchResponse {
    pub protocol_version: u32,
    pub request_id: Option<String>,
    pub accepted: bool,
    pub error: Option<String>,
    pub operation: Option<String>,
    pub daemon_pid: u32,
    pub hits: Option<Vec<DesktopSearchHit>>,
    pub hold_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopEnrollmentResponse {
    pub protocol_version: u32,
    pub request_id: Option<String>,
    pub accepted: bool,
    pub error: Option<String>,
    pub operation: Option<String>,
    pub daemon_pid: u32,
    pub device_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub protocol_version: u32,
    pub request_id: Option<String>,
    pub accepted: bool,
    pub error: Option<String>,
    pub operation: Option<String>,
    pub daemon_pid: u32,
}

/// Bind the daemon endpoint and serve the narrow health operation on a worker.
/// The worker exits with the daemon process. The caller must already hold the
/// daemon's single-instance lock before binding this stable endpoint.
pub fn spawn_health_server() -> io::Result<()> {
    spawn_server(None)
}

/// Bind the IPC endpoint with the local SecureStore context needed for guarded
/// Desktop search. The caller already owns the daemon instance lock.
pub fn spawn_retrieval_server(
    runtime_root: &Path,
    store: Arc<dyn SecureStore>,
    environment: SecureStoreEnvironment,
) -> io::Result<()> {
    let expected_desktop = desktop_process_identity()?;
    spawn_server(Some(SearchContext {
        runtime_root: runtime_root.to_path_buf(),
        store,
        environment,
        expected_desktop,
    }))
}

struct SearchContext {
    runtime_root: PathBuf,
    store: Arc<dyn SecureStore>,
    environment: SecureStoreEnvironment,
    expected_desktop: DesktopProcessIdentity,
}

struct DesktopProcessIdentity {
    pid: u32,
    #[cfg(windows)]
    process: std::os::windows::io::OwnedHandle,
}

impl DesktopProcessIdentity {
    fn matches_peer(&self, peer_pid: u32) -> bool {
        if peer_pid != self.pid {
            return false;
        }
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
            use windows_sys::Win32::System::Threading::WaitForSingleObject;

            unsafe { WaitForSingleObject(self.process.as_raw_handle(), 0) == WAIT_TIMEOUT }
        }
        #[cfg(unix)]
        {
            unsafe { libc::getppid() == self.pid as libc::pid_t }
        }
    }
}

fn spawn_server(context: Option<SearchContext>) -> io::Result<()> {
    let listener = create_listener()?;
    let context = context.map(Arc::new);
    let unauthenticated_workers = Arc::new(AtomicUsize::new(0));
    thread::Builder::new()
        .name("cryptex-ipc".into())
        .spawn(move || {
            for incoming in listener.incoming() {
                match incoming {
                    Ok(mut stream) => {
                        let is_desktop = context.as_ref().is_some_and(|context| {
                            peer_process_id(&stream)
                                .ok()
                                .is_some_and(|pid| context.expected_desktop.matches_peer(pid))
                        });
                        if context.is_some() && !is_desktop {
                            let Some(reservation) =
                                try_reserve_ipc_worker(&unauthenticated_workers)
                            else {
                                continue;
                            };
                            let worker_context = context.clone();
                            if let Err(error) = thread::Builder::new()
                                .name("cryptex-ipc-health".into())
                                .spawn(move || {
                                    let _reservation = reservation;
                                    if let Err(error) =
                                        handle_connection(&mut stream, worker_context.as_deref())
                                    {
                                        eprintln!("cryptexd IPC connection failed: {error}");
                                    }
                                })
                            {
                                eprintln!("cryptexd IPC worker failed to start: {error}");
                            }
                        } else if let Err(error) =
                            handle_connection(&mut stream, context.as_deref())
                        {
                            eprintln!("cryptexd IPC connection failed: {error}");
                        }
                    }
                    Err(error) => eprintln!("cryptexd IPC accept failed: {error}"),
                }
            }
        })?;
    Ok(())
}

fn try_reserve_ipc_worker(active: &Arc<AtomicUsize>) -> Option<IpcWorkerReservation> {
    active
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
            (current < MAX_UNAUTHENTICATED_IPC_WORKERS).then_some(current + 1)
        })
        .ok()
        .map(|_| IpcWorkerReservation(Arc::clone(active)))
}

struct IpcWorkerReservation(Arc<AtomicUsize>);

impl Drop for IpcWorkerReservation {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Connect as a Desktop client, retrying while the daemon endpoint is absent.
/// Each attempt opens a fresh stream so this function can reconnect after a
/// supervised daemon process restart.
pub fn desktop_health_ping(timeout: Duration) -> io::Result<HealthResponse> {
    health_ping("desktop", timeout)
}

pub fn health_ping(role: &str, timeout: Duration) -> io::Result<HealthResponse> {
    if !matches!(role, "desktop" | "cli" | "mcp") {
        return Err(invalid_data("unsupported local client role"));
    }
    let name = socket_name()?;
    let started = Instant::now();
    let mut last_error = None;
    let mut stream = loop {
        match Stream::connect(name.clone()) {
            Ok(stream) => break stream,
            Err(error) if started.elapsed() < timeout => {
                last_error = Some(error);
                thread::sleep(Duration::from_millis(50));
            }
            Err(error) => {
                return Err(last_error.unwrap_or(error));
            }
        }
    };

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let request_id = format!("{}-{nonce}", process::id());
    let request = Request {
        protocol_version: PROTOCOL_VERSION,
        request_id: request_id.clone(),
        client_role: role.into(),
        capabilities: vec![CAPABILITY_STATUS_READ.into()],
        operation: "health.ping".into(),
        query: None,
    };
    let frame = exchange(&mut stream, &request, MAX_RESPONSE_BYTES)?;
    let response: HealthResponse = serde_json::from_slice(&frame)?;
    if response.protocol_version != PROTOCOL_VERSION
        || response.request_id.as_deref() != Some(&request_id)
        || (response.accepted && response.operation.as_deref() != Some("health.pong"))
    {
        return Err(invalid_data("IPC response did not match the request"));
    }
    if !response.accepted {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            response.error.as_deref().unwrap_or("IPC request rejected"),
        ));
    }
    Ok(response)
}

pub fn desktop_search(
    query: DesktopSearchQuery,
    timeout: Duration,
) -> io::Result<DesktopSearchResponse> {
    let name = socket_name()?;
    let started = Instant::now();
    let mut last_error = None;
    let mut stream = loop {
        match Stream::connect(name.clone()) {
            Ok(stream) => break stream,
            Err(error) if started.elapsed() < timeout => {
                last_error = Some(error);
                thread::sleep(Duration::from_millis(50));
            }
            Err(error) => return Err(last_error.unwrap_or(error)),
        }
    };
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let request_id = format!("{}-{nonce}", process::id());
    let request = Request {
        protocol_version: PROTOCOL_VERSION,
        request_id: request_id.clone(),
        client_role: "desktop".into(),
        capabilities: vec![CAPABILITY_CONTENT_SEARCH.into()],
        operation: "content.search".into(),
        query: Some(query),
    };
    let frame = exchange(&mut stream, &request, MAX_SEARCH_RESPONSE_BYTES)?;
    let response: DesktopSearchResponse = serde_json::from_slice(&frame)?;
    if response.protocol_version != PROTOCOL_VERSION
        || response.request_id.as_deref() != Some(&request_id)
        || (response.accepted && response.operation.as_deref() != Some("content.search.result"))
    {
        return Err(invalid_data(
            "IPC search response did not match the request",
        ));
    }
    Ok(response)
}

/// Explicit user-invoked local identity enrollment. This is never called as a
/// side effect of starting the Desktop, searching, or loading a window.
pub fn desktop_enroll_local_device(timeout: Duration) -> io::Result<DesktopEnrollmentResponse> {
    let name = socket_name()?;
    let started = Instant::now();
    let mut last_error = None;
    let mut stream = loop {
        match Stream::connect(name.clone()) {
            Ok(stream) => break stream,
            Err(error) if started.elapsed() < timeout => {
                last_error = Some(error);
                thread::sleep(Duration::from_millis(50));
            }
            Err(error) => return Err(last_error.unwrap_or(error)),
        }
    };
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let request_id = format!("{}-{nonce}", process::id());
    let request = Request {
        protocol_version: PROTOCOL_VERSION,
        request_id: request_id.clone(),
        client_role: "desktop".into(),
        capabilities: vec![CAPABILITY_IDENTITY_SETUP.into()],
        operation: "identity.enroll_local".into(),
        query: None,
    };
    let frame = exchange(&mut stream, &request, MAX_RESPONSE_BYTES)?;
    let response: DesktopEnrollmentResponse = serde_json::from_slice(&frame)?;
    if response.protocol_version != PROTOCOL_VERSION
        || response.request_id.as_deref() != Some(&request_id)
        || (response.accepted && response.operation.as_deref() != Some("identity.enrollment_ready"))
    {
        return Err(invalid_data(
            "IPC enrollment response did not match the request",
        ));
    }
    Ok(response)
}

fn exchange(
    stream: &mut Stream,
    request: &Request,
    max_response_bytes: usize,
) -> io::Result<Vec<u8>> {
    stream.set_nonblocking(true)?;
    let mut request_frame = serde_json::to_vec(request)?;
    request_frame.push(b'\n');
    let deadline = Instant::now() + IPC_RESPONSE_DEADLINE;
    write_all_until(stream, &request_frame, deadline)?;
    read_frame_until(stream, max_response_bytes, deadline)
}

fn create_listener() -> io::Result<interprocess::local_socket::Listener> {
    let options = ListenerOptions::new()
        .name(socket_name()?)
        .try_overwrite(true);
    #[cfg(windows)]
    {
        use interprocess::os::windows::{
            local_socket::ListenerOptionsExt, security_descriptor::SecurityDescriptor,
        };
        use widestring::U16CString;

        let sid = current_user_sid()?;
        let descriptor = U16CString::from_str(format!("O:{sid}D:P(A;;GA;;;{sid})"))
            .map_err(|_| invalid_data("invalid current-user pipe security descriptor"))?;
        options
            .security_descriptor(SecurityDescriptor::deserialize(descriptor.as_ucstr())?)
            .create_sync()
    }
    #[cfg(unix)]
    {
        options.create_sync()
    }
}

#[cfg(windows)]
fn socket_name() -> io::Result<interprocess::local_socket::Name<'static>> {
    std::ffi::OsString::from(format!("cryptex-{ENDPOINT_ID}")).to_ns_name::<GenericNamespaced>()
}

#[cfg(windows)]
fn current_user_sid() -> io::Result<String> {
    use std::{mem, ptr};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE, LocalFree},
        Security::Authorization::ConvertSidToStringSidW,
        Security::{GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser},
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };

    let mut token: HANDLE = ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let result = (|| {
        let mut required = 0u32;
        unsafe { GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut required) };
        if required == 0 {
            return Err(io::Error::last_os_error());
        }
        let words = (required as usize).div_ceil(mem::size_of::<usize>());
        let mut storage = vec![0usize; words];
        if unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                storage.as_mut_ptr().cast(),
                required,
                &mut required,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let token_user = unsafe { &*(storage.as_ptr().cast::<TOKEN_USER>()) };
        let mut sid_text = ptr::null_mut();
        if unsafe { ConvertSidToStringSidW(token_user.User.Sid, &mut sid_text) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let sid = unsafe {
            let mut length = 0usize;
            while *sid_text.add(length) != 0 {
                length += 1;
            }
            String::from_utf16(std::slice::from_raw_parts(sid_text, length))
        }
        .map_err(|_| invalid_data("current account SID is not valid UTF-16"));
        unsafe { LocalFree(sid_text.cast()) };
        sid
    })();
    unsafe { CloseHandle(token) };
    result
}

#[cfg(unix)]
fn socket_name() -> io::Result<interprocess::local_socket::Name<'static>> {
    use interprocess::local_socket::GenericFilePath;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};

    let parent = socket_parent();
    match std::fs::symlink_metadata(&parent) {
        Ok(metadata) => {
            if !metadata.file_type().is_dir()
                || metadata.uid() != unsafe { libc::getuid() }
                || metadata.permissions().mode() & 0o077 != 0
            {
                return Err(invalid_data(
                    "IPC runtime directory ownership or mode is unsafe",
                ));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut builder = std::fs::DirBuilder::new();
            builder.mode(0o700);
            builder.create(&parent)?;
        }
        Err(error) => return Err(error),
    }
    let path = parent.join("s");
    let text = path
        .to_str()
        .ok_or_else(|| invalid_data("IPC socket path is not valid UTF-8"))?;
    if cfg!(target_os = "macos") && text.len() >= 104 {
        return Err(invalid_data(
            "IPC socket path exceeds macOS sockaddr_un capacity",
        ));
    }
    path.to_fs_name::<GenericFilePath>()
}

#[cfg(unix)]
fn socket_parent() -> std::path::PathBuf {
    let uid = unsafe { libc::getuid() };
    std::path::PathBuf::from("/tmp").join(format!("cpx-cryptex-{uid}"))
}

fn handle_connection(stream: &mut Stream, search: Option<&SearchContext>) -> io::Result<()> {
    let peer_is_desktop = search
        .and_then(|context| {
            peer_process_id(stream)
                .ok()
                .map(|pid| context.expected_desktop.matches_peer(pid))
        })
        .unwrap_or(false);
    stream.set_nonblocking(true)?;
    let request_deadline = Instant::now() + IPC_REQUEST_DEADLINE;
    let frame = match read_frame_until(stream, MAX_FRAME_BYTES, request_deadline) {
        Ok(frame) => frame,
        Err(error) if error.kind() == io::ErrorKind::TimedOut => return Err(error),
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {
            let response = serde_json::to_vec(&rejected(None, frame_rejection_code(&error)))?;
            return write_response(stream, &response, Instant::now() + IPC_REQUEST_DEADLINE);
        }
        Err(error) => return Err(error),
    };
    let response = if frame.is_empty() {
        return Ok(());
    } else {
        match serde_json::from_slice::<Request>(&frame) {
            Ok(request) => {
                if request.operation == "content.search" {
                    let response = handle_search(request, search, peer_is_desktop);
                    let bytes = serde_json::to_vec(&response)?;
                    if bytes.len() + 1 > MAX_SEARCH_RESPONSE_BYTES {
                        serde_json::to_vec(&search_rejected(
                            response.request_id,
                            "response_too_large",
                        ))?
                    } else {
                        bytes
                    }
                } else if request.operation == "identity.enroll_local" {
                    serde_json::to_vec(&handle_enrollment(request, search, peer_is_desktop))?
                } else {
                    serde_json::to_vec(&authorize(request))?
                }
            }
            Err(_) => serde_json::to_vec(&rejected(None, "malformed_request"))?,
        }
    };
    write_response(stream, &response, Instant::now() + IPC_RESPONSE_DEADLINE)
}

fn write_response(stream: &mut Stream, response: &[u8], deadline: Instant) -> io::Result<()> {
    let mut frame = Vec::with_capacity(response.len() + 1);
    frame.extend_from_slice(response);
    frame.push(b'\n');
    write_all_until(stream, &frame, deadline)
}

fn frame_rejection_code(error: &io::Error) -> &'static str {
    if error.to_string() == "IPC frame exceeds limit" {
        "frame_too_large"
    } else {
        "malformed_frame"
    }
}

fn write_all_until<W: Write>(
    writer: &mut W,
    mut bytes: &[u8],
    deadline: Instant,
) -> io::Result<()> {
    while !bytes.is_empty() {
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "IPC frame deadline exceeded",
            ));
        }
        match writer.write(bytes) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "IPC stream closed",
                ));
            }
            Ok(written) => bytes = &bytes[written..],
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => wait_until_ready(deadline)?,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn read_frame_until<R: Read>(
    reader: &mut R,
    max_bytes: usize,
    deadline: Instant,
) -> io::Result<Vec<u8>> {
    let mut frame = Vec::with_capacity(1024);
    let mut chunk = [0_u8; 4096];
    loop {
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "IPC frame deadline exceeded",
            ));
        }
        match reader.read(&mut chunk) {
            Ok(0) if frame.is_empty() => return Ok(frame),
            Ok(0) => return Err(invalid_data("incomplete IPC frame")),
            Ok(read) => {
                let bytes = &chunk[..read];
                let line_end = bytes.iter().position(|byte| *byte == b'\n');
                let append_len = line_end.map_or(bytes.len(), |index| index + 1);
                if frame.len().saturating_add(append_len) > max_bytes {
                    return Err(invalid_data("IPC frame exceeds limit"));
                }
                frame.extend_from_slice(&bytes[..append_len]);
                if let Some(end) = line_end {
                    if end + 1 != bytes.len() {
                        return Err(invalid_data("trailing bytes after IPC frame"));
                    }
                    return Ok(frame);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => wait_until_ready(deadline)?,
            Err(error) => return Err(error),
        }
    }
}

fn wait_until_ready(deadline: Instant) -> io::Result<()> {
    let now = Instant::now();
    if now >= deadline {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "IPC frame deadline exceeded",
        ));
    }
    thread::sleep(Duration::from_millis(5).min(deadline - now));
    Ok(())
}

#[cfg(windows)]
fn desktop_process_identity() -> io::Result<DesktopProcessIdentity> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        QueryFullProcessImageNameW,
    };

    let pid = windows_parent_process_id()?;
    let process = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            pid,
        )
    };
    if process.is_null() {
        return Err(io::Error::last_os_error());
    }
    let mut image_buffer = vec![0_u16; 32_768];
    let mut image_length = image_buffer.len() as u32;
    if unsafe {
        QueryFullProcessImageNameW(process, 0, image_buffer.as_mut_ptr(), &mut image_length)
    } == 0
    {
        let error = io::Error::last_os_error();
        unsafe { CloseHandle(process) };
        return Err(error);
    }
    use std::os::windows::ffi::OsStringExt;
    let parent_path = std::path::PathBuf::from(std::ffi::OsString::from_wide(
        &image_buffer[..image_length as usize],
    ));
    if !is_packaged_desktop_path(&parent_path) {
        unsafe { CloseHandle(process) };
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "daemon parent executable is not Cryptex Desktop",
        ));
    }
    use std::os::windows::io::FromRawHandle;
    Ok(DesktopProcessIdentity {
        pid,
        process: unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(process) },
    })
}

#[cfg(windows)]
fn windows_parent_process_id() -> io::Result<u32> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..PROCESSENTRY32W::default()
    };
    let mut found = None;
    let mut succeeded = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
    while succeeded {
        if entry.th32ProcessID == process::id() {
            found = Some(entry.th32ParentProcessID);
            break;
        }
        succeeded = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
    }
    let error = found.is_none().then(io::Error::last_os_error);
    unsafe { CloseHandle(snapshot) };
    found.ok_or_else(|| error.unwrap_or_else(|| invalid_data("daemon parent process not found")))
}

#[cfg(all(test, windows))]
fn test_desktop_process_identity() -> io::Result<DesktopProcessIdentity> {
    use std::os::windows::io::FromRawHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE};

    let pid = windows_parent_process_id()?;
    let process = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if process.is_null() {
        return Err(io::Error::last_os_error());
    }
    Ok(DesktopProcessIdentity {
        pid,
        process: unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(process) },
    })
}

#[cfg(target_os = "macos")]
fn desktop_process_identity() -> io::Result<DesktopProcessIdentity> {
    let pid = unsafe { libc::getppid() };
    let pid = u32::try_from(pid).map_err(|_| invalid_data("invalid daemon parent process id"))?;
    let parent_path = process_image_path(pid)?;
    if !is_packaged_desktop_path(&parent_path) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "daemon parent executable is not Cryptex Desktop",
        ));
    }
    Ok(DesktopProcessIdentity { pid })
}

#[cfg(all(test, target_os = "macos"))]
fn test_desktop_process_identity() -> io::Result<DesktopProcessIdentity> {
    let pid = unsafe { libc::getppid() };
    let pid = u32::try_from(pid).map_err(|_| invalid_data("invalid test parent process id"))?;
    Ok(DesktopProcessIdentity { pid })
}

#[cfg(target_os = "macos")]
fn process_image_path(pid: u32) -> io::Result<PathBuf> {
    use std::ffi::CStr;
    use std::os::unix::ffi::OsStrExt;

    unsafe extern "C" {
        fn proc_pidpath(
            pid: libc::c_int,
            buffer: *mut libc::c_void,
            buffersize: u32,
        ) -> libc::c_int;
    }
    let mut buffer = vec![0_u8; 4096];
    let length = unsafe {
        proc_pidpath(
            pid as libc::c_int,
            buffer.as_mut_ptr().cast(),
            buffer.len() as u32,
        )
    };
    if length <= 0 || length as usize >= buffer.len() {
        return Err(io::Error::last_os_error());
    }
    let path = unsafe { CStr::from_ptr(buffer.as_ptr().cast()) }.to_bytes();
    Ok(Path::new(std::ffi::OsStr::from_bytes(path)).to_path_buf())
}

#[cfg(target_os = "macos")]
fn is_packaged_desktop_path(parent: &Path) -> bool {
    is_expected_sibling(parent, &["Cryptex", "cryptex-desktop"])
}

#[cfg(windows)]
fn is_packaged_desktop_path(parent: &Path) -> bool {
    is_expected_sibling(parent, &["Cryptex.exe", "cryptex-desktop.exe"])
}

#[cfg(any(windows, target_os = "macos"))]
fn is_expected_sibling(parent: &Path, names: &[&str]) -> bool {
    let Ok(daemon) = std::env::current_exe() else {
        return false;
    };
    let Some(directory) = daemon.parent() else {
        return false;
    };
    let Ok(parent) = std::fs::canonicalize(parent) else {
        return false;
    };
    names.iter().any(|name| {
        std::fs::canonicalize(directory.join(name))
            .is_ok_and(|candidate| paths_equal(&candidate, &parent))
    })
}

#[cfg(windows)]
fn paths_equal(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

#[cfg(target_os = "macos")]
fn paths_equal(left: &Path, right: &Path) -> bool {
    left == right
}

#[cfg(all(unix, not(target_os = "macos")))]
fn desktop_process_identity() -> io::Result<DesktopProcessIdentity> {
    let pid = unsafe { libc::getppid() };
    let pid = u32::try_from(pid).map_err(|_| invalid_data("invalid daemon parent process id"))?;
    Ok(DesktopProcessIdentity { pid })
}

#[cfg(windows)]
fn peer_process_id(stream: &Stream) -> io::Result<u32> {
    stream
        .peer_creds()?
        .pid()
        .ok_or_else(|| io::Error::new(io::ErrorKind::PermissionDenied, "IPC peer PID unavailable"))
}

#[cfg(target_os = "macos")]
fn peer_process_id(stream: &Stream) -> io::Result<u32> {
    use std::os::fd::AsRawFd;

    let mut pid: libc::pid_t = 0;
    let mut length = std::mem::size_of_val(&pid) as libc::socklen_t;
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_LOCAL,
            libc::LOCAL_PEERPID,
            (&mut pid as *mut libc::pid_t).cast(),
            &mut length,
        )
    };
    if result != 0 || length as usize != std::mem::size_of_val(&pid) {
        return Err(io::Error::last_os_error());
    }
    u32::try_from(pid).map_err(|_| invalid_data("invalid IPC peer process id"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn peer_process_id(stream: &Stream) -> io::Result<u32> {
    stream
        .peer_creds()?
        .pid()
        .and_then(|pid| u32::try_from(pid).ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::PermissionDenied, "IPC peer PID unavailable"))
}

fn handle_enrollment(
    request: Request,
    context: Option<&SearchContext>,
    peer_is_desktop: bool,
) -> DesktopEnrollmentResponse {
    let request_id = request.request_id.clone();
    let authorization_error = if request.protocol_version != PROTOCOL_VERSION {
        Some("incompatible_protocol_version")
    } else if request.request_id.is_empty() || request.request_id.len() > 128 {
        Some("invalid_request_id")
    } else if request.client_role != "desktop" {
        Some("unauthorized_client_role")
    } else if !peer_is_desktop {
        Some("unauthorized_client_process")
    } else if request.capabilities.as_slice() != [CAPABILITY_IDENTITY_SETUP] {
        Some("unauthorized_capability")
    } else if request.query.is_some() {
        Some("unexpected_request_payload")
    } else {
        None
    };
    if let Some(error) = authorization_error {
        return enrollment_rejected(Some(request_id), error);
    }
    let Some(context) = context else {
        return enrollment_rejected(Some(request_id), "enrollment_unavailable");
    };
    match crate::local_device_identity::enroll_local_device(
        &context.runtime_root,
        context.store.as_ref(),
        context.environment,
    ) {
        Ok(identity) => DesktopEnrollmentResponse {
            protocol_version: PROTOCOL_VERSION,
            request_id: Some(request_id),
            accepted: true,
            error: None,
            operation: Some("identity.enrollment_ready".into()),
            daemon_pid: process::id(),
            device_id: Some(identity.device_id),
        },
        Err(error) => {
            let code = match error {
                crate::local_device_identity::LocalDeviceIdentityError::Brain(_) => {
                    "brain_identity_unavailable"
                }
                crate::local_device_identity::LocalDeviceIdentityError::EnrollmentRequired => {
                    "local_enrollment_required"
                }
                crate::local_device_identity::LocalDeviceIdentityError::ExistingDeviceState => {
                    "unbound_existing_device_state"
                }
                crate::local_device_identity::LocalDeviceIdentityError::BrainMismatch
                | crate::local_device_identity::LocalDeviceIdentityError::InvalidMetadata(_) => {
                    "local_identity_invalid"
                }
                crate::local_device_identity::LocalDeviceIdentityError::EventSigningKey(_) => {
                    "secure_store_unavailable"
                }
                _ => "enrollment_failed",
            };
            enrollment_rejected(Some(request_id), code)
        }
    }
}

fn enrollment_rejected(request_id: Option<String>, error: &str) -> DesktopEnrollmentResponse {
    DesktopEnrollmentResponse {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        accepted: false,
        error: Some(error.into()),
        operation: None,
        daemon_pid: process::id(),
        device_id: None,
    }
}

fn handle_search(
    request: Request,
    context: Option<&SearchContext>,
    peer_is_desktop: bool,
) -> DesktopSearchResponse {
    let request_id = request.request_id.clone();
    let authorization_error = if request.protocol_version != PROTOCOL_VERSION {
        Some("incompatible_protocol_version")
    } else if request.request_id.is_empty() || request.request_id.len() > 128 {
        Some("invalid_request_id")
    } else if request.client_role != "desktop" {
        Some("unauthorized_client_role")
    } else if !peer_is_desktop {
        Some("unauthorized_client_process")
    } else if request.capabilities.as_slice() != [CAPABILITY_CONTENT_SEARCH] {
        Some("unauthorized_capability")
    } else if request.query.is_none() {
        Some("missing_search_query")
    } else {
        None
    };
    if let Some(error) = authorization_error {
        return search_rejected(Some(request_id), error);
    }
    let Some(context) = context else {
        return search_rejected(Some(request_id), "search_unavailable");
    };
    let Some(wire_query) = request.query else {
        return search_rejected(Some(request_id), "missing_search_query");
    };
    let query = match wire_query.validate_and_convert() {
        Ok(query) => query,
        Err(error) => return search_rejected(Some(request_id), error),
    };
    let brain = match load_brain_identity(&context.runtime_root) {
        Ok(brain) => brain,
        Err(_) => return search_held(Some(request_id), "canonical_verification_unavailable"),
    };
    let local_identity = match load_local_device_identity(&context.runtime_root, &brain.brain_id) {
        Ok(identity) => identity,
        Err(crate::local_device_identity::LocalDeviceIdentityError::EnrollmentRequired) => {
            return search_held(Some(request_id), "local_enrollment_required");
        }
        Err(_) => return search_held(Some(request_id), "canonical_verification_unavailable"),
    };
    let device = match DeviceId::new(local_identity.device_id) {
        Ok(device) => device,
        Err(_) => return search_held(Some(request_id), "canonical_verification_unavailable"),
    };
    let mut resolver = match LocalDeviceKeyResolver::load(
        &context.runtime_root,
        &brain.brain_id,
        context.store.as_ref(),
        context.environment,
    ) {
        Ok(resolver) => resolver,
        Err(_) => return search_held(Some(request_id), "canonical_verification_unavailable"),
    };
    let db = match ProjectionDb::open_or_create(&context.runtime_root) {
        Ok(db) => db,
        Err(_) => return search_held(Some(request_id), "materialization_state_missing_or_stale"),
    };
    match search_events_with_retrieval_safety(
        &context.runtime_root,
        &brain.brain_id,
        &[device],
        &mut resolver,
        &db,
        &query,
    ) {
        Ok(hits) => search_results(Some(request_id), hits),
        Err(GuardedSearchError::BrainWideHold(reason)) => {
            search_held(Some(request_id), hold_reason_code(reason))
        }
        Err(GuardedSearchError::Query(_)) => search_rejected(Some(request_id), "invalid_query"),
    }
}

fn hold_reason_code(reason: RetrievalSafetyHold) -> &'static str {
    match reason {
        RetrievalSafetyHold::CanonicalVerificationUnavailable => {
            "canonical_verification_unavailable"
        }
        RetrievalSafetyHold::UnsupportedSemantics => "unsupported_semantics",
        RetrievalSafetyHold::V2SemanticsNotIntegrated => "v2_semantics_not_integrated",
        RetrievalSafetyHold::UnresolvedDependency => "unresolved_dependency",
        RetrievalSafetyHold::MaterializationStateMissingOrStale => {
            "materialization_state_missing_or_stale"
        }
        RetrievalSafetyHold::ProjectionDoesNotMatchCanonical => {
            "projection_does_not_match_canonical"
        }
    }
}

fn search_results(request_id: Option<String>, hits: Vec<SearchHit>) -> DesktopSearchResponse {
    DesktopSearchResponse {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        accepted: true,
        error: None,
        operation: Some("content.search.result".into()),
        daemon_pid: process::id(),
        hits: Some(hits.into_iter().map(DesktopSearchHit::from).collect()),
        hold_reason: None,
    }
}

fn search_held(request_id: Option<String>, reason: &str) -> DesktopSearchResponse {
    DesktopSearchResponse {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        accepted: true,
        error: None,
        operation: Some("content.search.result".into()),
        daemon_pid: process::id(),
        hits: None,
        hold_reason: Some(reason.into()),
    }
}

fn search_rejected(request_id: Option<String>, error: &str) -> DesktopSearchResponse {
    DesktopSearchResponse {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        accepted: false,
        error: Some(error.into()),
        operation: None,
        daemon_pid: process::id(),
        hits: None,
        hold_reason: None,
    }
}

fn authorize(request: Request) -> HealthResponse {
    let request_id = request.request_id.clone();
    let error = if request.protocol_version != PROTOCOL_VERSION {
        Some("incompatible_protocol_version")
    } else if request.request_id.is_empty() || request.request_id.len() > 128 {
        Some("invalid_request_id")
    } else if !matches!(request.client_role.as_str(), "desktop" | "cli" | "mcp") {
        Some("unauthorized_client_role")
    } else if request.capabilities.as_slice() != [CAPABILITY_STATUS_READ] {
        Some("unauthorized_capability")
    } else if request.operation != "health.ping" || request.query.is_some() {
        Some("unsupported_operation")
    } else {
        None
    };
    match error {
        Some(error) => rejected(Some(request_id), error),
        None => HealthResponse {
            protocol_version: PROTOCOL_VERSION,
            request_id: Some(request_id),
            accepted: true,
            error: None,
            operation: Some("health.pong".into()),
            daemon_pid: process::id(),
        },
    }
}

fn rejected(request_id: Option<String>, error: &str) -> HealthResponse {
    HealthResponse {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        accepted: false,
        error: Some(error.into()),
        operation: None,
        daemon_pid: process::id(),
    }
}

fn invalid_data(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use cryptex_core::BrainIdentity;

    use super::*;
    use crate::identity_runtime::write_brain_identity;
    use crate::secure_store::FakeSecureStore;
    use crate::test_temp::exclusive_temp_dir;

    fn search_request(role: &str, capabilities: Vec<String>) -> Request {
        Request {
            protocol_version: PROTOCOL_VERSION,
            request_id: "test-request".into(),
            client_role: role.into(),
            capabilities,
            operation: "content.search".into(),
            query: Some(DesktopSearchQuery::default()),
        }
    }

    #[test]
    fn desktop_query_enforces_text_filter_time_and_limit_bounds() {
        assert!(DesktopSearchQuery::default().validate_and_convert().is_ok());
        assert!(
            DesktopSearchQuery {
                text: Some("x".repeat(MAX_SEARCH_TEXT_BYTES + 1)),
                ..DesktopSearchQuery::default()
            }
            .validate_and_convert()
            .is_err()
        );
        assert!(
            DesktopSearchQuery {
                classification: Some("restricted".repeat(40)),
                ..DesktopSearchQuery::default()
            }
            .validate_and_convert()
            .is_err()
        );
        assert!(
            DesktopSearchQuery {
                created_at_from: Some("x".repeat(65)),
                ..DesktopSearchQuery::default()
            }
            .validate_and_convert()
            .is_err()
        );
        assert!(
            DesktopSearchQuery {
                limit: Some(0),
                ..DesktopSearchQuery::default()
            }
            .validate_and_convert()
            .is_err()
        );
        assert!(
            DesktopSearchQuery {
                limit: Some(21),
                ..DesktopSearchQuery::default()
            }
            .validate_and_convert()
            .is_err()
        );
    }

    #[test]
    fn content_search_is_desktop_only_and_requires_exact_capability() {
        let desktop = search_request("desktop", vec![CAPABILITY_CONTENT_SEARCH.into()]);
        let response = handle_search(desktop, None, true);
        assert!(!response.accepted);
        assert_eq!(response.error.as_deref(), Some("search_unavailable"));
        assert!(response.hits.is_none());

        let cli = search_request("cli", vec![CAPABILITY_CONTENT_SEARCH.into()]);
        let response = handle_search(cli, None, true);
        assert!(!response.accepted);
        assert_eq!(response.error.as_deref(), Some("unauthorized_client_role"));
        assert!(response.hits.is_none());

        let extra_capability = search_request(
            "desktop",
            vec![
                CAPABILITY_CONTENT_SEARCH.into(),
                CAPABILITY_STATUS_READ.into(),
            ],
        );
        let response = handle_search(extra_capability, None, true);
        assert!(!response.accepted);
        assert_eq!(response.error.as_deref(), Some("unauthorized_capability"));
    }

    #[test]
    fn missing_brain_returns_hold_without_search_content() {
        let root = exclusive_temp_dir("cryptex-ipc-search-missing-brain");
        let store: Arc<dyn SecureStore> = Arc::new(FakeSecureStore::new());
        let context = SearchContext {
            runtime_root: root.clone(),
            store,
            environment: SecureStoreEnvironment::Dev,
            expected_desktop: test_desktop_process_identity().expect("desktop parent identity"),
        };
        let response = handle_search(
            search_request("desktop", vec![CAPABILITY_CONTENT_SEARCH.into()]),
            Some(&context),
            true,
        );

        assert!(response.accepted);
        assert!(response.hits.is_none());
        assert_eq!(
            response.hold_reason.as_deref(),
            Some("canonical_verification_unavailable")
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn unenrolled_brain_returns_enrollment_hold_without_search_content() {
        let root = exclusive_temp_dir("cryptex-ipc-search-unenrolled");
        let brain = BrainIdentity::new(
            "11111111-1111-4111-8111-111111111111",
            "2026-10-08T00:00:00Z",
        )
        .expect("Brain");
        write_brain_identity(&root, &brain).expect("write Brain identity");
        let store: Arc<dyn SecureStore> = Arc::new(FakeSecureStore::new());
        let context = SearchContext {
            runtime_root: root.clone(),
            store,
            environment: SecureStoreEnvironment::Dev,
            expected_desktop: test_desktop_process_identity().expect("desktop parent identity"),
        };
        let response = handle_search(
            search_request("desktop", vec![CAPABILITY_CONTENT_SEARCH.into()]),
            Some(&context),
            true,
        );

        assert!(response.accepted);
        assert!(response.hits.is_none());
        assert_eq!(
            response.hold_reason.as_deref(),
            Some("local_enrollment_required")
        );
        assert!(!crate::local_device_identity::local_device_identity_path(&root).exists());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn explicit_enrollment_operation_requires_desktop_and_returns_stable_device_id() {
        let root = exclusive_temp_dir("cryptex-ipc-enrollment");
        let brain = BrainIdentity::new(
            "11111111-1111-4111-8111-111111111111",
            "2026-10-08T00:00:00Z",
        )
        .expect("Brain");
        write_brain_identity(&root, &brain).expect("write Brain identity");
        let store = Arc::new(FakeSecureStore::new());
        let context = SearchContext {
            runtime_root: root.clone(),
            store,
            environment: SecureStoreEnvironment::Dev,
            expected_desktop: test_desktop_process_identity().expect("desktop parent identity"),
        };
        let request = || Request {
            protocol_version: PROTOCOL_VERSION,
            request_id: "setup-test".into(),
            client_role: "desktop".into(),
            capabilities: vec![CAPABILITY_IDENTITY_SETUP.into()],
            operation: "identity.enroll_local".into(),
            query: None,
        };

        let first = handle_enrollment(request(), Some(&context), true);
        let second = handle_enrollment(request(), Some(&context), true);
        assert!(first.accepted);
        assert_eq!(first.device_id, second.device_id);
        assert_eq!(
            first.operation.as_deref(),
            Some("identity.enrollment_ready")
        );

        let unauthorized = handle_enrollment(
            Request {
                client_role: "cli".into(),
                ..request()
            },
            Some(&context),
            true,
        );
        assert!(!unauthorized.accepted);
        assert_eq!(
            unauthorized.error.as_deref(),
            Some("unauthorized_client_role")
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn desktop_claim_without_authenticated_peer_process_cannot_search_or_enroll() {
        let search = handle_search(
            search_request("desktop", vec![CAPABILITY_CONTENT_SEARCH.into()]),
            None,
            false,
        );
        assert!(!search.accepted);
        assert_eq!(search.error.as_deref(), Some("unauthorized_client_process"));
        assert!(search.hits.is_none());

        let enrollment = handle_enrollment(
            Request {
                protocol_version: PROTOCOL_VERSION,
                request_id: "unauthenticated-setup".into(),
                client_role: "desktop".into(),
                capabilities: vec![CAPABILITY_IDENTITY_SETUP.into()],
                operation: "identity.enroll_local".into(),
                query: None,
            },
            None,
            false,
        );
        assert!(!enrollment.accepted);
        assert_eq!(
            enrollment.error.as_deref(),
            Some("unauthorized_client_process")
        );
        assert!(enrollment.device_id.is_none());
    }

    #[test]
    fn health_ping_remains_available_to_non_desktop_roles() {
        let response = authorize(Request {
            protocol_version: PROTOCOL_VERSION,
            request_id: "health-from-cli".into(),
            client_role: "cli".into(),
            capabilities: vec![CAPABILITY_STATUS_READ.into()],
            operation: "health.ping".into(),
            query: None,
        });
        assert!(response.accepted);
        assert_eq!(response.operation.as_deref(), Some("health.pong"));
    }

    #[test]
    fn unauthenticated_ipc_workers_are_bounded_and_release_capacity() {
        let active = Arc::new(AtomicUsize::new(0));
        let reservations = (0..MAX_UNAUTHENTICATED_IPC_WORKERS)
            .map(|_| try_reserve_ipc_worker(&active).expect("available worker slot"))
            .collect::<Vec<_>>();
        assert!(try_reserve_ipc_worker(&active).is_none());
        assert_eq!(
            active.load(Ordering::Acquire),
            MAX_UNAUTHENTICATED_IPC_WORKERS
        );
        drop(reservations);
        assert_eq!(active.load(Ordering::Acquire), 0);
        assert!(try_reserve_ipc_worker(&active).is_some());
    }

    #[test]
    fn ipc_frames_are_bounded_and_require_one_terminated_frame() {
        let mut input: &[u8] = b"{\"ok\":true}\n";
        let frame = read_frame_until(&mut input, 64, Instant::now() + Duration::from_millis(100))
            .expect("read frame");
        assert_eq!(frame, b"{\"ok\":true}\n");

        let mut oversized: &[u8] = b"123456789\n";
        let error = read_frame_until(
            &mut oversized,
            8,
            Instant::now() + Duration::from_millis(100),
        )
        .expect_err("oversized frame must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);

        let mut oversized: &[u8] = b"123456789\n";
        let error = read_frame_until(
            &mut oversized,
            8,
            Instant::now() + Duration::from_millis(100),
        )
        .expect_err("oversized frame must fail");
        assert_eq!(frame_rejection_code(&error), "frame_too_large");

        let mut incomplete: &[u8] = b"partial";
        let error = read_frame_until(
            &mut incomplete,
            64,
            Instant::now() + Duration::from_millis(100),
        )
        .expect_err("unterminated frame must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(frame_rejection_code(&error), "malformed_frame");
    }

    struct WouldBlockReader;

    impl Read for WouldBlockReader {
        fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::WouldBlock))
        }
    }

    struct WouldBlockWriter;

    impl Write for WouldBlockWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::WouldBlock))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct SlowReader;

    impl Read for SlowReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            thread::sleep(Duration::from_millis(8));
            buffer[0] = b'x';
            Ok(1)
        }
    }

    struct SlowWriter;

    impl Write for SlowWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            thread::sleep(Duration::from_millis(8));
            Ok(1)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn stalled_ipc_read_and_write_return_at_the_deadline() {
        let deadline = Instant::now() + Duration::from_millis(20);
        let error = read_frame_until(&mut WouldBlockReader, 64, deadline)
            .expect_err("stalled reader must time out");
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);

        let deadline = Instant::now() + Duration::from_millis(20);
        let error = write_all_until(&mut WouldBlockWriter, b"request", deadline)
            .expect_err("stalled writer must time out");
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);

        let deadline = Instant::now() + Duration::from_millis(20);
        let error = read_frame_until(&mut SlowReader, 64, deadline)
            .expect_err("slow-drip reader must hit the total deadline");
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);

        let deadline = Instant::now() + Duration::from_millis(20);
        let error = write_all_until(&mut SlowWriter, b"request", deadline)
            .expect_err("slow-drip writer must hit the total deadline");
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn os_peer_credentials_report_the_connected_process_id() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        #[cfg(windows)]
        let name = std::ffi::OsString::from(format!("cryptex-ipc-peer-{}-{suffix}", process::id()))
            .to_ns_name::<GenericNamespaced>()
            .expect("test pipe name");
        #[cfg(unix)]
        let name = {
            std::env::temp_dir()
                .join(format!("cryptex-ipc-peer-{}-{suffix}.sock", process::id()))
                .to_fs_name::<interprocess::local_socket::GenericFilePath>()
                .expect("test socket name")
        };
        let listener = ListenerOptions::new()
            .name(name.borrow())
            .create_sync()
            .expect("test listener");
        let server = thread::spawn(move || {
            let peer = listener.accept().expect("accept test client");
            peer_process_id(&peer).expect("kernel peer process id")
        });
        let _client = Stream::connect(name.borrow()).expect("connect test client");
        assert_eq!(server.join().expect("peer test thread"), process::id());
    }

    #[test]
    fn desktop_process_authentication_is_bound_to_the_live_parent() {
        let identity = test_desktop_process_identity().expect("capture test parent process");
        assert!(identity.matches_peer(identity.pid));
        assert!(!identity.matches_peer(identity.pid.wrapping_add(1)));
    }
}
