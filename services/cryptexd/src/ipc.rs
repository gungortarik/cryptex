//! Local daemon IPC. Health and guarded Desktop retrieval are the only exposed
//! operations; the daemon owns canonical verification and SQLite access.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;
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
    spawn_server(Some(SearchContext {
        runtime_root: runtime_root.to_path_buf(),
        store,
        environment,
    }))
}

struct SearchContext {
    runtime_root: PathBuf,
    store: Arc<dyn SecureStore>,
    environment: SecureStoreEnvironment,
}

fn spawn_server(context: Option<SearchContext>) -> io::Result<()> {
    let listener = create_listener()?;
    thread::Builder::new()
        .name("cryptex-ipc".into())
        .spawn(move || {
            for incoming in listener.incoming() {
                match incoming {
                    Ok(mut stream) => {
                        if let Err(error) = handle_connection(&mut stream, context.as_ref()) {
                            eprintln!("cryptexd IPC connection failed: {error}");
                        }
                    }
                    Err(error) => eprintln!("cryptexd IPC accept failed: {error}"),
                }
            }
        })?;
    Ok(())
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
    serde_json::to_writer(&mut *stream, request)?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    let mut frame = Vec::with_capacity(256);
    let count = BufReader::new(stream)
        .take((max_response_bytes + 1) as u64)
        .read_until(b'\n', &mut frame)?;
    if count == 0 || count > max_response_bytes || frame.last() != Some(&b'\n') {
        return Err(invalid_data("invalid IPC response frame"));
    }
    Ok(frame)
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
    let mut reader = BufReader::new(stream);
    let mut frame = Vec::with_capacity(1024);
    let count = reader
        .by_ref()
        .take((MAX_FRAME_BYTES + 1) as u64)
        .read_until(b'\n', &mut frame)?;
    let response = if count == 0 {
        return Ok(());
    } else if count > MAX_FRAME_BYTES {
        serde_json::to_vec(&rejected(None, "frame_too_large"))?
    } else if frame.last() != Some(&b'\n') {
        serde_json::to_vec(&rejected(None, "malformed_frame"))?
    } else {
        match serde_json::from_slice::<Request>(&frame) {
            Ok(request) => {
                if request.operation == "content.search" {
                    let response = handle_search(request, search);
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
                    serde_json::to_vec(&handle_enrollment(request, search))?
                } else {
                    serde_json::to_vec(&authorize(request))?
                }
            }
            Err(_) => serde_json::to_vec(&rejected(None, "malformed_request"))?,
        }
    };
    reader.get_mut().write_all(&response)?;
    reader.get_mut().write_all(b"\n")?;
    reader.get_mut().flush()
}

fn handle_enrollment(
    request: Request,
    context: Option<&SearchContext>,
) -> DesktopEnrollmentResponse {
    let request_id = request.request_id.clone();
    let authorization_error = if request.protocol_version != PROTOCOL_VERSION {
        Some("incompatible_protocol_version")
    } else if request.request_id.is_empty() || request.request_id.len() > 128 {
        Some("invalid_request_id")
    } else if request.client_role != "desktop" {
        Some("unauthorized_client_role")
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

fn handle_search(request: Request, context: Option<&SearchContext>) -> DesktopSearchResponse {
    let request_id = request.request_id.clone();
    let authorization_error = if request.protocol_version != PROTOCOL_VERSION {
        Some("incompatible_protocol_version")
    } else if request.request_id.is_empty() || request.request_id.len() > 128 {
        Some("invalid_request_id")
    } else if request.client_role != "desktop" {
        Some("unauthorized_client_role")
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
        let response = handle_search(desktop, None);
        assert!(!response.accepted);
        assert_eq!(response.error.as_deref(), Some("search_unavailable"));
        assert!(response.hits.is_none());

        let cli = search_request("cli", vec![CAPABILITY_CONTENT_SEARCH.into()]);
        let response = handle_search(cli, None);
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
        let response = handle_search(extra_capability, None);
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
        };
        let response = handle_search(
            search_request("desktop", vec![CAPABILITY_CONTENT_SEARCH.into()]),
            Some(&context),
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
        };
        let response = handle_search(
            search_request("desktop", vec![CAPABILITY_CONTENT_SEARCH.into()]),
            Some(&context),
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
        };
        let request = || Request {
            protocol_version: PROTOCOL_VERSION,
            request_id: "setup-test".into(),
            client_role: "desktop".into(),
            capabilities: vec![CAPABILITY_IDENTITY_SETUP.into()],
            operation: "identity.enroll_local".into(),
            query: None,
        };

        let first = handle_enrollment(request(), Some(&context));
        let second = handle_enrollment(request(), Some(&context));
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
        );
        assert!(!unauthorized.accepted);
        assert_eq!(
            unauthorized.error.as_deref(),
            Some("unauthorized_client_role")
        );
        fs::remove_dir_all(root).expect("cleanup");
    }
}
