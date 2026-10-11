//! Local daemon IPC. Health and guarded Desktop retrieval are the only exposed
//! operations; the daemon owns canonical verification and SQLite access.

use std::io;
#[cfg(any(not(windows), test))]
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError, sync_channel};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::canonical_writer::CanonicalWriterConfig;
use crate::capture::{
    CaptureOutcome, CaptureRejectCode, CaptureRequest, CaptureUncertainReason, capture_memory_note,
    resolve_capture,
};
use crate::compat::DAEMON_PROTOCOL_VERSION;
use crate::knowledge_materialization::{GuardedSearchError, RetrievalSafetyHold};
use crate::local_device_identity::LocalDeviceKeyResolver;
use crate::secure_store::{SecureStore, SecureStoreEnvironment};
use crate::{
    identity_runtime::load_brain_identity, local_device_identity::load_local_device_identity,
};
use cryptex_core::DeviceId;
use cryptex_events::Classification;
use cryptex_events::limits::MAX_MEMORY_NOTE_TEXT_BYTES;
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
const MAX_AUTHENTICATED_DESKTOP_CONNECTIONS: usize = 8;
// Full-history verification/search is bounded independently from the Desktop
// connection pool, leaving capacity for Desktop health/enrollment requests.
const MAX_DESKTOP_SEARCH_WORKERS: usize = 2;
/// One capture/resolve at a time so health/search keep listener capacity.
const MAX_DESKTOP_CAPTURE_WORKERS: usize = 1;
const CAPABILITY_CONTENT_CAPTURE: &str = "content.capture";
/// Capture responses are small (no note body); keep under the legacy frame budget.
const MAX_CAPTURE_RESPONSE_BYTES: usize = MAX_FRAME_BYTES;
/// Worst-case JSON string expansion for UTF-8 text (every byte as `\u00XX`).
const MAX_JSON_STRING_ESCAPE_FACTOR: usize = 6;
/// Envelope/metadata allowance around the escaped text field.
const MAX_CAPTURE_REQUEST_OVERHEAD_BYTES: usize = 4 * 1024;
/// Authenticated Desktop capture frames only. Health/search stay at MAX_FRAME_BYTES.
const MAX_CAPTURE_FRAME_BYTES: usize =
    MAX_MEMORY_NOTE_TEXT_BYTES * MAX_JSON_STRING_ESCAPE_FACTOR + MAX_CAPTURE_REQUEST_OVERHEAD_BYTES;
/// Capture may wait on the device lock; deadline returns Uncertain, not cancelled.
const IPC_CAPTURE_RESPONSE_DEADLINE: Duration = Duration::from_secs(60);
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    capture: Option<DesktopCaptureBody>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    capture_resolve: Option<DesktopCaptureResolveBody>,
}

/// Wire body for `memory.capture`. Raw text is bounded independently of frame size.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopCaptureBody {
    pub capture_id: String,
    pub text: String,
    pub classification: String,
}

/// Wire body for `memory.capture_resolve` (never appends).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopCaptureResolveBody {
    pub capture_id: String,
}

/// Redacted capture/resolve outcome for Desktop. Never includes note body or paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopCaptureResponse {
    pub protocol_version: u32,
    pub request_id: Option<String>,
    pub accepted: bool,
    pub error: Option<String>,
    pub operation: Option<String>,
    pub daemon_pid: u32,
    pub outcome: Option<String>,
    pub reject_code: Option<String>,
    pub uncertain_reason: Option<String>,
    pub capture_id: Option<String>,
    pub event_id: Option<String>,
    pub classification: Option<String>,
    pub created_at: Option<String>,
    pub content_hash: Option<String>,
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
    let mut context = SearchContext {
        runtime_root: runtime_root.to_path_buf(),
        store,
        environment,
        expected_desktop: Arc::new(expected_desktop),
        projection_sync: None,
    };
    start_projection_sync_worker(&mut context);
    spawn_server(Some(context))
}

/// Start the product IPC endpoint. Guarded retrieval is enabled only when the
/// daemon's current parent can be authenticated as the packaged Desktop. CLI,
/// test, and other launches retain the health-only endpoint instead of
/// failing daemon startup; sensitive operations remain unavailable without a
/// verified Desktop identity.
pub fn spawn_daemon_ipc_server(
    runtime_root: &Path,
    store: Arc<dyn SecureStore>,
    environment: SecureStoreEnvironment,
) -> io::Result<()> {
    let expected_desktop = match desktop_process_identity() {
        Ok(identity) => identity,
        Err(error) => {
            eprintln!(
                "cryptexd: Desktop peer identity unavailable ({}); serving health-only IPC",
                error.kind()
            );
            return spawn_server(None);
        }
    };
    let mut context = SearchContext {
        runtime_root: runtime_root.to_path_buf(),
        store,
        environment,
        expected_desktop: Arc::new(expected_desktop),
        projection_sync: None,
    };
    start_projection_sync_worker(&mut context);
    spawn_server(Some(context))
}

#[derive(Clone)]
struct SearchContext {
    runtime_root: PathBuf,
    store: Arc<dyn SecureStore>,
    environment: SecureStoreEnvironment,
    expected_desktop: Arc<DesktopProcessIdentity>,
    projection_sync: Option<SyncSender<()>>,
}

/// Run derived catch-up off the IPC request path. Canonical capture success is
/// independent of this rebuild; a failed catch-up leaves retrieval guarded and
/// can be retried on the next stale-search signal.
fn start_projection_sync_worker(context: &mut SearchContext) {
    let (sender, receiver) = sync_channel::<()>(1);
    let mut worker_context = context.clone();
    worker_context.projection_sync = None;
    let spawned = thread::Builder::new()
        .name("cryptex-derived-catchup".into())
        .spawn(move || {
            while receiver.recv().is_ok() {
                if let Err(error) = synchronize_local_derived_state(&worker_context) {
                    // Do not log paths, memory content, or identity material.
                    eprintln!(
                        "cryptexd: derived catch-up deferred ({error:?}); retrieval remains guarded"
                    );
                }
            }
        });
    if spawned.is_ok() {
        context.projection_sync = Some(sender.clone());
        let _ = sender.try_send(()); // Reconcile canonical commits discovered after restart.
    } else {
        eprintln!("cryptexd: derived catch-up worker unavailable; retrieval remains guarded");
    }
}

fn request_projection_sync(context: &SearchContext) {
    let Some(sender) = &context.projection_sync else {
        return;
    };
    match sender.try_send(()) {
        Ok(()) | Err(TrySendError::Full(())) => {}
        Err(TrySendError::Disconnected(())) => {
            eprintln!("cryptexd: derived catch-up worker unavailable; retrieval remains guarded");
        }
    }
}

#[derive(Debug)]
enum DerivedCatchupError {
    BrainIdentityUnavailable,
    LocalDeviceIdentityUnavailable,
    InvalidDeviceIdentity,
    SigningIdentityUnavailable,
    ProjectionDatabaseUnavailable,
    ProjectionCatchupFailed,
    KnowledgeRebuildFailed,
}

fn synchronize_local_derived_state(context: &SearchContext) -> Result<(), DerivedCatchupError> {
    let brain = load_brain_identity(&context.runtime_root)
        .map_err(|_| DerivedCatchupError::BrainIdentityUnavailable)?;
    let identity = load_local_device_identity(&context.runtime_root, &brain.brain_id)
        .map_err(|_| DerivedCatchupError::LocalDeviceIdentityUnavailable)?;
    let device = DeviceId::new(identity.device_id)
        .map_err(|_| DerivedCatchupError::InvalidDeviceIdentity)?;
    let mut resolver = LocalDeviceKeyResolver::load(
        &context.runtime_root,
        &brain.brain_id,
        context.store.as_ref(),
        context.environment,
    )
    .map_err(|_| DerivedCatchupError::SigningIdentityUnavailable)?;
    let (mut db, _) = crate::projection::open_projection_db(&context.runtime_root)
        .map_err(|_| DerivedCatchupError::ProjectionDatabaseUnavailable)?;
    crate::projection::catch_up_device_projection(
        context.store.as_ref(),
        context.environment,
        &context.runtime_root,
        &device,
        &db,
    )
    .map_err(|_| DerivedCatchupError::ProjectionCatchupFailed)?;
    crate::knowledge_materialization::rebuild_knowledge_from_canonical(
        &context.runtime_root,
        &brain.brain_id,
        std::slice::from_ref(&device),
        &mut resolver,
        &mut db,
    )
    .map_err(|_| DerivedCatchupError::KnowledgeRebuildFailed)?;
    Ok(())
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
    let desktop_connections = Arc::new(AtomicUsize::new(0));
    let desktop_search_workers = Arc::new(AtomicUsize::new(0));
    let desktop_capture_workers = Arc::new(AtomicUsize::new(0));
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
                        if is_desktop {
                            let Some(connection_reservation) = try_reserve_ipc_worker(
                                &desktop_connections,
                                MAX_AUTHENTICATED_DESKTOP_CONNECTIONS,
                            ) else {
                                // Keep the listener and health worker pools
                                // responsive when Desktop opens excess streams.
                                continue;
                            };
                            let worker_context = context.clone();
                            let search_workers = Arc::clone(&desktop_search_workers);
                            let capture_workers = Arc::clone(&desktop_capture_workers);
                            if let Err(error) = thread::Builder::new()
                                .name("cryptex-ipc-desktop".into())
                                .spawn(move || {
                                    let _reservation = connection_reservation;
                                    if let Err(error) = handle_connection(
                                        &mut stream,
                                        worker_context.as_deref(),
                                        Some(&search_workers),
                                        Some(&capture_workers),
                                    ) {
                                        eprintln!("cryptexd Desktop IPC failed: {error}");
                                    }
                                })
                            {
                                eprintln!("cryptexd IPC search worker failed to start: {error}");
                            }
                        } else {
                            let Some(reservation) = try_reserve_ipc_worker(
                                &unauthenticated_workers,
                                MAX_UNAUTHENTICATED_IPC_WORKERS,
                            ) else {
                                continue;
                            };
                            let worker_context = context.clone();
                            if let Err(error) = thread::Builder::new()
                                .name("cryptex-ipc-health".into())
                                .spawn(move || {
                                    let _reservation = reservation;
                                    if let Err(error) = handle_connection(
                                        &mut stream,
                                        worker_context.as_deref(),
                                        None,
                                        None,
                                    ) {
                                        eprintln!("cryptexd IPC connection failed: {error}");
                                    }
                                })
                            {
                                eprintln!("cryptexd IPC worker failed to start: {error}");
                            }
                        }
                    }
                    Err(error) => eprintln!("cryptexd IPC accept failed: {error}"),
                }
            }
        })?;
    Ok(())
}

fn try_reserve_ipc_worker(
    active: &Arc<AtomicUsize>,
    maximum: usize,
) -> Option<IpcWorkerReservation> {
    active
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
            (current < maximum).then_some(current + 1)
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
        capture: None,
        capture_resolve: None,
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
        capture: None,
        capture_resolve: None,
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
        capture: None,
        capture_resolve: None,
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
    arm_ipc_stream(stream)?;
    let mut request_frame = serde_json::to_vec(request)?;
    request_frame.push(b'\n');
    let deadline = Instant::now() + IPC_RESPONSE_DEADLINE;
    write_ipc_all(stream, &request_frame, deadline)?;
    read_ipc_frame(stream, max_response_bytes, deadline)
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

fn handle_connection(
    stream: &mut Stream,
    search: Option<&SearchContext>,
    desktop_search_workers: Option<&Arc<AtomicUsize>>,
    desktop_capture_workers: Option<&Arc<AtomicUsize>>,
) -> io::Result<()> {
    let peer_is_desktop = search
        .and_then(|context| {
            peer_process_id(stream)
                .ok()
                .map(|pid| context.expected_desktop.matches_peer(pid))
        })
        .unwrap_or(false);
    handle_connection_for_peer(
        stream,
        search,
        desktop_search_workers,
        desktop_capture_workers,
        peer_is_desktop,
    )
}

fn handle_connection_for_peer(
    stream: &mut Stream,
    search: Option<&SearchContext>,
    desktop_search_workers: Option<&Arc<AtomicUsize>>,
    desktop_capture_workers: Option<&Arc<AtomicUsize>>,
    peer_is_desktop: bool,
) -> io::Result<()> {
    arm_ipc_stream(stream)?;
    let request_deadline = Instant::now() + IPC_REQUEST_DEADLINE;
    // Authenticate peer before accepting a capture-sized frame.
    let max_frame = if peer_is_desktop {
        MAX_CAPTURE_FRAME_BYTES
    } else {
        MAX_FRAME_BYTES
    };
    let frame = match read_ipc_frame(stream, max_frame, request_deadline) {
        Ok(frame) => frame,
        Err(error) if error.kind() == io::ErrorKind::TimedOut => return Err(error),
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {
            let response = serde_json::to_vec(&rejected(None, frame_rejection_code(&error)))?;
            return write_response(stream, &response, Instant::now() + IPC_REQUEST_DEADLINE);
        }
        Err(error) => return Err(error),
    };
    let (response_bytes, write_deadline) = if frame.is_empty() {
        return Ok(());
    } else {
        match serde_json::from_slice::<Request>(&frame) {
            Ok(request) => {
                let write_deadline = response_write_deadline_for_operation(&request.operation);
                let is_capture_op = matches!(
                    request.operation.as_str(),
                    "memory.capture" | "memory.capture_resolve"
                );
                let bytes = if !is_capture_op && frame.len() > MAX_FRAME_BYTES {
                    serde_json::to_vec(&rejected(Some(request.request_id), "frame_too_large"))?
                } else if request.operation == "content.search" {
                    let response_deadline = Instant::now() + IPC_RESPONSE_DEADLINE;
                    let reservation = desktop_search_workers.and_then(|workers| {
                        try_reserve_ipc_worker(workers, MAX_DESKTOP_SEARCH_WORKERS)
                    });
                    let response = if desktop_search_workers.is_some() && reservation.is_none() {
                        search_rejected(Some(request.request_id), "search_busy")
                    } else {
                        run_search_until(
                            request,
                            search.cloned(),
                            peer_is_desktop,
                            response_deadline,
                            reservation,
                        )
                    };
                    let bytes = serde_json::to_vec(&response)?;
                    if bytes.len() + 1 > MAX_SEARCH_RESPONSE_BYTES {
                        serde_json::to_vec(&search_rejected(
                            response.request_id,
                            "response_too_large",
                        ))?
                    } else {
                        bytes
                    }
                } else if request.operation == "memory.capture"
                    || request.operation == "memory.capture_resolve"
                {
                    let response_deadline = Instant::now() + IPC_CAPTURE_RESPONSE_DEADLINE;
                    let reservation = desktop_capture_workers.and_then(|workers| {
                        try_reserve_ipc_worker(workers, MAX_DESKTOP_CAPTURE_WORKERS)
                    });
                    let response = if desktop_capture_workers.is_some() && reservation.is_none() {
                        capture_rejected(Some(request.request_id), "capture_busy")
                    } else {
                        run_capture_until(
                            request,
                            search.cloned(),
                            peer_is_desktop,
                            response_deadline,
                            reservation,
                        )
                    };
                    let bytes = serde_json::to_vec(&response)?;
                    if bytes.len() + 1 > MAX_CAPTURE_RESPONSE_BYTES {
                        serde_json::to_vec(&capture_rejected(
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
                };
                (bytes, write_deadline)
            }
            Err(_) => (
                serde_json::to_vec(&rejected(None, "malformed_request"))?,
                Instant::now() + IPC_RESPONSE_DEADLINE,
            ),
        }
    };
    write_response(stream, &response_bytes, write_deadline)
}

fn response_write_deadline_for_operation(operation: &str) -> Instant {
    let duration = match operation {
        "memory.capture" | "memory.capture_resolve" => IPC_CAPTURE_RESPONSE_DEADLINE,
        _ => IPC_RESPONSE_DEADLINE,
    };
    Instant::now() + duration
}

fn run_search_until(
    request: Request,
    context: Option<SearchContext>,
    peer_is_desktop: bool,
    deadline: Instant,
    reservation: Option<IpcWorkerReservation>,
) -> DesktopSearchResponse {
    let request_id = request.request_id.clone();
    run_bounded_search(request_id, deadline, reservation, move || {
        handle_search_until(request, context.as_ref(), peer_is_desktop, Some(deadline))
    })
}

fn run_bounded_search<F>(
    request_id: String,
    deadline: Instant,
    reservation: Option<IpcWorkerReservation>,
    work: F,
) -> DesktopSearchResponse
where
    F: FnOnce() -> DesktopSearchResponse + Send + 'static,
{
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let spawned = thread::Builder::new()
        .name("cryptex-ipc-search".into())
        .spawn(move || {
            // A timed-out synchronous OS/SQLite operation may still be unwinding.
            // Keep its slot occupied until it actually exits, bounding orphaned work.
            let _reservation = reservation;
            let response = work();
            let _ = sender.send(response);
        });
    if spawned.is_err() {
        return search_held(Some(request_id), "search_worker_unavailable");
    }
    match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(response) if Instant::now() <= deadline => response,
        Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            search_held(Some(request_id), "operation_deadline_exceeded")
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            search_held(Some(request_id), "canonical_verification_unavailable")
        }
    }
}

fn write_response(stream: &mut Stream, response: &[u8], deadline: Instant) -> io::Result<()> {
    let mut frame = Vec::with_capacity(response.len() + 1);
    frame.extend_from_slice(response);
    frame.push(b'\n');
    write_ipc_all(stream, &frame, deadline)
}

fn frame_rejection_code(error: &io::Error) -> &'static str {
    if error.to_string() == "IPC frame exceeds limit" {
        "frame_too_large"
    } else {
        "malformed_frame"
    }
}

#[cfg(any(not(windows), test))]
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

fn arm_ipc_stream(stream: &Stream) -> io::Result<()> {
    #[cfg(not(windows))]
    {
        stream.set_nonblocking(true)?;
    }
    #[cfg(windows)]
    {
        // interprocess enables PIPE_NOWAIT for nonblocking named pipes. Its
        // reader then turns the resulting "no data yet" error into a
        // zero-length read, which this protocol treats as end-of-stream.
        // Windows framing waits with overlapped I/O and cancels it at the
        // deadline instead.
        let _ = stream;
    }
    Ok(())
}

fn read_ipc_frame(stream: &mut Stream, max_bytes: usize, deadline: Instant) -> io::Result<Vec<u8>> {
    #[cfg(windows)]
    {
        read_windows_pipe_frame(stream, max_bytes, deadline)
    }
    #[cfg(not(windows))]
    {
        read_frame_until(stream, max_bytes, deadline)
    }
}

fn write_ipc_all(stream: &mut Stream, bytes: &[u8], deadline: Instant) -> io::Result<()> {
    #[cfg(windows)]
    {
        write_windows_pipe_all(stream, bytes, deadline)
    }
    #[cfg(not(windows))]
    {
        write_all_until(stream, bytes, deadline)
    }
}

#[cfg(windows)]
fn frame_timeout() -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, "IPC frame deadline exceeded")
}

#[cfg(windows)]
fn read_windows_pipe_frame(
    stream: &Stream,
    max_bytes: usize,
    deadline: Instant,
) -> io::Result<Vec<u8>> {
    let mut frame = Vec::with_capacity(1024);
    let mut chunk = [0_u8; 4096];
    loop {
        if Instant::now() >= deadline {
            return Err(frame_timeout());
        }
        match transfer_named_pipe(stream, None, Some(&mut chunk), deadline) {
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
            Err(error) => return Err(error),
        }
    }
}

#[cfg(windows)]
fn write_windows_pipe_all(stream: &Stream, mut bytes: &[u8], deadline: Instant) -> io::Result<()> {
    while !bytes.is_empty() {
        if Instant::now() >= deadline {
            return Err(frame_timeout());
        }
        let written = transfer_named_pipe(stream, Some(bytes), None, deadline)?;
        if written == 0 {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "IPC stream closed",
            ));
        }
        bytes = &bytes[written..];
    }
    Ok(())
}

#[cfg(windows)]
fn transfer_named_pipe(
    stream: &Stream,
    write: Option<&[u8]>,
    read: Option<&mut [u8]>,
    deadline: Instant,
) -> io::Result<usize> {
    use windows_sys::Win32::System::Threading::WaitForSingleObject;

    transfer_named_pipe_with_wait(stream, write, read, deadline, |event, millis| unsafe {
        WaitForSingleObject(event, millis)
    })
}

#[cfg(windows)]
fn transfer_named_pipe_with_wait(
    stream: &Stream,
    write: Option<&[u8]>,
    read: Option<&mut [u8]>,
    deadline: Instant,
    wait_for_event: impl FnOnce(windows_sys::Win32::Foundation::HANDLE, u32) -> u32,
) -> io::Result<usize> {
    use std::os::windows::io::{AsHandle, AsRawHandle};
    use windows_sys::Win32::{
        Foundation::{ERROR_IO_INCOMPLETE, ERROR_IO_PENDING, WAIT_OBJECT_0, WAIT_TIMEOUT},
        Storage::FileSystem::{ReadFile, WriteFile},
        System::{IO::OVERLAPPED, Threading::CreateEventW},
    };

    if Instant::now() >= deadline {
        return Err(frame_timeout());
    }
    let Stream::NamedPipe(pipe) = stream;
    let handle = pipe.as_handle().as_raw_handle();
    let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
    if event.is_null() {
        return Err(io::Error::last_os_error());
    }
    let _event_guard = EventGuard(event);

    let reading = read.is_some();
    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    overlapped.hEvent = event;
    let started = match (write, read) {
        (Some(bytes), None) => {
            let len = u32::try_from(bytes.len())
                .map_err(|_| invalid_data("IPC transfer is too large"))?;
            unsafe {
                WriteFile(
                    handle,
                    bytes.as_ptr().cast(),
                    len,
                    std::ptr::null_mut(),
                    &mut overlapped,
                )
            }
        }
        (None, Some(bytes)) => {
            let len = u32::try_from(bytes.len())
                .map_err(|_| invalid_data("IPC transfer is too large"))?;
            unsafe {
                ReadFile(
                    handle,
                    bytes.as_mut_ptr().cast(),
                    len,
                    std::ptr::null_mut(),
                    &mut overlapped,
                )
            }
        }
        _ => return Err(invalid_data("IPC transfer direction is invalid")),
    };
    if started == 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(ERROR_IO_PENDING as i32) {
            return map_pipe_read_end(error, reading);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        let millis = u32::try_from(remaining.as_millis()).unwrap_or(u32::MAX - 1);
        let wait = wait_for_event(event, millis);
        if wait != WAIT_OBJECT_0 {
            let wait_error = if wait == WAIT_TIMEOUT {
                frame_timeout()
            } else {
                io::Error::last_os_error()
            };
            let cleanup_errors = cancel_and_drain_named_pipe(handle, &overlapped);
            return Err(with_pipe_cleanup_details(wait_error, cleanup_errors));
        }
    }
    let transfer = overlapped_byte_count(handle, &overlapped, false);
    let transfer = match transfer {
        Err(error) if error.raw_os_error() == Some(ERROR_IO_INCOMPLETE as i32) => {
            let cleanup_errors = cancel_and_drain_named_pipe(handle, &overlapped);
            Err(with_pipe_cleanup_details(error, cleanup_errors))
        }
        other => other,
    };
    let transfer = transfer.or_else(|error| map_pipe_read_end(error, reading));
    match transfer {
        Ok(_) if Instant::now() >= deadline => Err(frame_timeout()),
        other => other,
    }
}

#[cfg(windows)]
fn cancel_and_drain_named_pipe(
    handle: windows_sys::Win32::Foundation::HANDLE,
    overlapped: *const windows_sys::Win32::System::IO::OVERLAPPED,
) -> Vec<String> {
    use windows_sys::Win32::Foundation::{
        ERROR_IO_INCOMPLETE, ERROR_NOT_FOUND, ERROR_OPERATION_ABORTED,
    };
    use windows_sys::Win32::System::IO::CancelIoEx;

    let cancelled = unsafe { CancelIoEx(handle, overlapped) };
    let cancel_error = (cancelled == 0).then(io::Error::last_os_error);

    // The OVERLAPPED value, event, and caller buffer must outlive completion even
    // when the wait itself fails. A blocking completion query is the drain
    // boundary; ERROR_IO_INCOMPLETE is retried defensively because returning in
    // that state would let the kernel retain pointers into this stack frame.
    let completion = loop {
        match overlapped_byte_count(handle, overlapped, true) {
            Err(error) if error.raw_os_error() == Some(ERROR_IO_INCOMPLETE as i32) => {
                std::thread::yield_now();
            }
            result => break result,
        }
    };

    let mut errors = Vec::new();
    if let Some(error) =
        cancel_error.filter(|error| error.raw_os_error() != Some(ERROR_NOT_FOUND as i32))
    {
        errors.push(format!("CancelIoEx failed: {error}"));
    }
    if let Err(error) = completion
        && error.raw_os_error() != Some(ERROR_OPERATION_ABORTED as i32)
    {
        errors.push(format!("overlapped completion failed: {error}"));
    }
    errors
}

#[cfg(windows)]
fn with_pipe_cleanup_details(primary: io::Error, cleanup_errors: Vec<String>) -> io::Error {
    if cleanup_errors.is_empty() {
        primary
    } else {
        io::Error::new(
            primary.kind(),
            format!(
                "{primary}; overlapped I/O cleanup: {}",
                cleanup_errors.join("; ")
            ),
        )
    }
}

#[cfg(windows)]
fn overlapped_byte_count(
    handle: windows_sys::Win32::Foundation::HANDLE,
    overlapped: *const windows_sys::Win32::System::IO::OVERLAPPED,
    wait: bool,
) -> io::Result<usize> {
    use windows_sys::Win32::System::IO::GetOverlappedResult;

    let mut transferred = 0_u32;
    let completed =
        unsafe { GetOverlappedResult(handle, overlapped, &mut transferred, i32::from(wait)) };
    if completed == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(usize::try_from(transferred).unwrap_or(usize::MAX))
    }
}

#[cfg(windows)]
fn map_pipe_read_end(error: io::Error, reading: bool) -> io::Result<usize> {
    use windows_sys::Win32::Foundation::{ERROR_BROKEN_PIPE, ERROR_PIPE_NOT_CONNECTED};

    const BROKEN_PIPE: i32 = ERROR_BROKEN_PIPE as i32;
    const NOT_CONNECTED: i32 = ERROR_PIPE_NOT_CONNECTED as i32;
    if reading
        && matches!(
            error.raw_os_error(),
            Some(BROKEN_PIPE) | Some(NOT_CONNECTED)
        )
    {
        Ok(0)
    } else {
        Err(error)
    }
}

#[cfg(windows)]
struct EventGuard(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for EventGuard {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::CloseHandle;

        unsafe { CloseHandle(self.0) };
    }
}

#[cfg(any(not(windows), test))]
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

#[cfg(any(not(windows), test))]
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
            match stream {
                Stream::UdSocket(unix_stream) => unix_stream.inner().as_raw_fd(),
            },
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
    } else if request.query.is_some()
        || request.capture.is_some()
        || request.capture_resolve.is_some()
    {
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

#[cfg(test)]
fn handle_search(
    request: Request,
    context: Option<&SearchContext>,
    peer_is_desktop: bool,
) -> DesktopSearchResponse {
    handle_search_until(request, context, peer_is_desktop, None)
}

fn handle_search_until(
    request: Request,
    context: Option<&SearchContext>,
    peer_is_desktop: bool,
    deadline: Option<Instant>,
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
    } else if request.capture.is_some() || request.capture_resolve.is_some() {
        Some("unexpected_capture_body")
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
    if deadline_expired(deadline) {
        return search_held(Some(request_id.clone()), "operation_deadline_exceeded");
    }
    let brain = match load_brain_identity(&context.runtime_root) {
        Ok(brain) => brain,
        Err(_) => return search_held(Some(request_id), "canonical_verification_unavailable"),
    };
    if deadline_expired(deadline) {
        return search_held(Some(request_id.clone()), "operation_deadline_exceeded");
    }
    let local_identity = match load_local_device_identity(&context.runtime_root, &brain.brain_id) {
        Ok(identity) => identity,
        Err(crate::local_device_identity::LocalDeviceIdentityError::EnrollmentRequired) => {
            return search_held(Some(request_id), "local_enrollment_required");
        }
        Err(_) => return search_held(Some(request_id), "canonical_verification_unavailable"),
    };
    if deadline_expired(deadline) {
        return search_held(Some(request_id.clone()), "operation_deadline_exceeded");
    }
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
    if deadline_expired(deadline) {
        return search_held(Some(request_id.clone()), "operation_deadline_exceeded");
    }
    let db = match ProjectionDb::open_or_create(&context.runtime_root) {
        Ok(db) => db,
        Err(_) => {
            request_projection_sync(context);
            return search_held(Some(request_id), "materialization_state_missing_or_stale");
        }
    };
    if deadline_expired(deadline) {
        return search_held(Some(request_id.clone()), "operation_deadline_exceeded");
    }
    match crate::knowledge_materialization::search_events_with_retrieval_safety_deadline(
        &context.runtime_root,
        &brain.brain_id,
        &[device],
        &mut resolver,
        &db,
        &query,
        deadline,
    ) {
        Ok(hits) => search_results(Some(request_id), hits),
        Err(GuardedSearchError::BrainWideHold(reason)) => {
            if matches!(
                reason,
                RetrievalSafetyHold::MaterializationStateMissingOrStale
                    | RetrievalSafetyHold::ProjectionDoesNotMatchCanonical
            ) {
                drop(db);
                request_projection_sync(context);
            }
            search_held(Some(request_id), hold_reason_code(reason))
        }
        Err(GuardedSearchError::Query(_)) => search_rejected(Some(request_id), "invalid_query"),
    }
}

fn deadline_expired(deadline: Option<Instant>) -> bool {
    deadline.is_some_and(|deadline| Instant::now() >= deadline)
}

fn hold_reason_code(reason: RetrievalSafetyHold) -> &'static str {
    match reason {
        RetrievalSafetyHold::CanonicalVerificationUnavailable => {
            "canonical_verification_unavailable"
        }
        RetrievalSafetyHold::OperationDeadlineExceeded => "operation_deadline_exceeded",
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
    } else if request.operation != "health.ping"
        || request.query.is_some()
        || request.capture.is_some()
        || request.capture_resolve.is_some()
    {
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

fn run_capture_until(
    request: Request,
    context: Option<SearchContext>,
    peer_is_desktop: bool,
    deadline: Instant,
    reservation: Option<IpcWorkerReservation>,
) -> DesktopCaptureResponse {
    let request_id = request.request_id.clone();
    run_bounded_capture(request_id, deadline, reservation, move || {
        handle_capture_until(request, context.as_ref(), peer_is_desktop)
    })
}

fn run_bounded_capture<F>(
    request_id: String,
    deadline: Instant,
    reservation: Option<IpcWorkerReservation>,
    work: F,
) -> DesktopCaptureResponse
where
    F: FnOnce() -> DesktopCaptureResponse + Send + 'static,
{
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let spawned = thread::Builder::new()
        .name("cryptex-ipc-capture".into())
        .spawn(move || {
            // Timed-out capture work may still hold the device lock until exit.
            // Keep the worker slot occupied; do not claim the commit was cancelled.
            let _reservation = reservation;
            let response = work();
            let _ = sender.send(response);
        });
    if spawned.is_err() {
        return capture_rejected(Some(request_id), "capture_worker_unavailable");
    }
    match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(response) if Instant::now() <= deadline => response,
        Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            // Uncertain: work may still be committing. Not a cancel guarantee.
            capture_uncertain(Some(request_id), "operation_deadline_exceeded")
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            capture_uncertain(Some(request_id), "capture_worker_disconnected")
        }
    }
}

fn handle_capture_until(
    request: Request,
    context: Option<&SearchContext>,
    peer_is_desktop: bool,
) -> DesktopCaptureResponse {
    let request_id = request.request_id.clone();
    let is_resolve = request.operation == "memory.capture_resolve";
    let authorization_error = if request.protocol_version != PROTOCOL_VERSION {
        Some("incompatible_protocol_version")
    } else if request.request_id.is_empty() || request.request_id.len() > 128 {
        Some("invalid_request_id")
    } else if request.client_role != "desktop" {
        Some("unauthorized_client_role")
    } else if !peer_is_desktop {
        Some("unauthorized_client_process")
    } else if request.capabilities.as_slice() != [CAPABILITY_CONTENT_CAPTURE] {
        Some("unauthorized_capability")
    } else if request.query.is_some() {
        Some("unexpected_search_query")
    } else if is_resolve {
        if request.capture.is_some() {
            Some("unexpected_capture_body")
        } else if request.capture_resolve.is_none() {
            Some("missing_capture_resolve")
        } else {
            None
        }
    } else if request.operation == "memory.capture" {
        if request.capture_resolve.is_some() {
            Some("unexpected_capture_resolve")
        } else if request.capture.is_none() {
            Some("missing_capture_body")
        } else {
            None
        }
    } else {
        Some("unsupported_operation")
    };
    if let Some(error) = authorization_error {
        return capture_rejected(Some(request_id), error);
    }
    let Some(context) = context else {
        return capture_rejected(Some(request_id), "capture_unavailable");
    };

    let outcome = if is_resolve {
        let body = request.capture_resolve.expect("checked");
        if let Err(code) = validate_capture_id(&body.capture_id) {
            return capture_rejected(Some(request_id), code);
        }
        resolve_capture(
            context.store.as_ref(),
            context.environment,
            &context.runtime_root,
            &body.capture_id,
        )
    } else {
        let body = request.capture.expect("checked");
        match validate_capture_body(&body) {
            Ok(req) => capture_memory_note(
                context.store.as_ref(),
                context.environment,
                &context.runtime_root,
                req,
                &CanonicalWriterConfig::default(),
            ),
            Err(code) => return capture_rejected(Some(request_id), code),
        }
    };
    if matches!(&outcome, CaptureOutcome::Committed(_)) {
        request_projection_sync(context);
    }
    map_capture_outcome(
        request_id,
        if is_resolve {
            "memory.capture_resolve.result"
        } else {
            "memory.capture.result"
        },
        outcome,
    )
}

fn validate_capture_id(capture_id: &str) -> Result<(), &'static str> {
    if capture_id.len() != 36 {
        return Err("invalid_capture_id");
    }
    // UUID shape only; service rejects non-UUID via capture layer too.
    let ok = capture_id
        .as_bytes()
        .iter()
        .enumerate()
        .all(|(i, b)| match i {
            8 | 13 | 18 | 23 => *b == b'-',
            _ => b.is_ascii_hexdigit(),
        });
    if !ok {
        return Err("invalid_capture_id");
    }
    Ok(())
}

fn parse_wire_classification(value: &str) -> Result<Classification, &'static str> {
    match value {
        "public" => Ok(Classification::Public),
        "internal" => Ok(Classification::Internal),
        "private" => Ok(Classification::Private),
        "restricted" => Ok(Classification::Restricted),
        _ => Err("invalid_classification"),
    }
}

fn validate_capture_body(body: &DesktopCaptureBody) -> Result<CaptureRequest, &'static str> {
    validate_capture_id(&body.capture_id)?;
    if body.text.len() > MAX_MEMORY_NOTE_TEXT_BYTES {
        return Err("capture_text_too_large");
    }
    // Reject non-UTF-8 is impossible for String; reject NULs as untrusted control.
    if body.text.as_bytes().contains(&0) {
        return Err("invalid_capture_text");
    }
    let classification = parse_wire_classification(&body.classification)?;
    Ok(CaptureRequest {
        capture_id: body.capture_id.clone(),
        text: body.text.clone(),
        classification,
    })
}

fn map_capture_outcome(
    request_id: String,
    operation: &str,
    outcome: CaptureOutcome,
) -> DesktopCaptureResponse {
    match outcome {
        CaptureOutcome::Committed(commit) => DesktopCaptureResponse {
            protocol_version: PROTOCOL_VERSION,
            request_id: Some(request_id),
            accepted: true,
            error: None,
            operation: Some(operation.into()),
            daemon_pid: process::id(),
            outcome: Some("committed".into()),
            reject_code: None,
            uncertain_reason: None,
            capture_id: Some(commit.capture_id),
            event_id: Some(commit.event_id),
            classification: Some(commit.classification.as_str().into()),
            created_at: Some(commit.created_at),
            content_hash: Some(commit.content_hash),
        },
        CaptureOutcome::Rejected { code, .. } => DesktopCaptureResponse {
            protocol_version: PROTOCOL_VERSION,
            request_id: Some(request_id),
            accepted: true,
            error: None,
            operation: Some(operation.into()),
            daemon_pid: process::id(),
            outcome: Some("rejected".into()),
            reject_code: Some(reject_code_wire(code).into()),
            uncertain_reason: None,
            capture_id: None,
            event_id: None,
            classification: None,
            created_at: None,
            content_hash: None,
        },
        CaptureOutcome::Uncertain { reason, .. } => DesktopCaptureResponse {
            protocol_version: PROTOCOL_VERSION,
            request_id: Some(request_id),
            accepted: true,
            error: None,
            operation: Some(operation.into()),
            daemon_pid: process::id(),
            outcome: Some("uncertain".into()),
            reject_code: None,
            uncertain_reason: Some(uncertain_reason_wire(reason).into()),
            capture_id: None,
            event_id: None,
            classification: None,
            created_at: None,
            content_hash: None,
        },
    }
}

fn reject_code_wire(code: CaptureRejectCode) -> &'static str {
    match code {
        CaptureRejectCode::InvalidRequest => "invalid_request",
        CaptureRejectCode::EnrollmentRequired => "enrollment_required",
        CaptureRejectCode::IdentityUnavailable => "identity_unavailable",
        CaptureRejectCode::CaptureIdConflict => "capture_id_conflict",
        CaptureRejectCode::NotCommitted => "not_committed",
    }
}

fn uncertain_reason_wire(reason: CaptureUncertainReason) -> &'static str {
    match reason {
        CaptureUncertainReason::WriterFailure => "writer_failure",
        CaptureUncertainReason::DurabilityUnconfirmed => "durability_unconfirmed",
        CaptureUncertainReason::ReceiptMissing => "receipt_missing",
        CaptureUncertainReason::ReceiptPersistFailed => "receipt_persist_failed",
        CaptureUncertainReason::VerificationUnavailable => "verification_unavailable",
        CaptureUncertainReason::FingerprintMismatch => "fingerprint_mismatch",
        CaptureUncertainReason::LockFailed => "lock_failed",
    }
}

fn capture_rejected(request_id: Option<String>, error: &str) -> DesktopCaptureResponse {
    DesktopCaptureResponse {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        accepted: false,
        error: Some(error.into()),
        operation: None,
        daemon_pid: process::id(),
        outcome: None,
        reject_code: None,
        uncertain_reason: None,
        capture_id: None,
        event_id: None,
        classification: None,
        created_at: None,
        content_hash: None,
    }
}

fn capture_uncertain(request_id: Option<String>, reason: &str) -> DesktopCaptureResponse {
    DesktopCaptureResponse {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        accepted: true,
        error: None,
        operation: Some("memory.capture.result".into()),
        daemon_pid: process::id(),
        outcome: Some("uncertain".into()),
        reject_code: None,
        uncertain_reason: Some(reason.into()),
        capture_id: None,
        event_id: None,
        classification: None,
        created_at: None,
        content_hash: None,
    }
}

/// Desktop client: submit a capture. Transport errors are uncertain to the UI.
pub fn desktop_capture(
    body: DesktopCaptureBody,
    timeout: Duration,
) -> io::Result<DesktopCaptureResponse> {
    desktop_capture_exchange(
        "memory.capture",
        Some(body),
        None,
        timeout,
        "memory.capture.result",
    )
}

/// Desktop client: resolve a prior capture without appending.
pub fn desktop_capture_resolve(
    body: DesktopCaptureResolveBody,
    timeout: Duration,
) -> io::Result<DesktopCaptureResponse> {
    desktop_capture_exchange(
        "memory.capture_resolve",
        None,
        Some(body),
        timeout,
        "memory.capture_resolve.result",
    )
}

fn desktop_capture_exchange(
    operation: &str,
    capture: Option<DesktopCaptureBody>,
    capture_resolve: Option<DesktopCaptureResolveBody>,
    timeout: Duration,
    expected_operation: &str,
) -> io::Result<DesktopCaptureResponse> {
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
        capabilities: vec![CAPABILITY_CONTENT_CAPTURE.into()],
        operation: operation.into(),
        query: None,
        capture,
        capture_resolve,
    };
    let mut request_frame = serde_json::to_vec(&request)?;
    request_frame.push(b'\n');
    if request_frame.len() > MAX_CAPTURE_FRAME_BYTES {
        return Err(invalid_data("capture request exceeds capture frame bound"));
    }
    arm_ipc_stream(&stream)?;
    let deadline = Instant::now() + IPC_CAPTURE_RESPONSE_DEADLINE;
    write_ipc_all(&mut stream, &request_frame, deadline)?;
    let frame = read_ipc_frame(&mut stream, MAX_CAPTURE_RESPONSE_BYTES, deadline)?;
    let response: DesktopCaptureResponse = serde_json::from_slice(&frame)?;
    if response.protocol_version != PROTOCOL_VERSION
        || response.request_id.as_deref() != Some(&request_id)
    {
        return Err(invalid_data("IPC capture response did not match request"));
    }
    if response.accepted
        && response.outcome.is_some()
        && response.operation.as_deref() != Some(expected_operation)
    {
        return Err(invalid_data("IPC capture response operation mismatch"));
    }
    let _ = expected_operation;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use cryptex_core::BrainIdentity;
    use cryptex_events::Classification;
    use cryptex_storage::layout::{active_segment_path, canonical_events_device_dir};

    use super::*;
    use crate::canonical_writer::{
        AppendMemoryNoteArgs, CanonicalWriterConfig, append_memory_note,
    };
    use crate::identity_runtime::write_brain_identity;
    use crate::knowledge_materialization::rebuild_knowledge_from_canonical;
    use crate::local_device_identity::enroll_local_device;
    use crate::projection::rebuild_device_projection;
    use crate::secure_store::FakeSecureStore;
    use crate::test_temp::exclusive_temp_dir;
    use cryptex_projection::ProjectionDb;

    const TEST_BRAIN_ID: &str = "11111111-1111-4111-8111-111111111111";

    fn search_request(role: &str, capabilities: Vec<String>) -> Request {
        Request {
            protocol_version: PROTOCOL_VERSION,
            request_id: "test-request".into(),
            client_role: role.into(),
            capabilities,
            operation: "content.search".into(),
            query: Some(DesktopSearchQuery::default()),
            capture: None,
            capture_resolve: None,
        }
    }

    struct SearchFixture {
        root: PathBuf,
        brain_id: String,
        device: DeviceId,
        store: Arc<FakeSecureStore>,
        context: SearchContext,
    }

    fn prepared_search_fixture(label: &str) -> SearchFixture {
        let root = exclusive_temp_dir(&format!("cryptex-ipc-search-{label}"));
        let brain = BrainIdentity::new(TEST_BRAIN_ID, "2026-10-08T00:00:00Z").expect("Brain");
        write_brain_identity(&root, &brain).expect("write Brain identity");
        let store = Arc::new(FakeSecureStore::new());
        let identity = enroll_local_device(&root, store.as_ref(), SecureStoreEnvironment::Dev)
            .expect("enroll local device");
        let device = DeviceId::new(identity.device_id).expect("device id");
        append_memory_note(
            store.as_ref(),
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &root,
                brain_id: TEST_BRAIN_ID,
                device_id: &device,
                classification: Classification::Internal,
                text: "synthetic IPC search fixture note",
                event_id: None,
                config: &CanonicalWriterConfig::default(),
            },
        )
        .expect("append synthetic note");

        let mut resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            &root,
            TEST_BRAIN_ID,
            store.as_ref(),
            SecureStoreEnvironment::Dev,
        )
        .expect("load local event key");
        let mut db = ProjectionDb::open_or_create(&root).expect("open projection database");
        rebuild_device_projection(
            store.as_ref(),
            SecureStoreEnvironment::Dev,
            &root,
            &device,
            &db,
        )
        .expect("rebuild M3 projection");
        rebuild_knowledge_from_canonical(
            &root,
            TEST_BRAIN_ID,
            std::slice::from_ref(&device),
            &mut resolver,
            &mut db,
        )
        .expect("rebuild M4 knowledge state");
        drop(db);

        let context = SearchContext {
            runtime_root: root.clone(),
            store: store.clone(),
            environment: SecureStoreEnvironment::Dev,
            expected_desktop: Arc::new(
                test_desktop_process_identity().expect("capture test Desktop identity"),
            ),
            projection_sync: None,
        };
        SearchFixture {
            root,
            brain_id: TEST_BRAIN_ID.into(),
            device,
            store,
            context,
        }
    }

    #[test]
    fn derived_state_rebuilds_from_canonical_after_projection_database_loss() {
        let mut fixture = prepared_search_fixture("derived-restart-rebuild");
        let (sync_sender, sync_receiver) = sync_channel(1);
        fixture.context.projection_sync = Some(sync_sender);
        let projection_path = cryptex_projection::projection_sqlite_path(&fixture.root);
        fs::remove_file(&projection_path).expect("remove derived projection database");

        let held = content_search_ipc_round_trip(
            Some(fixture.context.clone()),
            DesktopSearchQuery::default(),
        );
        assert!(held.accepted);
        assert!(held.hits.is_none());
        assert_eq!(
            held.hold_reason.as_deref(),
            Some("materialization_state_missing_or_stale")
        );
        sync_receiver
            .try_recv()
            .expect("stale guarded search schedules derived catch-up");

        synchronize_local_derived_state(&fixture.context)
            .expect("rebuild M3 and M4 derived state from canonical events");

        let response = content_search_ipc_round_trip(
            Some(fixture.context.clone()),
            DesktopSearchQuery {
                text: Some("synthetic IPC search fixture note".into()),
                ..DesktopSearchQuery::default()
            },
        );
        assert!(response.accepted);
        let hits = response.hits.expect("rebuilt derived note is searchable");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].text, "synthetic IPC search fixture note");
        fs::remove_dir_all(fixture.root).expect("cleanup fixture");
    }

    #[test]
    fn daemon_startup_worker_reconstructs_missing_derived_state() {
        let fixture = prepared_search_fixture("startup-catchup");
        let projection_path = cryptex_projection::projection_sqlite_path(&fixture.root);
        fs::remove_file(&projection_path).expect("remove derived projection database");
        let mut context = fixture.context.clone();
        start_projection_sync_worker(&mut context);

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let response = content_search_ipc_round_trip(
                Some(context.clone()),
                DesktopSearchQuery {
                    text: Some("synthetic IPC search fixture note".into()),
                    ..DesktopSearchQuery::default()
                },
            );
            assert!(response.accepted);
            if let Some(hits) = response.hits {
                assert_eq!(hits.len(), 1);
                assert_eq!(hits[0].text, "synthetic IPC search fixture note");
                break;
            }
            assert!(
                Instant::now() < deadline,
                "startup catch-up did not make canonical note searchable"
            );
            thread::sleep(Duration::from_millis(20));
        }

        drop(context);
        fs::remove_dir_all(fixture.root).expect("cleanup fixture");
    }

    fn content_search_ipc_round_trip(
        context: Option<SearchContext>,
        query: DesktopSearchQuery,
    ) -> DesktopSearchResponse {
        let (name, listener) = isolated_ipc_listener("content-search");
        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("accept content search client");
            handle_connection_for_peer(&mut stream, context.as_ref(), None, None, true)
                .expect("serve content search request");
        });
        let mut client = Stream::connect(name.borrow()).expect("connect content search client");
        let deadline = Instant::now() + Duration::from_secs(20);
        let request = Request {
            query: Some(query),
            ..search_request("desktop", vec![CAPABILITY_CONTENT_SEARCH.into()])
        };
        let mut frame = serde_json::to_vec(&request).expect("encode request");
        frame.push(b'\n');
        write_ipc_all(&mut client, &frame, deadline).expect("write search request");
        let response_frame = read_ipc_frame(&mut client, MAX_SEARCH_RESPONSE_BYTES, deadline)
            .expect("read response");
        let response = serde_json::from_slice(&response_frame).expect("decode search response");
        server.join().expect("search server thread");
        response
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
            expected_desktop: Arc::new(
                test_desktop_process_identity().expect("desktop parent identity"),
            ),
            projection_sync: None,
        };
        let response = content_search_ipc_round_trip(Some(context), DesktopSearchQuery::default());

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
            expected_desktop: Arc::new(
                test_desktop_process_identity().expect("desktop parent identity"),
            ),
            projection_sync: None,
        };
        let response = content_search_ipc_round_trip(Some(context), DesktopSearchQuery::default());

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
    fn desktop_content_search_ipc_returns_a_verified_m3_hit_when_safety_allows() {
        let fixture = prepared_search_fixture("allowed");
        let response = content_search_ipc_round_trip(
            Some(fixture.context.clone()),
            DesktopSearchQuery {
                text: Some("synthetic IPC search fixture".into()),
                ..DesktopSearchQuery::default()
            },
        );

        assert!(response.accepted, "unexpected rejection: {response:?}");
        assert_eq!(response.operation.as_deref(), Some("content.search.result"));
        assert!(response.hold_reason.is_none());
        let hits = response.hits.expect("allowed response contains hits");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].device_id, fixture.device.as_str());
        assert_eq!(hits[0].brain_id, fixture.brain_id);
        assert_eq!(hits[0].text, "synthetic IPC search fixture note");

        fs::remove_dir_all(fixture.root).expect("cleanup");
    }

    #[test]
    fn desktop_content_search_ipc_holds_when_knowledge_state_is_missing_or_corrupt() {
        let fixture = prepared_search_fixture("missing-knowledge-state");
        let db = ProjectionDb::open_or_create(&fixture.root).expect("open projection db");
        db.connection()
            .execute_batch("DROP TABLE knowledge_meta;")
            .expect("remove M4 materialization metadata");
        drop(db);

        let missing = content_search_ipc_round_trip(
            Some(fixture.context.clone()),
            DesktopSearchQuery::default(),
        );
        assert!(missing.accepted);
        assert!(missing.hits.is_none());
        assert_eq!(
            missing.hold_reason.as_deref(),
            Some("materialization_state_missing_or_stale")
        );

        let mut db = ProjectionDb::open_or_create(&fixture.root).expect("reopen projection db");
        rebuild_knowledge_from_canonical(
            &fixture.root,
            &fixture.brain_id,
            std::slice::from_ref(&fixture.device),
            &mut crate::local_device_identity::LocalDeviceKeyResolver::load(
                &fixture.root,
                &fixture.brain_id,
                fixture.store.as_ref(),
                SecureStoreEnvironment::Dev,
            )
            .expect("load resolver"),
            &mut db,
        )
        .expect("restore M4 state");
        db.connection()
            .execute(
                "UPDATE knowledge_meta SET build_id = 'tampered' WHERE id = 1",
                [],
            )
            .expect("tamper M4 materialization identity");
        drop(db);

        let corrupt = content_search_ipc_round_trip(
            Some(fixture.context.clone()),
            DesktopSearchQuery::default(),
        );
        assert!(corrupt.accepted);
        assert!(corrupt.hits.is_none());
        assert_eq!(
            corrupt.hold_reason.as_deref(),
            Some("materialization_state_missing_or_stale")
        );

        fs::remove_dir_all(fixture.root).expect("cleanup");
    }

    #[test]
    fn desktop_content_search_ipc_holds_stale_projection_and_inventory_changes() {
        let stale_fixture = prepared_search_fixture("stale");
        append_memory_note(
            stale_fixture.store.as_ref(),
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &stale_fixture.root,
                brain_id: &stale_fixture.brain_id,
                device_id: &stale_fixture.device,
                classification: Classification::Internal,
                text: "synthetic second event after materialization",
                event_id: None,
                config: &CanonicalWriterConfig::default(),
            },
        )
        .expect("append event after materialization");
        let stale = content_search_ipc_round_trip(
            Some(stale_fixture.context.clone()),
            DesktopSearchQuery::default(),
        );
        assert!(stale.accepted);
        assert!(stale.hits.is_none());
        assert_eq!(
            stale.hold_reason.as_deref(),
            Some("materialization_state_missing_or_stale")
        );
        fs::remove_dir_all(stale_fixture.root).expect("cleanup stale fixture");

        let inventory_fixture = prepared_search_fixture("inventory-change");
        let unexpected = DeviceId::new("new-untrusted-device").expect("unexpected device");
        fs::create_dir_all(canonical_events_device_dir(
            &inventory_fixture.root,
            &unexpected,
        ))
        .expect("add untrusted canonical device directory");
        let changed_inventory = content_search_ipc_round_trip(
            Some(inventory_fixture.context.clone()),
            DesktopSearchQuery::default(),
        );
        assert!(changed_inventory.accepted);
        assert!(changed_inventory.hits.is_none());
        assert_eq!(
            changed_inventory.hold_reason.as_deref(),
            Some("canonical_verification_unavailable")
        );
        fs::remove_dir_all(inventory_fixture.root).expect("cleanup inventory fixture");
    }

    #[test]
    fn desktop_content_search_ipc_brain_wide_hold_never_returns_valid_prefix_hits() {
        let fixture = prepared_search_fixture("brain-wide-hold");
        append_memory_note(
            fixture.store.as_ref(),
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &fixture.root,
                brain_id: &fixture.brain_id,
                device_id: &fixture.device,
                classification: Classification::Internal,
                text: "synthetic second note for full-stream hold",
                event_id: None,
                config: &CanonicalWriterConfig::default(),
            },
        )
        .expect("append second note");
        let mut resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            &fixture.root,
            &fixture.brain_id,
            fixture.store.as_ref(),
            SecureStoreEnvironment::Dev,
        )
        .expect("load local key resolver");
        let mut db = ProjectionDb::open_or_create(&fixture.root).expect("open projection db");
        rebuild_device_projection(
            fixture.store.as_ref(),
            SecureStoreEnvironment::Dev,
            &fixture.root,
            &fixture.device,
            &db,
        )
        .expect("project both notes");
        rebuild_knowledge_from_canonical(
            &fixture.root,
            &fixture.brain_id,
            std::slice::from_ref(&fixture.device),
            &mut resolver,
            &mut db,
        )
        .expect("materialize both notes");
        drop(db);

        let pack = active_segment_path(&fixture.root, &fixture.device, 0);
        let mut bytes = fs::read(&pack).expect("read synthetic canonical pack");
        let last = bytes.last_mut().expect("pack contains events");
        *last ^= 0x01;
        fs::write(&pack, bytes).expect("corrupt trailing canonical event");

        let held = content_search_ipc_round_trip(
            Some(fixture.context.clone()),
            DesktopSearchQuery::default(),
        );
        assert!(held.accepted);
        assert!(held.hits.is_none(), "valid-prefix text must not leak");
        assert_eq!(
            held.hold_reason.as_deref(),
            Some("canonical_verification_unavailable")
        );

        fs::remove_dir_all(fixture.root).expect("cleanup");
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
            expected_desktop: Arc::new(
                test_desktop_process_identity().expect("desktop parent identity"),
            ),
            projection_sync: None,
        };
        let request = || Request {
            protocol_version: PROTOCOL_VERSION,
            request_id: "setup-test".into(),
            client_role: "desktop".into(),
            capabilities: vec![CAPABILITY_IDENTITY_SETUP.into()],
            operation: "identity.enroll_local".into(),
            query: None,
            capture: None,
            capture_resolve: None,
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
                capture: None,
                capture_resolve: None,
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
            capture: None,
            capture_resolve: None,
        });
        assert!(response.accepted);
        assert_eq!(response.operation.as_deref(), Some("health.pong"));
    }

    #[test]
    fn unauthenticated_ipc_workers_are_bounded_and_release_capacity() {
        let active = Arc::new(AtomicUsize::new(0));
        let reservations = (0..MAX_UNAUTHENTICATED_IPC_WORKERS)
            .map(|_| {
                try_reserve_ipc_worker(&active, MAX_UNAUTHENTICATED_IPC_WORKERS)
                    .expect("available worker slot")
            })
            .collect::<Vec<_>>();
        assert!(try_reserve_ipc_worker(&active, MAX_UNAUTHENTICATED_IPC_WORKERS).is_none());
        assert_eq!(
            active.load(Ordering::Acquire),
            MAX_UNAUTHENTICATED_IPC_WORKERS
        );
        drop(reservations);
        assert_eq!(active.load(Ordering::Acquire), 0);
        assert!(try_reserve_ipc_worker(&active, MAX_UNAUTHENTICATED_IPC_WORKERS).is_some());
    }

    #[test]
    fn desktop_search_capacity_is_bounded_separately_from_health_workers() {
        let desktop_connections = Arc::new(AtomicUsize::new(0));
        let search = Arc::new(AtomicUsize::new(0));
        let health = Arc::new(AtomicUsize::new(0));
        let connections = (0..MAX_DESKTOP_SEARCH_WORKERS + 1)
            .map(|_| {
                try_reserve_ipc_worker(&desktop_connections, MAX_AUTHENTICATED_DESKTOP_CONNECTIONS)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let searches = (0..MAX_DESKTOP_SEARCH_WORKERS)
            .map(|_| try_reserve_ipc_worker(&search, MAX_DESKTOP_SEARCH_WORKERS).unwrap())
            .collect::<Vec<_>>();
        assert!(try_reserve_ipc_worker(&search, MAX_DESKTOP_SEARCH_WORKERS).is_none());
        let health_slot = try_reserve_ipc_worker(&health, MAX_UNAUTHENTICATED_IPC_WORKERS);
        assert!(
            health_slot.is_some(),
            "search load must not consume health capacity"
        );
        drop(searches);
        drop(connections);
        assert_eq!(search.load(Ordering::Acquire), 0);
        assert_eq!(desktop_connections.load(Ordering::Acquire), 0);
        assert_eq!(health.load(Ordering::Acquire), 1);
    }

    #[test]
    fn timed_out_search_returns_hold_and_keeps_its_slot_until_work_exits() {
        let active = Arc::new(AtomicUsize::new(0));
        let reservation = try_reserve_ipc_worker(&active, MAX_DESKTOP_SEARCH_WORKERS).unwrap();
        let deadline = Instant::now() + Duration::from_millis(30);
        let response =
            run_bounded_search("slow-search".into(), deadline, Some(reservation), || {
                thread::sleep(Duration::from_millis(120));
                search_results(Some("slow-search".into()), Vec::new())
            });

        assert!(response.accepted);
        assert!(response.hits.is_none(), "late results must never escape");
        assert_eq!(
            response.hold_reason.as_deref(),
            Some("operation_deadline_exceeded")
        );
        assert_eq!(active.load(Ordering::Acquire), 1);
        assert!(try_reserve_ipc_worker(&active, 1).is_none());

        let cleanup_deadline = Instant::now() + Duration::from_secs(2);
        while active.load(Ordering::Acquire) != 0 && Instant::now() < cleanup_deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(active.load(Ordering::Acquire), 0);
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

    fn isolated_ipc_listener(
        _label: &str,
    ) -> (
        interprocess::local_socket::Name<'static>,
        interprocess::local_socket::Listener,
    ) {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        #[cfg(windows)]
        let name =
            std::ffi::OsString::from(format!("cryptex-ipc-{_label}-{}-{suffix}", process::id()))
                .to_ns_name::<GenericNamespaced>()
                .expect("test pipe name");
        #[cfg(unix)]
        let name = {
            // Unix-domain socket paths have a small sun_path limit. Keep test
            // socket names short and use /tmp instead of macOS's long per-user
            // TMPDIR so the same framed-IPC tests fit on hosted runners.
            std::path::Path::new("/tmp")
                .join(format!("cx-{}-{suffix}.sock", process::id()))
                .to_fs_name::<interprocess::local_socket::GenericFilePath>()
                .expect("test socket name")
        };
        let listener = ListenerOptions::new()
            .name(name.borrow())
            .create_sync()
            .expect("test listener");
        (name, listener)
    }

    #[test]
    fn desktop_health_frame_round_trips_on_a_named_pipe() {
        let (name, listener) = isolated_ipc_listener("health");
        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("accept test client");
            handle_connection(&mut stream, None, None, None).expect("health connection");
        });
        let mut client = Stream::connect(name.borrow()).expect("connect test client");
        let deadline = Instant::now() + Duration::from_secs(2);
        let request = Request {
            protocol_version: PROTOCOL_VERSION,
            request_id: "round-trip".into(),
            client_role: "desktop".into(),
            capabilities: vec![CAPABILITY_STATUS_READ.into()],
            operation: "health.ping".into(),
            query: None,
            capture: None,
            capture_resolve: None,
        };
        let mut frame = serde_json::to_vec(&request).expect("request encodes");
        frame.push(b'\n');
        write_ipc_all(&mut client, &frame, deadline).expect("client write");
        let response_frame =
            read_ipc_frame(&mut client, MAX_RESPONSE_BYTES, deadline).expect("client read");
        let response: HealthResponse =
            serde_json::from_slice(&response_frame).expect("response decodes");
        assert!(response.accepted);
        assert_eq!(response.request_id.as_deref(), Some("round-trip"));
        assert_eq!(response.operation.as_deref(), Some("health.pong"));
        assert!(response.error.is_none());
        server.join().expect("server thread");
    }

    #[test]
    fn stalled_pipe_read_returns_at_the_deadline() {
        let (name, listener) = isolated_ipc_listener("stall");
        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("accept stalled client");
            arm_ipc_stream(&stream).expect("arm stalled stream");
            let started = Instant::now();
            let error = read_ipc_frame(&mut stream, 256, started + Duration::from_millis(250))
                .expect_err("stalled read");
            assert_eq!(error.kind(), io::ErrorKind::TimedOut);
            let elapsed = started.elapsed();
            assert!(
                elapsed >= Duration::from_millis(150),
                "stalled read returned before the deadline: {elapsed:?}"
            );
            assert!(
                elapsed < Duration::from_secs(2),
                "stalled read exceeded the deadline bound: {elapsed:?}"
            );
        });
        let _client = Stream::connect(name.borrow()).expect("connect stalled client");
        server.join().expect("server thread");
    }

    #[cfg(windows)]
    #[test]
    fn stalled_pipe_write_returns_at_the_deadline() {
        let (name, listener) = isolated_ipc_listener("write-stall");
        let server = thread::spawn(move || {
            let _stream = listener.accept().expect("accept stalled writer");
            thread::sleep(Duration::from_millis(500));
        });
        let client = Stream::connect(name.borrow()).expect("connect stalled writer");
        let bytes = vec![b'x'; 8 * 1024 * 1024];
        let started = Instant::now();
        let error = write_windows_pipe_all(&client, &bytes, started + Duration::from_millis(250))
            .expect_err("stalled write must reach its deadline");
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "stalled write exceeded the deadline bound: {:?}",
            started.elapsed()
        );
        server.join().expect("stalled writer server thread");
    }

    #[cfg(windows)]
    #[test]
    fn failed_pipe_wait_cancels_and_drains_pending_read() {
        use windows_sys::Win32::Foundation::{ERROR_INVALID_HANDLE, SetLastError, WAIT_FAILED};

        let (name, listener) = isolated_ipc_listener("wait-failure");
        let (sent_tx, sent_rx) = std::sync::mpsc::channel();
        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("accept pending reader");
            thread::sleep(Duration::from_millis(100));
            stream
                .write_all(b"x")
                .expect("write after client cancellation");
            sent_tx.send(()).expect("notify completed peer write");
        });
        let client = Stream::connect(name.borrow()).expect("connect pending reader");
        let mut byte = [0_u8; 1];
        let started = Instant::now();
        let error = transfer_named_pipe_with_wait(
            &client,
            None,
            Some(&mut byte),
            started + Duration::from_secs(2),
            |_, _| {
                unsafe { SetLastError(ERROR_INVALID_HANDLE) };
                WAIT_FAILED
            },
        )
        .expect_err("injected wait failure");
        assert_eq!(error.raw_os_error(), Some(ERROR_INVALID_HANDLE as i32));
        sent_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("peer write after failed wait");
        assert_eq!(byte, [0], "canceled read must not modify its caller buffer");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "failed wait did not cancel and drain promptly: {:?}",
            started.elapsed()
        );
        server.join().expect("pending reader server thread");
    }

    fn capture_request(role: &str, capabilities: Vec<String>, body: DesktopCaptureBody) -> Request {
        Request {
            protocol_version: PROTOCOL_VERSION,
            request_id: "capture-test".into(),
            client_role: role.into(),
            capabilities,
            operation: "memory.capture".into(),
            query: None,
            capture: Some(body),
            capture_resolve: None,
        }
    }

    fn sample_capture_body() -> DesktopCaptureBody {
        DesktopCaptureBody {
            capture_id: "cccccccc-cccc-4ccc-8ccc-cccccccccccc".into(),
            text: "inbox note".into(),
            classification: "internal".into(),
        }
    }

    #[test]
    fn capture_frame_bound_exceeds_search_frame_and_accounts_for_escape_expansion() {
        const {
            assert!(MAX_CAPTURE_FRAME_BYTES > MAX_FRAME_BYTES);
        }
        assert_eq!(
            MAX_CAPTURE_FRAME_BYTES,
            MAX_MEMORY_NOTE_TEXT_BYTES * MAX_JSON_STRING_ESCAPE_FACTOR
                + MAX_CAPTURE_REQUEST_OVERHEAD_BYTES
        );
    }

    #[test]
    fn unauthorized_peer_cannot_capture_or_resolve() {
        let body = sample_capture_body();
        let rejected = handle_capture_until(
            capture_request(
                "desktop",
                vec![CAPABILITY_CONTENT_CAPTURE.into()],
                body.clone(),
            ),
            None,
            false,
        );
        assert!(!rejected.accepted);
        assert_eq!(
            rejected.error.as_deref(),
            Some("unauthorized_client_process")
        );
        assert!(rejected.outcome.is_none());

        let resolve = handle_capture_until(
            Request {
                protocol_version: PROTOCOL_VERSION,
                request_id: "resolve-unauth".into(),
                client_role: "desktop".into(),
                capabilities: vec![CAPABILITY_CONTENT_CAPTURE.into()],
                operation: "memory.capture_resolve".into(),
                query: None,
                capture: None,
                capture_resolve: Some(DesktopCaptureResolveBody {
                    capture_id: body.capture_id,
                }),
            },
            None,
            false,
        );
        assert!(!resolve.accepted);
        assert_eq!(
            resolve.error.as_deref(),
            Some("unauthorized_client_process")
        );
    }

    #[test]
    fn capture_rejects_invalid_classification_and_unknown_fields() {
        let fixture = prepared_search_fixture("capture-class");
        let mut body = sample_capture_body();
        body.classification = "secret".into();
        let response = handle_capture_until(
            capture_request("desktop", vec![CAPABILITY_CONTENT_CAPTURE.into()], body),
            Some(&fixture.context),
            true,
        );
        assert!(!response.accepted);
        assert_eq!(response.error.as_deref(), Some("invalid_classification"));

        let malformed = serde_json::json!({
            "protocol_version": PROTOCOL_VERSION,
            "request_id": "x",
            "client_role": "desktop",
            "capabilities": [CAPABILITY_CONTENT_CAPTURE],
            "operation": "memory.capture",
            "capture": {
                "captureId": "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
                "text": "hi",
                "classification": "internal",
                "extra": true
            }
        });
        let err = serde_json::from_value::<Request>(malformed).expect_err("deny unknown");
        assert!(err.to_string().contains("unknown field") || err.is_data());
        let _ = fs::remove_dir_all(&fixture.root);
    }

    #[test]
    fn capture_commits_and_resolve_replays_without_search_claim() {
        let mut fixture = prepared_search_fixture("capture-commit");
        let (sync_sender, sync_receiver) = sync_channel(1);
        fixture.context.projection_sync = Some(sync_sender);
        let body = sample_capture_body();
        let response = handle_capture_until(
            capture_request(
                "desktop",
                vec![CAPABILITY_CONTENT_CAPTURE.into()],
                body.clone(),
            ),
            Some(&fixture.context),
            true,
        );
        assert!(response.accepted, "{response:?}");
        assert_eq!(response.outcome.as_deref(), Some("committed"));
        assert_eq!(
            response.capture_id.as_deref(),
            Some(body.capture_id.as_str())
        );
        assert!(response.event_id.is_some());
        sync_receiver
            .try_recv()
            .expect("committed capture schedules derived catch-up");
        // Redaction: never echo note body fields that don't exist on the wire type.
        let encoded = serde_json::to_string(&response).unwrap();
        assert!(!encoded.contains("inbox note"));

        let replay = handle_capture_until(
            capture_request(
                "desktop",
                vec![CAPABILITY_CONTENT_CAPTURE.into()],
                body.clone(),
            ),
            Some(&fixture.context),
            true,
        );
        assert_eq!(replay.event_id, response.event_id);

        let resolved = handle_capture_until(
            Request {
                protocol_version: PROTOCOL_VERSION,
                request_id: "resolve-ok".into(),
                client_role: "desktop".into(),
                capabilities: vec![CAPABILITY_CONTENT_CAPTURE.into()],
                operation: "memory.capture_resolve".into(),
                query: None,
                capture: None,
                capture_resolve: Some(DesktopCaptureResolveBody {
                    capture_id: body.capture_id,
                }),
            },
            Some(&fixture.context),
            true,
        );
        assert_eq!(resolved.outcome.as_deref(), Some("committed"));
        assert_eq!(resolved.event_id, response.event_id);
        let _ = fs::remove_dir_all(&fixture.root);
    }

    #[test]
    fn capture_escape_heavy_and_multibyte_text_within_raw_limit() {
        let fixture = prepared_search_fixture("capture-escape");
        let text = format!("{}{}", "\\\"\n\t".repeat(1000), "内存笔记".repeat(100));
        assert!(text.len() <= MAX_MEMORY_NOTE_TEXT_BYTES);
        let body = DesktopCaptureBody {
            capture_id: "dddddddd-dddd-4ddd-8ddd-dddddddddddd".into(),
            text,
            classification: "private".into(),
        };
        let request = capture_request("desktop", vec![CAPABILITY_CONTENT_CAPTURE.into()], body);
        let frame = serde_json::to_vec(&request).unwrap();
        assert!(frame.len() <= MAX_CAPTURE_FRAME_BYTES);
        let response = handle_capture_until(request, Some(&fixture.context), true);
        assert_eq!(
            response.outcome.as_deref(),
            Some("committed"),
            "{response:?}"
        );
        let _ = fs::remove_dir_all(&fixture.root);
    }

    #[test]
    fn capture_id_conflict_and_receipt_missing_are_typed() {
        let fixture = prepared_search_fixture("capture-conflict");
        let mut body = sample_capture_body();
        body.capture_id = "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee".into();
        let first = handle_capture_until(
            capture_request(
                "desktop",
                vec![CAPABILITY_CONTENT_CAPTURE.into()],
                body.clone(),
            ),
            Some(&fixture.context),
            true,
        );
        assert_eq!(first.outcome.as_deref(), Some("committed"));
        body.text = "different text".into();
        let conflict = handle_capture_until(
            capture_request(
                "desktop",
                vec![CAPABILITY_CONTENT_CAPTURE.into()],
                body.clone(),
            ),
            Some(&fixture.context),
            true,
        );
        assert_eq!(conflict.outcome.as_deref(), Some("rejected"));
        assert_eq!(conflict.reject_code.as_deref(), Some("capture_id_conflict"));

        let missing = handle_capture_until(
            Request {
                protocol_version: PROTOCOL_VERSION,
                request_id: "resolve-missing".into(),
                client_role: "desktop".into(),
                capabilities: vec![CAPABILITY_CONTENT_CAPTURE.into()],
                operation: "memory.capture_resolve".into(),
                query: None,
                capture: None,
                capture_resolve: Some(DesktopCaptureResolveBody {
                    capture_id: "ffffffff-ffff-4fff-8fff-ffffffffffff".into(),
                }),
            },
            Some(&fixture.context),
            true,
        );
        assert_eq!(missing.outcome.as_deref(), Some("uncertain"));
        assert_eq!(missing.uncertain_reason.as_deref(), Some("receipt_missing"));
        let _ = fs::remove_dir_all(&fixture.root);
    }

    #[test]
    fn search_capability_cannot_invoke_capture() {
        let fixture = prepared_search_fixture("capture-cap");
        let response = handle_capture_until(
            capture_request(
                "desktop",
                vec![CAPABILITY_CONTENT_SEARCH.into()],
                sample_capture_body(),
            ),
            Some(&fixture.context),
            true,
        );
        assert!(!response.accepted);
        assert_eq!(response.error.as_deref(), Some("unauthorized_capability"));
        let _ = fs::remove_dir_all(&fixture.root);
    }

    #[test]
    fn desktop_capture_capacity_is_bounded_and_keeps_health_slots() {
        let capture = Arc::new(AtomicUsize::new(0));
        let health = Arc::new(AtomicUsize::new(0));
        let held = try_reserve_ipc_worker(&capture, MAX_DESKTOP_CAPTURE_WORKERS)
            .expect("first capture slot");
        assert!(
            try_reserve_ipc_worker(&capture, MAX_DESKTOP_CAPTURE_WORKERS).is_none(),
            "second capture worker must be rejected while one is held"
        );
        let health_slot = try_reserve_ipc_worker(&health, MAX_UNAUTHENTICATED_IPC_WORKERS);
        assert!(
            health_slot.is_some(),
            "capture load must not consume health capacity"
        );
        drop(held);
        assert_eq!(capture.load(Ordering::Acquire), 0);
        assert!(try_reserve_ipc_worker(&capture, MAX_DESKTOP_CAPTURE_WORKERS).is_some());
    }

    #[test]
    fn response_write_deadline_is_operation_appropriate() {
        let capture = response_write_deadline_for_operation("memory.capture");
        let resolve = response_write_deadline_for_operation("memory.capture_resolve");
        let health = response_write_deadline_for_operation("status.health");
        let search = response_write_deadline_for_operation("content.search");
        let enroll = response_write_deadline_for_operation("identity.enroll_local");
        let now = Instant::now();
        let capture_skew = capture.saturating_duration_since(now);
        let health_skew = health.saturating_duration_since(now);
        assert!(
            capture_skew >= IPC_CAPTURE_RESPONSE_DEADLINE - Duration::from_millis(50),
            "capture write deadline must use the capture bound, got {capture_skew:?}"
        );
        assert!(
            resolve.saturating_duration_since(now)
                >= IPC_CAPTURE_RESPONSE_DEADLINE - Duration::from_millis(50)
        );
        assert!(
            health_skew <= IPC_RESPONSE_DEADLINE + Duration::from_millis(50),
            "health write deadline must stay on the standard response bound, got {health_skew:?}"
        );
        assert!(
            search.saturating_duration_since(now)
                <= IPC_RESPONSE_DEADLINE + Duration::from_millis(50)
        );
        assert!(
            enroll.saturating_duration_since(now)
                <= IPC_RESPONSE_DEADLINE + Duration::from_millis(50)
        );
        assert!(IPC_CAPTURE_RESPONSE_DEADLINE > IPC_RESPONSE_DEADLINE);
    }

    #[test]
    fn bounded_capture_deadline_returns_uncertain_not_cancel() {
        let response = run_bounded_capture(
            "deadline-capture".into(),
            Instant::now() + Duration::from_millis(30),
            None,
            || {
                thread::sleep(Duration::from_millis(200));
                capture_rejected(Some("deadline-capture".into()), "should_not_surface")
            },
        );
        assert!(response.accepted);
        assert_eq!(response.outcome.as_deref(), Some("uncertain"));
        assert_eq!(
            response.uncertain_reason.as_deref(),
            Some("operation_deadline_exceeded")
        );
        assert!(response.error.is_none());
        // Give the detached worker a moment to finish without joining (no cancel claim).
        thread::sleep(Duration::from_millis(250));
    }
}
