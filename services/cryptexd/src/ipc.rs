//! Local daemon IPC. Health and guarded Desktop retrieval are the only exposed
//! operations; the daemon owns canonical verification and SQLite access.

use std::collections::HashMap;
use std::fs;
use std::io;
#[cfg(any(not(windows), test))]
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
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
use crate::event_signing_key::{
    load_existing_event_signing_key, load_existing_event_verifying_key,
};
use crate::knowledge_materialization::{GuardedSearchError, RetrievalSafetyHold};
use crate::local_device_identity::LocalDeviceKeyResolver;
use crate::secure_store::{SecureStore, SecureStoreEnvironment};
use crate::{
    identity_runtime::load_brain_identity, local_device_identity::load_local_device_identity,
};
use cryptex_core::DeviceId;
use cryptex_events::Classification;
use cryptex_events::limits::MAX_MEMORY_NOTE_TEXT_BYTES;
use cryptex_projection::{ProjectionDb, SearchHit, SearchQuery, TimelineCursor, TimelineQuery};
#[cfg(windows)]
use interprocess::local_socket::GenericNamespaced;
use interprocess::local_socket::{ListenerOptions, Stream, prelude::*};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const PROTOCOL_VERSION: u32 = DAEMON_PROTOCOL_VERSION;
const MAX_FRAME_BYTES: usize = 64 * 1024;
const CAPABILITY_STATUS_READ: &str = "status.read";
const CAPABILITY_CONTENT_SEARCH: &str = "content.search";
const CAPABILITY_IDENTITY_SETUP: &str = "identity.setup";
const CAPABILITY_BOOTSTRAP_APPROVAL: &str = "peer.approval.manage";
const MAX_SEARCH_TEXT_BYTES: usize = 1024;
const MAX_SEARCH_FILTER_BYTES: usize = 128;
const MAX_SEARCH_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_DETAIL_RESPONSE_BYTES: usize =
    MAX_MEMORY_NOTE_TEXT_BYTES * MAX_JSON_STRING_ESCAPE_FACTOR + MAX_CAPTURE_REQUEST_OVERHEAD_BYTES;
const MAX_TIMELINE_PAGE_SIZE: u32 = 50;
const TIMELINE_SNIPPET_BYTES: usize = 384;
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
const CAPABILITY_CONTENT_VAULT: &str = "content.vault";
const IPC_VAULT_RESPONSE_DEADLINE: Duration = Duration::from_secs(300);
const MAX_VAULT_ROOT_BYTES: usize = 4096;
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
const IPC_APPROVAL_SESSION_DEADLINE: Duration = Duration::from_secs(5 * 60);
const MAX_BOOTSTRAP_APPROVAL_FRAME_BYTES: usize = 1024 * 1024 + 4096;
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vault: Option<DesktopVaultBody>,
}

/// Closed wire envelope for the two-phase, connection-bound approval flow.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BootstrapApprovalIpcRequest {
    protocol_version: u32,
    request_id: String,
    client_role: String,
    capabilities: Vec<String>,
    operation: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    transcript_messages: Option<Vec<String>>,
    #[serde(default)]
    approval_request_digest: Option<String>,
    #[serde(default)]
    capability: Option<String>,
    #[serde(default)]
    transport_device_id: Option<String>,
    #[serde(default)]
    offer_message: Option<String>,
    #[serde(default)]
    brain_identity_json: Option<String>,
    #[serde(default)]
    transaction_id: Option<String>,
    #[serde(default)]
    owner_operation: Option<OwnerSecurityOperationRequest>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum OwnerSecurityOperationRequest {
    DeviceRestriction {
        target_device_id: String,
        reason: crate::bootstrap_journal::RestrictionReason,
        cutoff_seq: u64,
    },
    SelfRetirement,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapApprovalIpcResponse {
    protocol_version: u32,
    request_id: Option<String>,
    accepted: bool,
    error: Option<&'static str>,
    operation: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    approval_request_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    capability: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    snapshot: Option<crate::bootstrap_journal::ApprovalSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signed_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exchange_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    brain_identity_json: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BootstrapApprovalIpcResponseWire {
    protocol_version: u32,
    request_id: Option<String>,
    accepted: bool,
    error: Option<String>,
    operation: String,
    approval_request_digest: Option<String>,
    capability: Option<String>,
    snapshot: Option<crate::bootstrap_journal::ApprovalSnapshot>,
    signed_message: Option<String>,
    exchange_message: Option<String>,
    transaction_id: Option<String>,
    brain_identity_json: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopBootstrapApprovalPrepared {
    /// Opaque Desktop-local handle. The signing capability remains in memory
    /// beside the IPC stream and is never exposed to the renderer.
    pub session_handle: String,
    pub snapshot: crate::bootstrap_journal::ApprovalSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopOwnerSecurityOperationInput {
    pub kind: String,
    #[serde(default)]
    pub target_device_id: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub cutoff_seq: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopOwnerSecurityOperationPrepared {
    /// Opaque Desktop-local handle. The confirmation capability remains in
    /// memory beside the authenticated IPC stream and never reaches the UI.
    pub session_handle: String,
    pub snapshot: crate::bootstrap_journal::ApprovalSnapshot,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopOwnerSecurityOperationResult {
    pub accepted: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopBootstrapApprovalResult {
    pub accepted: bool,
    pub error: Option<String>,
    pub signed_message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopBootstrapExchangeResult {
    pub accepted: bool,
    pub error: Option<String>,
    pub transaction_id: Option<String>,
    pub exchange_message: Option<String>,
    pub brain_identity_json: Option<String>,
}

struct PendingDesktopBootstrapApproval {
    stream: Stream,
    request_id: String,
    role: String,
    transcript_messages: Vec<String>,
    approval_request_digest: String,
    capability: String,
    expires_at: Instant,
}

struct PendingDesktopOwnerSecurityOperation {
    stream: Stream,
    request_id: String,
    transaction_id: String,
    approval_request_digest: String,
    capability: String,
    expires_at: Instant,
}

static BOOTSTRAP_APPROVAL_SESSIONS: OnceLock<crate::approval_session::ApprovalSessionRegistry> =
    OnceLock::new();
static DESKTOP_BOOTSTRAP_APPROVAL_SESSIONS: OnceLock<
    Mutex<HashMap<String, PendingDesktopBootstrapApproval>>,
> = OnceLock::new();
static DESKTOP_OWNER_SECURITY_SESSIONS: OnceLock<
    Mutex<HashMap<String, PendingDesktopOwnerSecurityOperation>>,
> = OnceLock::new();

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

/// Explicit destination for a daemon-owned derived-vault rebuild.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopVaultBody {
    pub vault_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopVaultResponse {
    pub protocol_version: u32,
    pub request_id: Option<String>,
    pub accepted: bool,
    pub error: Option<String>,
    pub operation: Option<String>,
    pub daemon_pid: u32,
    pub note_count: Option<usize>,
    pub open_note_path: Option<String>,
    pub hold_reason: Option<String>,
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
    #[serde(default)]
    pub cursor: Option<DesktopTimelineCursor>,
    #[serde(default)]
    pub event_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopTimelineCursor {
    pub created_at: String,
    pub event_id: String,
}

impl DesktopSearchQuery {
    fn validate_and_convert(&self) -> Result<SearchQuery, &'static str> {
        if self.cursor.is_some() || self.event_id.is_some() {
            return Err("invalid_search_cursor");
        }
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

    fn validate_timeline_and_convert(&self) -> Result<TimelineQuery, &'static str> {
        if self.event_id.is_some() {
            return Err("unexpected_timeline_event_id");
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
        if self.text.is_some() {
            return Err("unexpected_timeline_text");
        }
        if self
            .limit
            .is_some_and(|limit| !(1..=MAX_TIMELINE_PAGE_SIZE).contains(&limit))
        {
            return Err("invalid_timeline_limit");
        }
        let cursor = self
            .cursor
            .as_ref()
            .map(|cursor| {
                if cursor.created_at.is_empty()
                    || cursor.created_at.len() > 64
                    || cursor.event_id.is_empty()
                    || cursor.event_id.len() > 128
                {
                    return Err("invalid_timeline_cursor");
                }
                Ok(TimelineCursor {
                    created_at: cursor.created_at.clone(),
                    event_id: cursor.event_id.clone(),
                })
            })
            .transpose()?;
        Ok(TimelineQuery {
            classification: self.classification.clone(),
            event_type: self.event_type.clone(),
            device_id: self.device_id.clone(),
            brain_id: None,
            created_at_from: self.created_at_from.clone(),
            created_at_to: self.created_at_to.clone(),
            cursor,
            limit: self.limit.unwrap_or(20),
        })
    }

    fn validate_event_detail_id(&self) -> Result<&str, &'static str> {
        if self.text.is_some()
            || self.classification.is_some()
            || self.event_type.is_some()
            || self.device_id.is_some()
            || self.created_at_from.is_some()
            || self.created_at_to.is_some()
            || self.limit.is_some()
            || self.cursor.is_some()
        {
            return Err("unexpected_event_detail_filter");
        }
        let event_id = self.event_id.as_deref().ok_or("missing_event_id")?;
        if event_id.is_empty() || event_id.len() > 128 {
            return Err("invalid_event_id");
        }
        Ok(event_id)
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
    pub content_hash: String,
    pub signing_key_id: String,
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
            content_hash: hit.content_hash,
            signing_key_id: hit.signing_key_id,
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
    #[serde(default)]
    pub next_cursor: Option<DesktopTimelineCursor>,
    #[serde(default)]
    pub has_more: bool,
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
        #[cfg(windows)]
        transport_status: None,
        #[cfg(test)]
        transport_device_id_override: None,
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
    spawn_daemon_ipc_server_inner(
        runtime_root,
        store,
        environment,
        #[cfg(windows)]
        None,
    )
}

#[cfg(windows)]
pub fn spawn_daemon_ipc_server_with_transport(
    runtime_root: &Path,
    store: Arc<dyn SecureStore>,
    environment: SecureStoreEnvironment,
    transport: crate::managed_profile::worker::StatusReader,
) -> io::Result<()> {
    spawn_daemon_ipc_server_inner(runtime_root, store, environment, Some(transport))
}

fn spawn_daemon_ipc_server_inner(
    runtime_root: &Path,
    store: Arc<dyn SecureStore>,
    environment: SecureStoreEnvironment,
    #[cfg(windows)] transport: Option<crate::managed_profile::worker::StatusReader>,
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
        #[cfg(windows)]
        transport_status: None,
        #[cfg(test)]
        transport_device_id_override: None,
    };
    start_projection_sync_worker(&mut context);
    #[cfg(windows)]
    {
        context.transport_status = transport;
    }
    spawn_server(Some(context))
}

#[derive(Clone)]
struct SearchContext {
    runtime_root: PathBuf,
    store: Arc<dyn SecureStore>,
    environment: SecureStoreEnvironment,
    expected_desktop: Arc<DesktopProcessIdentity>,
    projection_sync: Option<SyncSender<()>>,
    #[cfg(windows)]
    transport_status: Option<crate::managed_profile::worker::StatusReader>,
    #[cfg(test)]
    transport_device_id_override: Option<String>,
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
    let desktop_vault_workers = Arc::new(AtomicUsize::new(0));
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
                            let vault_workers = Arc::clone(&desktop_vault_workers);
                            if let Err(error) = thread::Builder::new()
                                .name("cryptex-ipc-desktop".into())
                                .spawn(move || {
                                    let _reservation = connection_reservation;
                                    if let Err(error) = handle_connection(
                                        &mut stream,
                                        worker_context.as_deref(),
                                        Some(&search_workers),
                                        Some(&capture_workers),
                                        Some(&vault_workers),
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
        vault: None,
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
        vault: None,
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

/// Read the transport ID returned by the authenticated, daemon-owned adapter.
/// The adapter must be configured, active and identity-matched; no credentials
/// or filesystem paths cross IPC.
pub fn desktop_read_transport_device_id(timeout: Duration) -> io::Result<String> {
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
    arm_ipc_stream(&stream)?;
    let request_id = new_bootstrap_event_id().map_err(invalid_data)?;
    let request = Request {
        protocol_version: PROTOCOL_VERSION,
        request_id: request_id.clone(),
        client_role: "desktop".into(),
        capabilities: vec![CAPABILITY_STATUS_READ.into()],
        operation: "transport.status".into(),
        query: None,
        capture: None,
        capture_resolve: None,
        vault: None,
    };
    let frame = exchange(&mut stream, &request, MAX_RESPONSE_BYTES)?;
    let response: DesktopTransportResponse = serde_json::from_slice(&frame)?;
    if response.protocol_version != PROTOCOL_VERSION
        || response.request_id.as_deref() != Some(request_id.as_str())
        || response.operation.as_deref() != Some("transport.status.result")
    {
        return Err(invalid_data("transport status response mismatch"));
    }
    if !response.accepted || !response.configured || response.protected {
        return Err(invalid_data(
            response
                .error
                .as_deref()
                .unwrap_or("transport not configured"),
        ));
    }
    response
        .transport_device_id
        .filter(|id| crate::syncthing_rest::native_id_shape(id))
        .ok_or_else(|| invalid_data("verified transport identity unavailable"))
}

pub fn desktop_timeline(
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
        operation: "content.timeline".into(),
        query: Some(query),
        capture: None,
        capture_resolve: None,
        vault: None,
    };
    let frame = exchange(&mut stream, &request, MAX_SEARCH_RESPONSE_BYTES)?;
    let response: DesktopSearchResponse = serde_json::from_slice(&frame)?;
    if response.protocol_version != PROTOCOL_VERSION
        || response.request_id.as_deref() != Some(&request_id)
        || (response.accepted && response.operation.as_deref() != Some("content.timeline.result"))
    {
        return Err(invalid_data(
            "IPC timeline response did not match the request",
        ));
    }
    Ok(response)
}

pub fn desktop_event_detail(
    event_id: String,
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
        operation: "content.event.detail".into(),
        query: Some(DesktopSearchQuery {
            event_id: Some(event_id),
            ..DesktopSearchQuery::default()
        }),
        capture: None,
        capture_resolve: None,
        vault: None,
    };
    let frame = exchange(&mut stream, &request, MAX_DETAIL_RESPONSE_BYTES)?;
    let response: DesktopSearchResponse = serde_json::from_slice(&frame)?;
    if response.protocol_version != PROTOCOL_VERSION
        || response.request_id.as_deref() != Some(&request_id)
        || (response.accepted
            && response.operation.as_deref() != Some("content.event.detail.result"))
    {
        return Err(invalid_data(
            "IPC event detail response did not match the request",
        ));
    }
    Ok(response)
}

/// Rebuild the selected Obsidian vault from verified local canonical history.
/// The destination is explicit and is never logged or echoed in the response.
pub fn desktop_rebuild_vault(
    vault_root: String,
    timeout: Duration,
) -> io::Result<DesktopVaultResponse> {
    if vault_root.is_empty() || vault_root.len() > MAX_VAULT_ROOT_BYTES {
        return Err(invalid_data("invalid vault destination"));
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
        capabilities: vec![CAPABILITY_CONTENT_VAULT.into()],
        operation: "content.vault.rebuild".into(),
        query: None,
        capture: None,
        capture_resolve: None,
        vault: Some(DesktopVaultBody { vault_root }),
    };
    let deadline = Instant::now() + timeout.min(IPC_VAULT_RESPONSE_DEADLINE);
    let frame = exchange_until(&mut stream, &request, MAX_RESPONSE_BYTES, deadline)?;
    let response: DesktopVaultResponse = serde_json::from_slice(&frame)?;
    if response.protocol_version != PROTOCOL_VERSION
        || response.request_id.as_deref() != Some(&request_id)
        || (response.accepted && response.operation.as_deref() != Some("content.vault.rebuilt"))
    {
        return Err(invalid_data("IPC vault response did not match the request"));
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
        vault: None,
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

/// Ask the daemon to verify a signed bootstrap prefix and freeze the exact
/// owner-visible approval snapshot. The returned handle keeps the same local
/// IPC stream alive; no signing capability is returned to the renderer.
pub fn desktop_prepare_bootstrap_approval(
    role: String,
    transcript_messages: Vec<String>,
    timeout: Duration,
) -> io::Result<DesktopBootstrapApprovalPrepared> {
    let started = Instant::now();
    let name = socket_name()?;
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
    arm_ipc_stream(&stream)?;
    let request_id = new_bootstrap_event_id().map_err(invalid_data)?;
    let request = BootstrapApprovalIpcRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: request_id.clone(),
        client_role: "desktop".into(),
        capabilities: vec![CAPABILITY_BOOTSTRAP_APPROVAL.into()],
        operation: "bootstrap.approval.prepare".into(),
        role: Some(role.clone()),
        transcript_messages: Some(transcript_messages.clone()),
        approval_request_digest: None,
        capability: None,
        transport_device_id: None,
        offer_message: None,
        brain_identity_json: None,
        transaction_id: None,
        owner_operation: None,
    };
    let response = exchange_bootstrap_approval(&mut stream, &request, Instant::now() + timeout)?;
    if response.protocol_version != PROTOCOL_VERSION
        || response.request_id.as_deref() != Some(request_id.as_str())
        || response.operation != "bootstrap.approval.prepared"
        || !response.accepted
    {
        return Err(invalid_data(
            response
                .error
                .as_deref()
                .unwrap_or("daemon rejected bootstrap approval preparation"),
        ));
    }
    let snapshot = response
        .snapshot
        .ok_or_else(|| invalid_data("approval snapshot missing"))?;
    let approval_request_digest = response
        .approval_request_digest
        .ok_or_else(|| invalid_data("approval digest missing"))?;
    let capability = response
        .capability
        .ok_or_else(|| invalid_data("approval capability missing"))?;
    let session_handle = new_bootstrap_event_id().map_err(invalid_data)?;
    let sessions = DESKTOP_BOOTSTRAP_APPROVAL_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut sessions = sessions
        .lock()
        .map_err(|_| invalid_data("approval session store unavailable"))?;
    sessions.retain(|_, session| session.expires_at > Instant::now());
    if sessions.len() >= 8 {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "too many pending approval sessions",
        ));
    }
    sessions.insert(
        session_handle.clone(),
        PendingDesktopBootstrapApproval {
            stream,
            request_id,
            role,
            transcript_messages,
            approval_request_digest,
            capability,
            expires_at: Instant::now() + IPC_APPROVAL_SESSION_DEADLINE,
        },
    );
    Ok(DesktopBootstrapApprovalPrepared {
        session_handle,
        snapshot,
    })
}

/// Prepare a frozen owner-security operation request. All identity and trust
/// fields are supplied by cryptexd; this input contains only the requested
/// action and its public cutoff parameters.
pub fn desktop_prepare_owner_security_operation(
    input: DesktopOwnerSecurityOperationInput,
    timeout: Duration,
) -> io::Result<DesktopOwnerSecurityOperationPrepared> {
    let owner_operation = match input.kind.as_str() {
        "device_restriction" => {
            let target_device_id = input
                .target_device_id
                .filter(|id| !id.is_empty() && id.len() <= 128)
                .ok_or_else(|| invalid_data("restriction target is required"))?;
            let reason = match input.reason.as_deref() {
                Some("retired") => crate::bootstrap_journal::RestrictionReason::Retired,
                Some("compromised") => crate::bootstrap_journal::RestrictionReason::Compromised,
                _ => return Err(invalid_data("restriction reason is invalid")),
            };
            let cutoff_seq = input
                .cutoff_seq
                .ok_or_else(|| invalid_data("restriction cutoff is required"))?;
            OwnerSecurityOperationRequest::DeviceRestriction {
                target_device_id,
                reason,
                cutoff_seq,
            }
        }
        "self_retirement" => {
            if input.target_device_id.is_some()
                || input.reason.is_some()
                || input.cutoff_seq.is_some()
            {
                return Err(invalid_data("self-retirement has unexpected peer fields"));
            }
            OwnerSecurityOperationRequest::SelfRetirement
        }
        _ => return Err(invalid_data("unsupported owner-security operation")),
    };
    let started = Instant::now();
    let name = socket_name()?;
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
    arm_ipc_stream(&stream)?;
    let request_id = new_bootstrap_event_id().map_err(invalid_data)?;
    let transaction_id = new_bootstrap_transaction_id().map_err(invalid_data)?;
    let request = BootstrapApprovalIpcRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: request_id.clone(),
        client_role: "desktop".into(),
        capabilities: vec![CAPABILITY_BOOTSTRAP_APPROVAL.into()],
        operation: "owner.security.prepare".into(),
        role: None,
        transcript_messages: None,
        approval_request_digest: None,
        capability: None,
        transport_device_id: None,
        offer_message: None,
        brain_identity_json: None,
        transaction_id: Some(transaction_id.clone()),
        owner_operation: Some(owner_operation),
    };
    let response = exchange_bootstrap_approval(&mut stream, &request, Instant::now() + timeout)?;
    if response.protocol_version != PROTOCOL_VERSION
        || response.request_id.as_deref() != Some(request_id.as_str())
        || response.operation != "owner.security.prepared"
        || !response.accepted
    {
        return Err(invalid_data(
            response
                .error
                .as_deref()
                .unwrap_or("daemon rejected owner-security preparation"),
        ));
    }
    let snapshot = response
        .snapshot
        .ok_or_else(|| invalid_data("owner-security snapshot missing"))?;
    let approval_request_digest = response
        .approval_request_digest
        .ok_or_else(|| invalid_data("owner-security digest missing"))?;
    let capability = response
        .capability
        .ok_or_else(|| invalid_data("owner-security capability missing"))?;
    let session_handle = new_bootstrap_event_id().map_err(invalid_data)?;
    let sessions = DESKTOP_OWNER_SECURITY_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut sessions = sessions
        .lock()
        .map_err(|_| invalid_data("owner-security session store unavailable"))?;
    sessions.retain(|_, session| session.expires_at > Instant::now());
    if sessions.len() >= 8 {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "too many pending owner-security sessions",
        ));
    }
    sessions.insert(
        session_handle.clone(),
        PendingDesktopOwnerSecurityOperation {
            stream,
            request_id,
            transaction_id,
            approval_request_digest,
            capability,
            expires_at: Instant::now() + IPC_APPROVAL_SESSION_DEADLINE,
        },
    );
    Ok(DesktopOwnerSecurityOperationPrepared {
        session_handle,
        snapshot,
    })
}

pub fn desktop_confirm_owner_security_operation(
    session_handle: String,
) -> io::Result<DesktopOwnerSecurityOperationResult> {
    finish_desktop_owner_security_operation(session_handle, "owner.security.confirm")
}

pub fn desktop_cancel_owner_security_operation(
    session_handle: String,
) -> io::Result<DesktopOwnerSecurityOperationResult> {
    finish_desktop_owner_security_operation(session_handle, "owner.security.cancel")
}

fn finish_desktop_owner_security_operation(
    session_handle: String,
    operation: &str,
) -> io::Result<DesktopOwnerSecurityOperationResult> {
    if session_handle.len() != 36
        || !session_handle
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
    {
        return Err(invalid_data("invalid owner-security session handle"));
    }
    let sessions = DESKTOP_OWNER_SECURITY_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut session = sessions
        .lock()
        .map_err(|_| invalid_data("owner-security session store unavailable"))?
        .remove(&session_handle)
        .ok_or_else(|| invalid_data("owner-security session expired or unavailable"))?;
    if session.expires_at <= Instant::now() {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "owner-security session expired",
        ));
    }
    let request = BootstrapApprovalIpcRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: session.request_id.clone(),
        client_role: "desktop".into(),
        capabilities: vec![CAPABILITY_BOOTSTRAP_APPROVAL.into()],
        operation: operation.into(),
        role: None,
        transcript_messages: None,
        approval_request_digest: Some(session.approval_request_digest),
        capability: Some(session.capability),
        transport_device_id: None,
        offer_message: None,
        brain_identity_json: None,
        transaction_id: Some(session.transaction_id),
        owner_operation: None,
    };
    let response = exchange_bootstrap_approval(
        &mut session.stream,
        &request,
        Instant::now() + IPC_RESPONSE_DEADLINE,
    )?;
    let expected = if operation == "owner.security.cancel" {
        "owner.security.cancelled"
    } else {
        "owner.security.result"
    };
    if response.protocol_version != PROTOCOL_VERSION
        || response.request_id.as_deref() != Some(session.request_id.as_str())
        || response.operation != expected
    {
        return Err(invalid_data(
            "owner-security response did not match the request",
        ));
    }
    Ok(DesktopOwnerSecurityOperationResult {
        accepted: response.accepted,
        error: response.error,
    })
}

/// Create or resume an initiator offer. The transaction ID is generated once
/// in this authenticated Desktop backend so a retried IPC request retrieves
/// the exact journaled offer rather than minting new bytes.
pub fn desktop_create_bootstrap_offer(
    transaction_id: String,
    transport_device_id: String,
    timeout: Duration,
) -> io::Result<DesktopBootstrapExchangeResult> {
    if transaction_id.len() != 64
        || !transaction_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid_data("invalid bootstrap transaction ID"));
    }
    desktop_exchange_bootstrap_message(
        "bootstrap.exchange.offer.create",
        Some(transport_device_id),
        None,
        None,
        Some(transaction_id),
        timeout,
    )
}

/// Generate a transaction identifier before the offer request so Desktop can
/// retry an interrupted request using the exact same durable journal key.
pub fn desktop_new_bootstrap_transaction_id() -> io::Result<String> {
    new_bootstrap_transaction_id().map_err(invalid_data)
}

/// Read the exact canonical public Brain identity for transfer to a joining
/// installation. This returns no signing material or local private state.
pub fn desktop_read_bootstrap_brain_identity(timeout: Duration) -> io::Result<String> {
    let response = desktop_exchange_bootstrap_message(
        "bootstrap.exchange.brain_identity.read",
        None,
        None,
        None,
        None,
        timeout,
    )?;
    if !response.accepted {
        return Err(invalid_data(
            response
                .error
                .as_deref()
                .unwrap_or("Brain identity read refused"),
        ));
    }
    response
        .brain_identity_json
        .ok_or_else(|| invalid_data("Brain identity response missing"))
}

/// Persist an incoming offer, create its locally signed proof, then return the
/// exact journaled proof. Replays use the same proof bytes and transaction.
pub fn desktop_create_bootstrap_proof(
    offer_message: String,
    brain_identity_json: String,
    transport_device_id: String,
    timeout: Duration,
) -> io::Result<DesktopBootstrapExchangeResult> {
    desktop_exchange_bootstrap_message(
        "bootstrap.exchange.proof.create",
        Some(transport_device_id),
        Some(offer_message),
        Some(brain_identity_json),
        None,
        timeout,
    )
}

fn desktop_exchange_bootstrap_message(
    operation: &str,
    transport_device_id: Option<String>,
    offer_message: Option<String>,
    brain_identity_json: Option<String>,
    transaction_id: Option<String>,
    timeout: Duration,
) -> io::Result<DesktopBootstrapExchangeResult> {
    let started = Instant::now();
    let name = socket_name()?;
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
    arm_ipc_stream(&stream)?;
    let request_id = new_bootstrap_event_id().map_err(invalid_data)?;
    let request = BootstrapApprovalIpcRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: request_id.clone(),
        client_role: "desktop".into(),
        capabilities: vec![CAPABILITY_BOOTSTRAP_APPROVAL.into()],
        operation: operation.into(),
        role: None,
        transcript_messages: None,
        approval_request_digest: None,
        capability: None,
        transport_device_id,
        offer_message,
        brain_identity_json,
        transaction_id,
        owner_operation: None,
    };
    let response = exchange_bootstrap_approval(&mut stream, &request, Instant::now() + timeout)?;
    if response.protocol_version != PROTOCOL_VERSION
        || response.request_id.as_deref() != Some(request_id.as_str())
    {
        return Err(invalid_data("bootstrap exchange response mismatch"));
    }
    let expected_operation = match operation {
        "bootstrap.exchange.offer.create" => "bootstrap.exchange.offer.created",
        "bootstrap.exchange.proof.create" => "bootstrap.exchange.proof.created",
        "bootstrap.exchange.brain_identity.read" => "bootstrap.exchange.brain_identity.read",
        _ => return Err(invalid_data("unsupported bootstrap exchange request")),
    };
    if response.operation != expected_operation {
        return Err(invalid_data("bootstrap exchange operation mismatch"));
    }
    Ok(DesktopBootstrapExchangeResult {
        accepted: response.accepted,
        error: response.error,
        transaction_id: response.transaction_id,
        exchange_message: response.exchange_message,
        brain_identity_json: response.brain_identity_json,
    })
}

/// Confirm the frozen approval on the original connection. The daemon consumes
/// the one-use grant and durably journals before returning the signed message.
pub fn desktop_confirm_bootstrap_approval(
    session_handle: String,
) -> io::Result<DesktopBootstrapApprovalResult> {
    finish_desktop_bootstrap_approval(session_handle, "bootstrap.approval.confirm")
}

/// Cancel a pending approval on the original connection. No signature is
/// generated and the daemon persists the cancellation before acknowledging it.
pub fn desktop_cancel_bootstrap_approval(
    session_handle: String,
) -> io::Result<DesktopBootstrapApprovalResult> {
    finish_desktop_bootstrap_approval(session_handle, "bootstrap.approval.cancel")
}

/// Submit the exact four-message transcript after the peer's signature has
/// returned. The daemon requires the matching released owner approval in its
/// journal before it creates the canonical local trust anchor.
pub fn desktop_complete_peer_approval(
    transcript_messages: Vec<String>,
    timeout: Duration,
) -> io::Result<DesktopBootstrapApprovalResult> {
    let started = Instant::now();
    let name = socket_name()?;
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
    arm_ipc_stream(&stream)?;
    let request_id = new_bootstrap_event_id().map_err(invalid_data)?;
    let request = BootstrapApprovalIpcRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: request_id.clone(),
        client_role: "desktop".into(),
        capabilities: vec![CAPABILITY_BOOTSTRAP_APPROVAL.into()],
        operation: "bootstrap.approval.complete".into(),
        role: None,
        transcript_messages: Some(transcript_messages),
        approval_request_digest: None,
        capability: None,
        transport_device_id: None,
        offer_message: None,
        brain_identity_json: None,
        transaction_id: None,
        owner_operation: None,
    };
    let response = exchange_bootstrap_approval(&mut stream, &request, Instant::now() + timeout)?;
    if response.protocol_version != PROTOCOL_VERSION
        || response.request_id.as_deref() != Some(request_id.as_str())
        || response.operation != "bootstrap.approval.anchored"
    {
        return Err(invalid_data("peer approval completion response mismatch"));
    }
    Ok(DesktopBootstrapApprovalResult {
        accepted: response.accepted,
        error: response.error,
        signed_message: None,
    })
}

fn finish_desktop_bootstrap_approval(
    session_handle: String,
    operation: &str,
) -> io::Result<DesktopBootstrapApprovalResult> {
    if session_handle.len() != 36
        || !session_handle
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
    {
        return Err(invalid_data("invalid approval session handle"));
    }
    let sessions = DESKTOP_BOOTSTRAP_APPROVAL_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut session = sessions
        .lock()
        .map_err(|_| invalid_data("approval session store unavailable"))?
        .remove(&session_handle)
        .ok_or_else(|| invalid_data("approval session expired or unavailable"))?;
    if session.expires_at <= Instant::now() {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "approval session expired",
        ));
    }
    let request = BootstrapApprovalIpcRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: session.request_id.clone(),
        client_role: "desktop".into(),
        capabilities: vec![CAPABILITY_BOOTSTRAP_APPROVAL.into()],
        operation: operation.into(),
        role: Some(session.role),
        transcript_messages: Some(session.transcript_messages),
        approval_request_digest: Some(session.approval_request_digest),
        capability: Some(session.capability),
        transport_device_id: None,
        offer_message: None,
        brain_identity_json: None,
        transaction_id: None,
        owner_operation: None,
    };
    let response = exchange_bootstrap_approval(
        &mut session.stream,
        &request,
        Instant::now() + IPC_RESPONSE_DEADLINE,
    )?;
    if response.protocol_version != PROTOCOL_VERSION
        || response.request_id.as_deref() != Some(session.request_id.as_str())
        || !matches!(
            response.operation.as_str(),
            "bootstrap.approval.signed"
                | "bootstrap.approval.cancelled"
                | "bootstrap.approval.result"
        )
    {
        return Err(invalid_data("approval response did not match the request"));
    }
    Ok(DesktopBootstrapApprovalResult {
        accepted: response.accepted,
        error: response.error,
        signed_message: response.signed_message,
    })
}

fn exchange_bootstrap_approval(
    stream: &mut Stream,
    request: &BootstrapApprovalIpcRequest,
    deadline: Instant,
) -> io::Result<BootstrapApprovalIpcResponseWire> {
    let mut bytes = serde_json::to_vec(request)?;
    bytes.push(b'\n');
    if bytes.len() > MAX_BOOTSTRAP_APPROVAL_FRAME_BYTES {
        return Err(invalid_data("approval request exceeds frame limit"));
    }
    write_ipc_all(stream, &bytes, deadline)?;
    let response = read_ipc_frame(stream, MAX_BOOTSTRAP_APPROVAL_FRAME_BYTES, deadline)?;
    Ok(serde_json::from_slice(&response)?)
}

fn exchange(
    stream: &mut Stream,
    request: &Request,
    max_response_bytes: usize,
) -> io::Result<Vec<u8>> {
    exchange_until(
        stream,
        request,
        max_response_bytes,
        Instant::now() + IPC_RESPONSE_DEADLINE,
    )
}

fn exchange_until(
    stream: &mut Stream,
    request: &Request,
    max_response_bytes: usize,
    deadline: Instant,
) -> io::Result<Vec<u8>> {
    arm_ipc_stream(stream)?;
    let mut request_frame = serde_json::to_vec(request)?;
    request_frame.push(b'\n');
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
    desktop_vault_workers: Option<&Arc<AtomicUsize>>,
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
        desktop_vault_workers,
        peer_is_desktop,
    )
}

fn handle_connection_for_peer(
    stream: &mut Stream,
    search: Option<&SearchContext>,
    desktop_search_workers: Option<&Arc<AtomicUsize>>,
    desktop_capture_workers: Option<&Arc<AtomicUsize>>,
    desktop_vault_workers: Option<&Arc<AtomicUsize>>,
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
    if !frame.is_empty() && is_bootstrap_approval_frame(&frame) {
        return handle_bootstrap_approval_session(stream, search, peer_is_desktop, &frame);
    }
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
                } else if request.operation == "content.timeline" {
                    let response_deadline = Instant::now() + IPC_RESPONSE_DEADLINE;
                    let reservation = desktop_search_workers.and_then(|workers| {
                        try_reserve_ipc_worker(workers, MAX_DESKTOP_SEARCH_WORKERS)
                    });
                    let response = if desktop_search_workers.is_some() && reservation.is_none() {
                        timeline_rejected(Some(request.request_id), "search_busy")
                    } else {
                        run_timeline_until(
                            request,
                            search.cloned(),
                            peer_is_desktop,
                            response_deadline,
                            reservation,
                        )
                    };
                    let bytes = serde_json::to_vec(&response)?;
                    if bytes.len() + 1 > MAX_SEARCH_RESPONSE_BYTES {
                        serde_json::to_vec(&timeline_rejected(
                            response.request_id,
                            "response_too_large",
                        ))?
                    } else {
                        bytes
                    }
                } else if request.operation == "content.event.detail" {
                    let response_deadline = Instant::now() + IPC_RESPONSE_DEADLINE;
                    let reservation = desktop_search_workers.and_then(|workers| {
                        try_reserve_ipc_worker(workers, MAX_DESKTOP_SEARCH_WORKERS)
                    });
                    let response = if desktop_search_workers.is_some() && reservation.is_none() {
                        event_detail_rejected(Some(request.request_id), "search_busy")
                    } else {
                        run_event_detail_until(
                            request,
                            search.cloned(),
                            peer_is_desktop,
                            response_deadline,
                            reservation,
                        )
                    };
                    let bytes = serde_json::to_vec(&response)?;
                    if bytes.len() + 1 > MAX_DETAIL_RESPONSE_BYTES {
                        serde_json::to_vec(&event_detail_rejected(
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
                } else if request.operation == "content.vault.rebuild" {
                    let reservation = desktop_vault_workers
                        .and_then(|workers| try_reserve_ipc_worker(workers, 1));
                    if desktop_vault_workers.is_some() && reservation.is_none() {
                        serde_json::to_vec(&vault_rejected(Some(request.request_id), "vault_busy"))?
                    } else {
                        let _reservation = reservation;
                        serde_json::to_vec(&handle_vault_rebuild(request, search, peer_is_desktop))?
                    }
                } else if request.operation == "identity.enroll_local" {
                    serde_json::to_vec(&handle_enrollment(request, search, peer_is_desktop))?
                } else if request.operation == "transport.status" {
                    serde_json::to_vec(&handle_transport_status(request, search, peer_is_desktop))?
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

fn is_bootstrap_approval_frame(frame: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(frame)
        .ok()
        .and_then(|value| value.get("operation")?.as_str().map(str::to_owned))
        .is_some_and(|operation| {
            operation.starts_with("bootstrap.") || operation.starts_with("owner.security.")
        })
}

fn approval_role(value: Option<&str>) -> Option<cryptex_events::BootstrapApprovalRole> {
    match value? {
        "initiator" => Some(cryptex_events::BootstrapApprovalRole::Initiator),
        "responder" => Some(cryptex_events::BootstrapApprovalRole::Responder),
        _ => None,
    }
}

fn bootstrap_approval_response(
    request_id: Option<String>,
    accepted: bool,
    error: Option<&'static str>,
    operation: &'static str,
    grant: Option<crate::approval_session::ApprovalSessionGrant>,
    snapshot: Option<crate::bootstrap_journal::ApprovalSnapshot>,
    signed_message: Option<String>,
) -> BootstrapApprovalIpcResponse {
    BootstrapApprovalIpcResponse {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        accepted,
        error,
        operation,
        approval_request_digest: grant
            .as_ref()
            .map(|grant| grant.approval_request_digest.clone()),
        capability: grant.map(|grant| grant.capability),
        snapshot,
        signed_message,
        exchange_message: None,
        transaction_id: None,
        brain_identity_json: None,
    }
}

fn bootstrap_exchange_response(
    request_id: Option<String>,
    accepted: bool,
    error: Option<&'static str>,
    operation: &'static str,
    transaction_id: Option<String>,
    exchange_message: Option<String>,
    brain_identity_json: Option<String>,
) -> BootstrapApprovalIpcResponse {
    BootstrapApprovalIpcResponse {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        accepted,
        error,
        operation,
        approval_request_digest: None,
        capability: None,
        snapshot: None,
        signed_message: None,
        exchange_message,
        transaction_id,
        brain_identity_json,
    }
}

fn handle_bootstrap_exchange(
    stream: &mut Stream,
    search: &SearchContext,
    peer_is_desktop: bool,
    request: BootstrapApprovalIpcRequest,
) -> io::Result<()> {
    if !peer_is_desktop
        || !approval_request_is_authorized(&request)
        || !bootstrap_exchange_request_is_well_shaped(&request)
    {
        let response = serde_json::to_vec(&bootstrap_exchange_response(
            Some(request.request_id),
            false,
            Some("bootstrap_exchange_request_rejected"),
            "bootstrap.exchange.result",
            None,
            None,
            None,
        ))?;
        return write_response(stream, &response, Instant::now() + IPC_REQUEST_DEADLINE);
    }

    if request.operation == "bootstrap.exchange.brain_identity.read" {
        let result = read_local_brain_identity(search);
        let (accepted, error, identity) = match result {
            Ok(identity) => (true, None, Some(identity)),
            Err(error) => (false, Some(error), None),
        };
        let response = serde_json::to_vec(&bootstrap_exchange_response(
            Some(request.request_id),
            accepted,
            error,
            "bootstrap.exchange.brain_identity.read",
            None,
            None,
            identity,
        ))?;
        return write_response(stream, &response, Instant::now() + IPC_RESPONSE_DEADLINE);
    }

    let result = match request.operation.as_str() {
        "bootstrap.exchange.offer.create" => create_or_resume_bootstrap_offer(search, &request),
        "bootstrap.exchange.proof.create" => create_or_resume_bootstrap_proof(search, &request),
        _ => Err("bootstrap_exchange_request_rejected"),
    };
    let (accepted, error, transaction_id, message, operation) = match result {
        Ok((transaction_id, message, operation)) => {
            (true, None, Some(transaction_id), Some(message), operation)
        }
        Err(error) => (
            false,
            Some(error),
            request.transaction_id,
            None,
            "bootstrap.exchange.result",
        ),
    };
    let response = serde_json::to_vec(&bootstrap_exchange_response(
        Some(request.request_id),
        accepted,
        error,
        operation,
        transaction_id,
        message,
        None,
    ))?;
    write_response(stream, &response, Instant::now() + IPC_RESPONSE_DEADLINE)
}

fn read_local_brain_identity(context: &SearchContext) -> Result<String, &'static str> {
    let path = crate::identity_runtime::brain_identity_path(&context.runtime_root);
    let metadata = fs::symlink_metadata(&path).map_err(|_| "local_identity_unavailable")?;
    if !metadata.file_type().is_file() || metadata.len() > 4096 {
        return Err("local_identity_unavailable");
    }
    let raw = fs::read_to_string(&path).map_err(|_| "local_identity_unavailable")?;
    let identity =
        cryptex_core::BrainIdentity::from_json(&raw).map_err(|_| "local_identity_unavailable")?;
    if identity
        .to_json_lf()
        .map_err(|_| "local_identity_unavailable")?
        != raw
    {
        return Err("local_identity_unavailable");
    }
    Ok(raw)
}

fn create_or_resume_bootstrap_offer(
    context: &SearchContext,
    request: &BootstrapApprovalIpcRequest,
) -> Result<(String, String, &'static str), &'static str> {
    let transaction_id = request
        .transaction_id
        .as_deref()
        .ok_or("bootstrap_exchange_request_rejected")?;
    let transport_id = request
        .transport_device_id
        .as_deref()
        .ok_or("bootstrap_exchange_request_rejected")?;
    require_current_transport_device_id(context, transport_id)?;
    let (brain_id, device_id, ready, local_identity) =
        load_local_bootstrap_exchange_identity(context)?;
    let journal = crate::bootstrap_journal::BootstrapJournalStore::new(&context.runtime_root)
        .map_err(|_| "bootstrap_journal_unavailable")?;
    if let Some(message) = journaled_local_offer(
        &journal,
        transaction_id,
        &brain_id,
        &local_identity,
        transport_id,
    )? {
        return Ok((
            transaction_id.to_owned(),
            message,
            "bootstrap.exchange.offer.created",
        ));
    }
    let mut nonce = [0_u8; 32];
    getrandom::getrandom(&mut nonce).map_err(|_| "secure_random_unavailable")?;
    let offer = cryptex_events::create_bootstrap_offer(
        &brain_id,
        transaction_id,
        &device_id,
        transport_id,
        &nonce,
        &ready.signing_key,
    )
    .map_err(|_| "bootstrap_offer_creation_failed")?;
    let event_id = new_bootstrap_event_id().map_err(|_| "secure_random_unavailable")?;
    if journal.record_initiator_offer(&offer, &event_id).is_err() {
        if let Some(message) = journaled_local_offer(
            &journal,
            transaction_id,
            &brain_id,
            &local_identity,
            transport_id,
        )? {
            return Ok((
                transaction_id.to_owned(),
                message,
                "bootstrap.exchange.offer.created",
            ));
        }
        return Err("bootstrap_exchange_conflict");
    }
    let message = String::from_utf8(offer).map_err(|_| "bootstrap_offer_serialization_failed")?;
    Ok((
        transaction_id.to_owned(),
        message,
        "bootstrap.exchange.offer.created",
    ))
}

fn journaled_local_offer(
    journal: &crate::bootstrap_journal::BootstrapJournalStore,
    transaction_id: &str,
    brain_id: &str,
    local_identity: &crate::bootstrap_journal::ApprovalIdentity,
    transport_id: &str,
) -> Result<Option<String>, &'static str> {
    let journal = journal
        .load()
        .map_err(|_| "bootstrap_journal_unavailable")?;
    if let Some(exchange) = journal
        .exchanges
        .iter()
        .find(|exchange| exchange.transaction_id == transaction_id)
    {
        if exchange.role != crate::bootstrap_journal::BootstrapExchangeRole::Initiator
            || exchange.brain_id != brain_id
            || &exchange.local_identity != local_identity
            || exchange.local_transport_device_id != transport_id
        {
            return Err("bootstrap_exchange_conflict");
        }
        return Ok(exchange.transcript_messages.first().cloned());
    }
    if let Some(transaction) = journal
        .transactions
        .iter()
        .find(|transaction| transaction.snapshot.transaction_id == transaction_id)
    {
        if transaction.snapshot.role != crate::bootstrap_journal::BootstrapRole::Initiator
            || transaction.snapshot.brain_id != brain_id
            || transaction.snapshot.local_identity != *local_identity
            || transaction
                .snapshot
                .transport_association
                .as_ref()
                .is_none_or(|association| association.local_device_id != transport_id)
        {
            return Err("bootstrap_exchange_conflict");
        }
        return Ok(transaction.transcript_messages.first().cloned());
    }
    Ok(None)
}

fn create_or_resume_bootstrap_proof(
    context: &SearchContext,
    request: &BootstrapApprovalIpcRequest,
) -> Result<(String, String, &'static str), &'static str> {
    let offer = request
        .offer_message
        .as_deref()
        .ok_or("bootstrap_exchange_request_rejected")?;
    let transport_id = request
        .transport_device_id
        .as_deref()
        .ok_or("bootstrap_exchange_request_rejected")?;
    require_current_transport_device_id(context, transport_id)?;
    let verified_offer = cryptex_events::verify_bootstrap_offer(offer.as_bytes())
        .map_err(|_| "invalid_signed_offer")?;
    let brain_identity_json = request
        .brain_identity_json
        .as_deref()
        .ok_or("bootstrap_exchange_request_rejected")?;
    if brain_identity_json.len() > 4096 {
        return Err("brain_identity_too_large");
    }
    crate::local_device_identity::join_existing_brain(
        &context.runtime_root,
        context.store.as_ref(),
        context.environment,
        brain_identity_json,
        verified_offer.brain_id(),
    )
    .map_err(|error| match error {
        crate::local_device_identity::LocalDeviceIdentityError::BrainMismatch => "brain_mismatch",
        crate::local_device_identity::LocalDeviceIdentityError::ExistingDeviceState => {
            "existing_device_state"
        }
        crate::local_device_identity::LocalDeviceIdentityError::EventSigningKey(_) => {
            "local_signing_identity_unavailable"
        }
        _ => "local_identity_setup_failed",
    })?;
    let (brain_id, device_id, ready, local_identity) =
        load_local_bootstrap_exchange_identity(context)?;
    if verified_offer.brain_id() != brain_id {
        return Err("brain_mismatch");
    }
    let transaction_id = verified_offer.transaction_id().to_owned();
    let journal = crate::bootstrap_journal::BootstrapJournalStore::new(&context.runtime_root)
        .map_err(|_| "bootstrap_journal_unavailable")?;
    if let Some(message) = journaled_local_proof(
        &journal,
        &transaction_id,
        offer.as_bytes(),
        &local_identity,
        transport_id,
    )? {
        return Ok((transaction_id, message, "bootstrap.exchange.proof.created"));
    }

    let event_id = new_bootstrap_event_id().map_err(|_| "secure_random_unavailable")?;
    journal
        .record_responder_offer(
            offer.as_bytes(),
            local_identity.clone(),
            transport_id.to_owned(),
            &event_id,
        )
        .map_err(|_| "bootstrap_exchange_conflict")?;
    let mut nonce = [0_u8; 32];
    getrandom::getrandom(&mut nonce).map_err(|_| "secure_random_unavailable")?;
    let proof = cryptex_events::create_bootstrap_proof(
        offer.as_bytes(),
        &device_id,
        transport_id,
        &nonce,
        &ready.signing_key,
    )
    .map_err(|_| "bootstrap_proof_creation_failed")?;
    if journal
        .record_exchange_proof(&transaction_id, &proof)
        .is_err()
    {
        if let Some(message) = journaled_local_proof(
            &journal,
            &transaction_id,
            offer.as_bytes(),
            &local_identity,
            transport_id,
        )? {
            return Ok((transaction_id, message, "bootstrap.exchange.proof.created"));
        }
        return Err("bootstrap_exchange_conflict");
    }
    let message = String::from_utf8(proof).map_err(|_| "bootstrap_proof_serialization_failed")?;
    Ok((transaction_id, message, "bootstrap.exchange.proof.created"))
}

fn require_current_transport_device_id(
    context: &SearchContext,
    supplied: &str,
) -> Result<(), &'static str> {
    #[cfg(test)]
    {
        let actual = context
            .transport_device_id_override
            .as_deref()
            .ok_or("transport_identity_unavailable")?;
        if !crate::syncthing_rest::native_id_shape(actual) {
            return Err("transport_identity_unavailable");
        }
        if actual != supplied {
            return Err("transport_identity_mismatch");
        }
        Ok(())
    }
    #[cfg(all(windows, not(test)))]
    {
        let reader = context
            .transport_status
            .as_ref()
            .ok_or("transport_identity_unavailable")?;
        let actual = reader
            .try_transport_device_id()
            .map_err(|_| "transport_identity_unavailable")?;
        if actual != supplied {
            return Err("transport_identity_mismatch");
        }
        Ok(())
    }
    #[cfg(all(not(windows), not(test)))]
    {
        let _ = (context, supplied);
        Err("transport_identity_unavailable")
    }
}

fn journaled_local_proof(
    store: &crate::bootstrap_journal::BootstrapJournalStore,
    transaction_id: &str,
    offer: &[u8],
    local_identity: &crate::bootstrap_journal::ApprovalIdentity,
    transport_id: &str,
) -> Result<Option<String>, &'static str> {
    let journal = store.load().map_err(|_| "bootstrap_journal_unavailable")?;
    if let Some(exchange) = journal
        .exchanges
        .iter()
        .find(|exchange| exchange.transaction_id == transaction_id)
    {
        if exchange.role != crate::bootstrap_journal::BootstrapExchangeRole::Responder
            || exchange.local_identity != *local_identity
            || exchange.local_transport_device_id != transport_id
            || exchange.transcript_messages.first().map(String::as_bytes) != Some(offer)
        {
            return Err("bootstrap_exchange_conflict");
        }
        return Ok(exchange.transcript_messages.get(1).cloned());
    }
    if let Some(transaction) = journal
        .transactions
        .iter()
        .find(|transaction| transaction.snapshot.transaction_id == transaction_id)
    {
        if transaction.snapshot.role != crate::bootstrap_journal::BootstrapRole::Responder
            || transaction.snapshot.local_identity != *local_identity
            || transaction
                .snapshot
                .transport_association
                .as_ref()
                .is_none_or(|association| association.local_device_id != transport_id)
            || transaction
                .transcript_messages
                .first()
                .map(String::as_bytes)
                != Some(offer)
        {
            return Err("bootstrap_exchange_conflict");
        }
        return Ok(transaction.transcript_messages.get(1).cloned());
    }
    Ok(None)
}

fn load_local_bootstrap_exchange_identity(
    context: &SearchContext,
) -> Result<
    (
        String,
        DeviceId,
        crate::event_signing_key::ReadyEventSigningKey,
        crate::bootstrap_journal::ApprovalIdentity,
    ),
    &'static str,
> {
    let brain =
        load_brain_identity(&context.runtime_root).map_err(|_| "local_identity_unavailable")?;
    let local = load_local_device_identity(&context.runtime_root, &brain.brain_id)
        .map_err(|_| "local_identity_unavailable")?;
    let device = DeviceId::new(local.device_id).map_err(|_| "local_identity_unavailable")?;
    let ready = load_existing_event_signing_key(
        context.store.as_ref(),
        context.environment,
        &context.runtime_root,
        &device,
    )
    .map_err(|_| "local_signing_identity_unavailable")?;
    let verifying_key = ready.signing_key.verifying_key();
    let fingerprint = Sha256::digest(verifying_key.as_bytes());
    let identity = crate::bootstrap_journal::ApprovalIdentity {
        device_id: device.as_str().to_owned(),
        signing_key_id: ready.public.signing_key_id.clone(),
        public_key_fingerprint_sha256: blake3::Hash::from_bytes(fingerprint.into())
            .to_hex()
            .to_string(),
    };
    if ready.public.device_id != device.as_str()
        || ready.public.signing_key_id
            != cryptex_events::signing_key_id_from_verifying_key(&verifying_key)
                .map_err(|_| "local_signing_identity_unavailable")?
                .as_str()
    {
        return Err("local_signing_identity_mismatch");
    }
    Ok((brain.brain_id, device, ready, identity))
}

fn approval_request_is_authorized(request: &BootstrapApprovalIpcRequest) -> bool {
    request.protocol_version == PROTOCOL_VERSION
        && request.client_role == "desktop"
        && request.request_id.len() <= 128
        && !request.request_id.is_empty()
        && request
            .request_id
            .bytes()
            .all(|byte| byte.is_ascii_graphic())
        && request.capabilities.len() == 1
        && request.capabilities[0] == CAPABILITY_BOOTSTRAP_APPROVAL
}

fn approval_request_has_prepare_shape(request: &BootstrapApprovalIpcRequest) -> bool {
    request.role.is_some()
        && request.transcript_messages.is_some()
        && request.approval_request_digest.is_none()
        && request.capability.is_none()
        && request.transport_device_id.is_none()
        && request.offer_message.is_none()
        && request.brain_identity_json.is_none()
        && request.transaction_id.is_none()
}

fn approval_request_has_followup_shape(request: &BootstrapApprovalIpcRequest) -> bool {
    request.role.is_some()
        && request.transcript_messages.is_some()
        && request
            .approval_request_digest
            .as_deref()
            .is_some_and(|digest| {
                digest.len() == 64
                    && digest
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            })
        && request.capability.as_deref().is_some_and(|capability| {
            capability.len() == 64
                && capability
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        })
        && request.transport_device_id.is_none()
        && request.offer_message.is_none()
        && request.brain_identity_json.is_none()
        && request.transaction_id.is_none()
}

fn owner_security_prepare_has_shape(request: &BootstrapApprovalIpcRequest) -> bool {
    request.role.is_none()
        && request.transcript_messages.is_none()
        && request.approval_request_digest.is_none()
        && request.capability.is_none()
        && request.transport_device_id.is_none()
        && request.offer_message.is_none()
        && request.brain_identity_json.is_none()
        && request.transaction_id.as_deref().is_some_and(|id| {
            id.len() == 64
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        && request.owner_operation.is_some()
}

fn owner_security_followup_has_shape(request: &BootstrapApprovalIpcRequest) -> bool {
    request.role.is_none()
        && request.transcript_messages.is_none()
        && request.owner_operation.is_none()
        && request.transport_device_id.is_none()
        && request.offer_message.is_none()
        && request.brain_identity_json.is_none()
        && request.transaction_id.as_deref().is_some_and(|id| {
            id.len() == 64
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        && request
            .approval_request_digest
            .as_deref()
            .is_some_and(is_lower_hex_digest)
        && request
            .capability
            .as_deref()
            .is_some_and(is_lower_hex_digest)
}

fn is_lower_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn bootstrap_exchange_request_is_well_shaped(request: &BootstrapApprovalIpcRequest) -> bool {
    if request.role.is_some()
        || request.transcript_messages.is_some()
        || request.approval_request_digest.is_some()
        || request.capability.is_some()
    {
        return false;
    }
    match request.operation.as_str() {
        "bootstrap.exchange.brain_identity.read" => {
            request.transport_device_id.is_none()
                && request.offer_message.is_none()
                && request.brain_identity_json.is_none()
                && request.transaction_id.is_none()
        }
        "bootstrap.exchange.offer.create" => {
            request.transport_device_id.is_some()
                && request.offer_message.is_none()
                && request.brain_identity_json.is_none()
                && request.transaction_id.as_deref().is_some_and(|id| {
                    id.len() == 64
                        && id
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                })
        }
        "bootstrap.exchange.proof.create" => {
            request.transport_device_id.is_some()
                && request
                    .offer_message
                    .as_ref()
                    .is_some_and(|offer| offer.len() <= 64 * 1024)
                && request
                    .brain_identity_json
                    .as_ref()
                    .is_some_and(|identity| identity.len() <= 4096)
                && request.transaction_id.is_none()
        }
        _ => false,
    }
}

fn verify_approval_prefix(
    request: &BootstrapApprovalIpcRequest,
) -> Result<
    (
        cryptex_events::VerifiedBootstrapApprovalRequest,
        Vec<Vec<u8>>,
    ),
    &'static str,
> {
    let role = approval_role(request.role.as_deref()).ok_or("invalid_approval_role")?;
    let messages = request
        .transcript_messages
        .as_ref()
        .ok_or("missing_transcript")?
        .iter()
        .map(|message| message.as_bytes().to_vec())
        .collect::<Vec<_>>();
    if messages.iter().any(|message| message.len() > 64 * 1024) {
        return Err("transcript_too_large");
    }
    let refs = messages.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let verified = cryptex_events::verify_bootstrap_approval_request(&refs, role)
        .map_err(|_| "invalid_transcript")?;
    Ok((verified, messages))
}

fn validate_local_bootstrap_identity(
    context: &SearchContext,
    request: &cryptex_events::VerifiedBootstrapApprovalRequest,
) -> Result<DeviceId, &'static str> {
    let brain =
        load_brain_identity(&context.runtime_root).map_err(|_| "local_identity_unavailable")?;
    if brain.brain_id != request.brain_id() {
        return Err("brain_mismatch");
    }
    let local = load_local_device_identity(&context.runtime_root, &brain.brain_id)
        .map_err(|_| "local_identity_unavailable")?;
    let device = DeviceId::new(local.device_id).map_err(|_| "local_identity_unavailable")?;
    if device.as_str() != request.local_identity().device_id.as_str() {
        return Err("local_device_mismatch");
    }
    let (metadata, verifying_key) = load_existing_event_verifying_key(
        context.store.as_ref(),
        context.environment,
        &context.runtime_root,
        &device,
    )
    .map_err(|_| "local_signing_identity_unavailable")?;
    if metadata.signing_key_id != request.local_identity().signing_key_id.as_str()
        || verifying_key.to_bytes() != request.local_identity().public_key
    {
        return Err("local_signing_identity_mismatch");
    }
    Ok(device)
}

fn new_bootstrap_event_id() -> Result<String, &'static str> {
    let mut bytes = [0_u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|_| "secure_random_unavailable")?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

fn new_bootstrap_transaction_id() -> Result<String, &'static str> {
    let mut bytes = [0_u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| "secure_random_unavailable")?;
    Ok(blake3::Hash::from_bytes(bytes).to_hex().to_string())
}

fn handle_bootstrap_approval_session(
    stream: &mut Stream,
    search: Option<&SearchContext>,
    peer_is_desktop: bool,
    first_frame: &[u8],
) -> io::Result<()> {
    if !peer_is_desktop || first_frame.len() > MAX_BOOTSTRAP_APPROVAL_FRAME_BYTES {
        let response = serde_json::to_vec(&bootstrap_approval_response(
            None,
            false,
            Some("desktop_authentication_required"),
            "bootstrap.approval.result",
            None,
            None,
            None,
        ))?;
        return write_response(stream, &response, Instant::now() + IPC_REQUEST_DEADLINE);
    }
    let context = search.ok_or_else(|| invalid_data("approval service unavailable"))?;
    let request: BootstrapApprovalIpcRequest = match serde_json::from_slice(first_frame) {
        Ok(request) => request,
        Err(_) => {
            let response = serde_json::to_vec(&bootstrap_approval_response(
                None,
                false,
                Some("malformed_request"),
                "bootstrap.approval.result",
                None,
                None,
                None,
            ))?;
            return write_response(stream, &response, Instant::now() + IPC_REQUEST_DEADLINE);
        }
    };
    if request.operation.starts_with("owner.security.") {
        return handle_owner_security_operation_session(stream, context, peer_is_desktop, request);
    }
    if request.operation.starts_with("bootstrap.exchange.") {
        return handle_bootstrap_exchange(stream, context, peer_is_desktop, request);
    }
    if request.operation == "bootstrap.approval.complete" {
        return handle_bootstrap_approval_completion(stream, context, peer_is_desktop, request);
    }
    if !approval_request_is_authorized(&request)
        || !approval_request_has_prepare_shape(&request)
        || request.operation != "bootstrap.approval.prepare"
    {
        let response = serde_json::to_vec(&bootstrap_approval_response(
            Some(request.request_id),
            false,
            Some("approval_request_rejected"),
            "bootstrap.approval.result",
            None,
            None,
            None,
        ))?;
        return write_response(stream, &response, Instant::now() + IPC_REQUEST_DEADLINE);
    }
    let (verified, messages) = match verify_approval_prefix(&request) {
        Ok(prefix) => prefix,
        Err(error) => {
            let response = serde_json::to_vec(&bootstrap_approval_response(
                Some(request.request_id),
                false,
                Some(error),
                "bootstrap.approval.result",
                None,
                None,
                None,
            ))?;
            return write_response(stream, &response, Instant::now() + IPC_REQUEST_DEADLINE);
        }
    };
    if let Err(error) = validate_local_bootstrap_identity(context, &verified) {
        let response = serde_json::to_vec(&bootstrap_approval_response(
            Some(request.request_id),
            false,
            Some(error),
            "bootstrap.approval.result",
            None,
            None,
            None,
        ))?;
        return write_response(stream, &response, Instant::now() + IPC_REQUEST_DEADLINE);
    }

    let snapshot =
        crate::bootstrap_journal::ApprovalSnapshot::from_verified_bootstrap_request(&verified)
            .map_err(|_| invalid_data("approval snapshot invalid"))?;
    let journal = crate::bootstrap_journal::BootstrapJournalStore::new(&context.runtime_root)
        .map_err(|_| invalid_data("approval journal unavailable"))?;
    let event_id = new_bootstrap_event_id().map_err(invalid_data)?;
    if journal
        .begin_verified_approval_prefix(&verified, &messages, &event_id)
        .is_err()
    {
        let response = serde_json::to_vec(&bootstrap_approval_response(
            Some(request.request_id),
            false,
            Some("approval_transaction_conflict"),
            "bootstrap.approval.result",
            None,
            None,
            None,
        ))?;
        return write_response(stream, &response, Instant::now() + IPC_REQUEST_DEADLINE);
    }

    let registry = BOOTSTRAP_APPROVAL_SESSIONS
        .get_or_init(crate::approval_session::ApprovalSessionRegistry::default);
    let connection = registry
        .open_connection()
        .map_err(|_| invalid_data("approval session unavailable"))?;
    let grant = registry
        .issue(&connection, snapshot.clone())
        .map_err(|_| invalid_data("approval session unavailable"))?;
    let prepared = serde_json::to_vec(&bootstrap_approval_response(
        Some(request.request_id.clone()),
        true,
        None,
        "bootstrap.approval.prepared",
        Some(grant.clone()),
        Some(snapshot),
        None,
    ))?;
    write_response(stream, &prepared, Instant::now() + IPC_RESPONSE_DEADLINE)?;

    let followup_frame = match read_ipc_frame(
        stream,
        MAX_BOOTSTRAP_APPROVAL_FRAME_BYTES,
        Instant::now() + IPC_APPROVAL_SESSION_DEADLINE,
    ) {
        Ok(frame) if !frame.is_empty() => frame,
        Ok(_) => return Ok(()),
        Err(error) if error.kind() == io::ErrorKind::TimedOut => return Ok(()),
        Err(error) => return Err(error),
    };
    let followup: BootstrapApprovalIpcRequest = match serde_json::from_slice(&followup_frame) {
        Ok(request) => request,
        Err(_) => {
            let response = serde_json::to_vec(&bootstrap_approval_response(
                Some(request.request_id),
                false,
                Some("malformed_confirmation"),
                "bootstrap.approval.result",
                None,
                None,
                None,
            ))?;
            return write_response(stream, &response, Instant::now() + IPC_REQUEST_DEADLINE);
        }
    };
    if !approval_request_is_authorized(&followup)
        || !approval_request_has_followup_shape(&followup)
        || followup.request_id != request.request_id
        || followup.role != request.role
    {
        let response = serde_json::to_vec(&bootstrap_approval_response(
            Some(request.request_id),
            false,
            Some("approval_session_mismatch"),
            "bootstrap.approval.result",
            None,
            None,
            None,
        ))?;
        return write_response(stream, &response, Instant::now() + IPC_REQUEST_DEADLINE);
    }
    let capability = followup.capability.as_deref().unwrap_or_default();
    let digest = followup
        .approval_request_digest
        .as_deref()
        .unwrap_or_default();
    let (confirm_request, _) = match verify_approval_prefix(&followup) {
        Ok(prefix) => prefix,
        Err(error) => {
            let response = serde_json::to_vec(&bootstrap_approval_response(
                Some(request.request_id),
                false,
                Some(error),
                "bootstrap.approval.result",
                None,
                None,
                None,
            ))?;
            return write_response(stream, &response, Instant::now() + IPC_REQUEST_DEADLINE);
        }
    };
    if confirm_request != verified
        || followup.approval_request_digest.as_deref()
            != Some(grant.approval_request_digest.as_str())
    {
        let response = serde_json::to_vec(&bootstrap_approval_response(
            Some(request.request_id),
            false,
            Some("approval_snapshot_changed"),
            "bootstrap.approval.result",
            None,
            None,
            None,
        ))?;
        return write_response(stream, &response, Instant::now() + IPC_REQUEST_DEADLINE);
    }
    if followup.operation == "bootstrap.approval.cancel" {
        let result = registry
            .cancel(&connection, capability)
            .map_err(|_| invalid_data("approval cancellation rejected"))
            .and_then(|()| {
                journal
                    .cancel_pending_approval(verified.transaction_id())
                    .map_err(|_| invalid_data("approval cancellation could not be persisted"))
            });
        let response = serde_json::to_vec(&bootstrap_approval_response(
            Some(request.request_id),
            result.is_ok(),
            result.err().map(|_| "approval_cancellation_rejected"),
            "bootstrap.approval.cancelled",
            None,
            None,
            None,
        ))?;
        return write_response(stream, &response, Instant::now() + IPC_RESPONSE_DEADLINE);
    }
    if followup.operation != "bootstrap.approval.confirm" {
        let response = serde_json::to_vec(&bootstrap_approval_response(
            Some(request.request_id),
            false,
            Some("invalid_approval_action"),
            "bootstrap.approval.result",
            None,
            None,
            None,
        ))?;
        return write_response(stream, &response, Instant::now() + IPC_REQUEST_DEADLINE);
    }
    let signed = registry.confirm_bootstrap_approval_journaled(
        &connection,
        capability,
        digest,
        &confirm_request,
        &journal,
        |verified_request| {
            let brain = load_brain_identity(&context.runtime_root).map_err(|_| {
                crate::approval_session::ApprovalSessionError::ApprovalSigningFailed
            })?;
            let local = load_local_device_identity(&context.runtime_root, &brain.brain_id)
                .map_err(|_| {
                    crate::approval_session::ApprovalSessionError::ApprovalSigningFailed
                })?;
            let device = DeviceId::new(local.device_id).map_err(|_| {
                crate::approval_session::ApprovalSessionError::ApprovalSigningFailed
            })?;
            let ready = load_existing_event_signing_key(
                context.store.as_ref(),
                context.environment,
                &context.runtime_root,
                &device,
            )
            .map_err(|_| crate::approval_session::ApprovalSessionError::ApprovalSigningFailed)?;
            if brain.brain_id != verified_request.brain_id()
                || device.as_str() != verified_request.local_identity().device_id.as_str()
                || ready.public.signing_key_id
                    != verified_request.local_identity().signing_key_id.as_str()
                || ready.signing_key.verifying_key().to_bytes()
                    != verified_request.local_identity().public_key
            {
                return Err(crate::approval_session::ApprovalSessionError::ApprovalSigningFailed);
            }
            cryptex_events::sign_bootstrap_approval(verified_request, &ready.signing_key)
                .map_err(|_| crate::approval_session::ApprovalSessionError::ApprovalSigningFailed)
        },
    );
    let response = match signed {
        Ok(bytes) => {
            let signed_message = String::from_utf8(bytes)
                .map_err(|_| invalid_data("signed approval encoding invalid"))?;
            bootstrap_approval_response(
                Some(request.request_id),
                true,
                None,
                "bootstrap.approval.signed",
                None,
                None,
                Some(signed_message),
            )
        }
        Err(error) => bootstrap_approval_response(
            Some(request.request_id),
            false,
            Some(match error {
                crate::approval_session::ApprovalSessionError::ApprovalJournalFailed => {
                    "approval_journal_failed"
                }
                crate::approval_session::ApprovalSessionError::SnapshotMismatch => {
                    "approval_snapshot_changed"
                }
                _ => "approval_confirmation_failed",
            }),
            "bootstrap.approval.result",
            None,
            None,
            None,
        ),
    };
    let response = serde_json::to_vec(&response)?;
    write_response(stream, &response, Instant::now() + IPC_RESPONSE_DEADLINE)
}

fn write_owner_security_rejection(
    stream: &mut Stream,
    request_id: &str,
    error: &'static str,
) -> io::Result<()> {
    let bytes = serde_json::to_vec(&bootstrap_approval_response(
        Some(request_id.to_owned()),
        false,
        Some(error),
        "owner.security.result",
        None,
        None,
        None,
    ))?;
    write_response(stream, &bytes, Instant::now() + IPC_REQUEST_DEADLINE)
}

fn handle_owner_security_operation_session(
    stream: &mut Stream,
    context: &SearchContext,
    peer_is_desktop: bool,
    request: BootstrapApprovalIpcRequest,
) -> io::Result<()> {
    let reply = |accepted, error, operation, grant, snapshot| {
        bootstrap_approval_response(
            Some(request.request_id.clone()),
            accepted,
            error,
            operation,
            grant,
            snapshot,
            None,
        )
    };
    if cfg!(target_os = "macos") {
        return write_owner_security_rejection(
            stream,
            &request.request_id,
            "platform_peer_identity_unavailable",
        );
    }
    if !peer_is_desktop || !approval_request_is_authorized(&request) {
        return write_owner_security_rejection(
            stream,
            &request.request_id,
            "desktop_authentication_required",
        );
    }
    if request.operation != "owner.security.prepare" || !owner_security_prepare_has_shape(&request)
    {
        return write_owner_security_rejection(
            stream,
            &request.request_id,
            "owner_operation_request_rejected",
        );
    }
    let transaction_id = request.transaction_id.as_deref().unwrap_or_default();
    let (operation, target, reason, cutoff) = match request.owner_operation.as_ref() {
        Some(OwnerSecurityOperationRequest::DeviceRestriction {
            target_device_id,
            reason,
            cutoff_seq,
        }) => (
            crate::bootstrap_journal::BootstrapOperation::DeviceRestriction,
            Some(target_device_id.as_str()),
            Some(*reason),
            Some(*cutoff_seq),
        ),
        Some(OwnerSecurityOperationRequest::SelfRetirement) => (
            crate::bootstrap_journal::BootstrapOperation::SelfRetirement,
            None,
            None,
            None,
        ),
        None => {
            return write_owner_security_rejection(
                stream,
                &request.request_id,
                "owner_operation_request_rejected",
            );
        }
    };
    let brain = match load_brain_identity(&context.runtime_root) {
        Ok(brain) => brain,
        Err(_) => {
            return write_owner_security_rejection(
                stream,
                &request.request_id,
                "local_identity_unavailable",
            );
        }
    };
    let snapshot = match crate::knowledge_materialization::build_owner_security_operation_snapshot(
        &context.runtime_root,
        &brain.brain_id,
        context.store.as_ref(),
        context.environment,
        crate::knowledge_materialization::OwnerSecuritySnapshotRequest {
            transaction_id: transaction_id.to_owned(),
            operation,
            target_device_id: target,
            reason,
            cutoff_seq: cutoff,
        },
    ) {
        Ok(snapshot) => snapshot,
        Err(_) => {
            return write_owner_security_rejection(
                stream,
                &request.request_id,
                "owner_operation_policy_held",
            );
        }
    };
    let journal = match crate::bootstrap_journal::BootstrapJournalStore::new(&context.runtime_root)
    {
        Ok(journal) => journal,
        Err(_) => {
            return write_owner_security_rejection(
                stream,
                &request.request_id,
                "approval_journal_unavailable",
            );
        }
    };
    let existing = match journal.load() {
        Ok(journal) => journal
            .transactions
            .into_iter()
            .find(|transaction| transaction.snapshot.transaction_id == transaction_id),
        Err(_) => {
            return write_owner_security_rejection(
                stream,
                &request.request_id,
                "approval_journal_unavailable",
            );
        }
    };
    if let Some(existing) = existing {
        if existing.snapshot != snapshot
            || existing.state != crate::bootstrap_journal::BootstrapState::OwnerPending
        {
            return write_owner_security_rejection(
                stream,
                &request.request_id,
                "approval_transaction_conflict",
            );
        }
    } else {
        let event_id = match new_bootstrap_event_id() {
            Ok(event_id) => event_id,
            Err(error) => {
                return write_owner_security_rejection(stream, &request.request_id, error);
            }
        };
        if journal
            .begin_owner_operation(snapshot.clone(), &event_id)
            .is_err()
        {
            return write_owner_security_rejection(
                stream,
                &request.request_id,
                "approval_transaction_conflict",
            );
        }
    }
    let registry = BOOTSTRAP_APPROVAL_SESSIONS
        .get_or_init(crate::approval_session::ApprovalSessionRegistry::default);
    let connection = match registry.open_connection() {
        Ok(connection) => connection,
        Err(_) => {
            return write_owner_security_rejection(
                stream,
                &request.request_id,
                "approval_session_unavailable",
            );
        }
    };
    let grant = match registry.issue(&connection, snapshot.clone()) {
        Ok(grant) => grant,
        Err(_) => {
            return write_owner_security_rejection(
                stream,
                &request.request_id,
                "approval_session_unavailable",
            );
        }
    };
    let prepared = serde_json::to_vec(&reply(
        true,
        None,
        "owner.security.prepared",
        Some(grant.clone()),
        Some(snapshot.clone()),
    ))?;
    write_response(stream, &prepared, Instant::now() + IPC_RESPONSE_DEADLINE)?;

    let followup_frame = match read_ipc_frame(
        stream,
        MAX_BOOTSTRAP_APPROVAL_FRAME_BYTES,
        Instant::now() + IPC_APPROVAL_SESSION_DEADLINE,
    ) {
        Ok(frame) if !frame.is_empty() => frame,
        Ok(_) => return Ok(()),
        Err(error) if error.kind() == io::ErrorKind::TimedOut => return Ok(()),
        Err(error) => return Err(error),
    };
    let followup: BootstrapApprovalIpcRequest = match serde_json::from_slice(&followup_frame) {
        Ok(request) => request,
        Err(_) => {
            return write_owner_security_rejection(
                stream,
                &request.request_id,
                "malformed_confirmation",
            );
        }
    };
    if !approval_request_is_authorized(&followup)
        || !owner_security_followup_has_shape(&followup)
        || followup.request_id != request.request_id
        || followup.transaction_id.as_deref() != Some(transaction_id)
        || !matches!(
            followup.operation.as_str(),
            "owner.security.confirm" | "owner.security.cancel"
        )
    {
        return write_owner_security_rejection(
            stream,
            &request.request_id,
            "approval_session_mismatch",
        );
    }
    if !peer_process_id(stream)
        .ok()
        .is_some_and(|pid| context.expected_desktop.matches_peer(pid))
    {
        return write_owner_security_rejection(
            stream,
            &request.request_id,
            "desktop_authentication_required",
        );
    }
    let capability = followup.capability.as_deref().unwrap_or_default();
    let digest = followup
        .approval_request_digest
        .as_deref()
        .unwrap_or_default();
    match registry.consume(&connection, capability, digest) {
        Ok(consumed) if consumed == snapshot => {}
        Ok(_) => {
            return write_owner_security_rejection(
                stream,
                &request.request_id,
                "approval_snapshot_changed",
            );
        }
        Err(_) => {
            return write_owner_security_rejection(
                stream,
                &request.request_id,
                "approval_confirmation_failed",
            );
        }
    }
    if followup.operation == "owner.security.cancel" {
        let result = journal.cancel_pending_approval(transaction_id);
        let response = serde_json::to_vec(&reply(
            result.is_ok(),
            result.err().map(|_| "approval_cancellation_rejected"),
            "owner.security.cancelled",
            None,
            None,
        ))?;
        return write_response(stream, &response, Instant::now() + IPC_RESPONSE_DEADLINE);
    }
    if journal
        .confirm_owner_operation(transaction_id, digest)
        .is_err()
    {
        return write_owner_security_rejection(
            stream,
            &request.request_id,
            "approval_journal_failed",
        );
    }
    let result = crate::knowledge_materialization::append_confirmed_owner_security_operation(
        &context.runtime_root,
        &brain.brain_id,
        context.store.as_ref(),
        context.environment,
        transaction_id,
    );
    let response = serde_json::to_vec(&reply(
        result.is_ok(),
        result.err().map(|_| "owner_operation_append_held"),
        "owner.security.result",
        None,
        None,
    ))?;
    write_response(stream, &response, Instant::now() + IPC_RESPONSE_DEADLINE)
}

fn handle_bootstrap_approval_completion(
    stream: &mut Stream,
    context: &SearchContext,
    peer_is_desktop: bool,
    request: BootstrapApprovalIpcRequest,
) -> io::Result<()> {
    let reject = |error| {
        serde_json::to_vec(&bootstrap_approval_response(
            Some(request.request_id.clone()),
            false,
            Some(error),
            "bootstrap.approval.anchored",
            None,
            None,
            None,
        ))
    };
    // macOS currently exposes only PID-level peer credentials in this IPC
    // adapter. Do not let a previously confirmed transcript create a durable
    // trust anchor until the platform can bind the completion connection to
    // the same live Desktop process instance.
    if cfg!(target_os = "macos") {
        let response = reject("platform_peer_identity_unavailable")?;
        return write_response(stream, &response, Instant::now() + IPC_RESPONSE_DEADLINE);
    }
    let response = if !peer_is_desktop
        || !approval_request_is_authorized(&request)
        || request.operation != "bootstrap.approval.complete"
        || request.role.is_some()
        || request.approval_request_digest.is_some()
        || request.capability.is_some()
    {
        reject("approval_request_rejected")?
    } else {
        let Some(transcript_messages) = request.transcript_messages.as_ref() else {
            let bytes = reject("approval_request_rejected")?;
            return write_response(stream, &bytes, Instant::now() + IPC_RESPONSE_DEADLINE);
        };
        let messages = transcript_messages
            .iter()
            .map(|message| message.as_bytes().to_vec())
            .collect::<Vec<_>>();
        let verified =
            if messages.len() == 4 && messages.iter().all(|message| message.len() <= 64 * 1024) {
                let refs = messages.iter().map(Vec::as_slice).collect::<Vec<_>>();
                cryptex_events::verify_bootstrap_transcript(&refs).ok()
            } else {
                None
            };
        let Some(verified) = verified else {
            let bytes = reject("invalid_transcript")?;
            return write_response(stream, &bytes, Instant::now() + IPC_RESPONSE_DEADLINE);
        };
        let journal =
            match crate::bootstrap_journal::BootstrapJournalStore::new(&context.runtime_root) {
                Ok(journal) => journal,
                Err(_) => {
                    let bytes = reject("approval_journal_unavailable")?;
                    return write_response(stream, &bytes, Instant::now() + IPC_RESPONSE_DEADLINE);
                }
            };
        if journal
            .record_verified_complete_transcript(verified.transaction_id(), &messages)
            .is_err()
        {
            reject("approval_transaction_conflict")?
        } else if crate::knowledge_materialization::append_local_peer_approval_anchor(
            &context.runtime_root,
            verified.brain_id(),
            context.store.as_ref(),
            context.environment,
            verified.transaction_id(),
            &messages,
        )
        .is_err()
        {
            reject("approval_anchor_held")?
        } else {
            serde_json::to_vec(&bootstrap_approval_response(
                Some(request.request_id.clone()),
                true,
                None,
                "bootstrap.approval.anchored",
                None,
                None,
                None,
            ))?
        }
    };
    write_response(stream, &response, Instant::now() + IPC_RESPONSE_DEADLINE)
}

fn response_write_deadline_for_operation(operation: &str) -> Instant {
    let duration = match operation {
        "memory.capture" | "memory.capture_resolve" => IPC_CAPTURE_RESPONSE_DEADLINE,
        "content.vault.rebuild" => IPC_VAULT_RESPONSE_DEADLINE,
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

fn run_timeline_until(
    request: Request,
    context: Option<SearchContext>,
    peer_is_desktop: bool,
    deadline: Instant,
    reservation: Option<IpcWorkerReservation>,
) -> DesktopSearchResponse {
    let request_id = request.request_id.clone();
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let spawned = thread::Builder::new()
        .name("cryptex-ipc-timeline".into())
        .spawn(move || {
            let _reservation = reservation;
            let response =
                handle_timeline_until(request, context.as_ref(), peer_is_desktop, Some(deadline));
            let _ = sender.send(response);
        });
    if spawned.is_err() {
        return timeline_held(Some(request_id), "search_worker_unavailable");
    }
    match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(response) if Instant::now() <= deadline => response,
        Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            timeline_held(Some(request_id), "operation_deadline_exceeded")
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            timeline_held(Some(request_id), "canonical_verification_unavailable")
        }
    }
}

fn run_event_detail_until(
    request: Request,
    context: Option<SearchContext>,
    peer_is_desktop: bool,
    deadline: Instant,
    reservation: Option<IpcWorkerReservation>,
) -> DesktopSearchResponse {
    let request_id = request.request_id.clone();
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let spawned = thread::Builder::new()
        .name("cryptex-ipc-event-detail".into())
        .spawn(move || {
            let _reservation = reservation;
            let response = handle_event_detail_until(
                request,
                context.as_ref(),
                peer_is_desktop,
                Some(deadline),
            );
            let _ = sender.send(response);
        });
    if spawned.is_err() {
        return event_detail_held(Some(request_id), "search_worker_unavailable");
    }
    match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(response) if Instant::now() <= deadline => response,
        Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            event_detail_held(Some(request_id), "operation_deadline_exceeded")
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            event_detail_held(Some(request_id), "canonical_verification_unavailable")
        }
    }
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

/// Read-only Desktop status. No paths, transport identities, API credentials or
/// raw worker/native diagnostics cross the wire. Local readiness is not trust.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesktopTransportResponse {
    pub protocol_version: u32,
    pub request_id: Option<String>,
    pub accepted: bool,
    pub error: Option<String>,
    pub operation: Option<String>,
    pub daemon_pid: u32,
    pub configured: bool,
    pub protected: bool,
    pub state: Option<String>,
    pub phase: Option<String>,
    pub attempt: Option<u8>,
    pub native_pid: Option<u32>,
    pub stop_requested: bool,
    pub worker_error: Option<String>,
    /// Present only while the owned native worker is active and its
    /// authenticated REST status matches its pinned profile identity.
    pub transport_device_id: Option<String>,
}

fn handle_transport_status(
    request: Request,
    context: Option<&SearchContext>,
    peer_is_desktop: bool,
) -> DesktopTransportResponse {
    let error = if request.operation != "transport.status" {
        Some("unsupported_operation")
    } else if request.protocol_version != PROTOCOL_VERSION {
        Some("incompatible_protocol_version")
    } else if request.request_id.is_empty() || request.request_id.len() > 128 {
        Some("invalid_request_id")
    } else if request.client_role != "desktop" {
        Some("unauthorized_client_role")
    } else if !peer_is_desktop {
        Some("unauthorized_client_process")
    } else if request.capabilities.as_slice() != [CAPABILITY_STATUS_READ] {
        Some("unauthorized_capability")
    } else if request.query.is_some()
        || request.capture.is_some()
        || request.capture_resolve.is_some()
        || request.vault.is_some()
    {
        Some("unexpected_request_payload")
    } else if context.is_none() {
        Some("transport_status_unavailable")
    } else {
        None
    };
    let mut response = DesktopTransportResponse {
        protocol_version: PROTOCOL_VERSION,
        request_id: Some(request.request_id),
        accepted: error.is_none(),
        error: error.map(str::to_owned),
        operation: error.is_none().then(|| "transport.status.result".into()),
        daemon_pid: process::id(),
        configured: false,
        protected: false,
        state: None,
        phase: None,
        attempt: None,
        native_pid: None,
        stop_requested: false,
        worker_error: None,
        transport_device_id: None,
    };
    if error.is_some() {
        return response;
    }
    #[cfg(windows)]
    if let Some(reader) = context.and_then(|context| context.transport_status.as_ref()) {
        use crate::managed_profile::{
            controller::Phase,
            worker::{WorkerError, WorkerState},
        };
        let snapshot = match reader.try_snapshot() {
            Ok(snapshot) => snapshot,
            Err(_) => {
                response.accepted = false;
                response.operation = None;
                response.error = Some("transport_status_unavailable".into());
                return response;
            }
        };
        response.transport_device_id = reader.try_transport_device_id().ok();
        response.configured = true;
        response.state = Some(
            match snapshot.state {
                WorkerState::Opening => "opening",
                WorkerState::Active => "active",
                WorkerState::Stopping => "stopping",
                WorkerState::Finished => "finished",
                WorkerState::Failed => "failed",
            }
            .into(),
        );
        response.stop_requested = snapshot.stop_requested;
        if let Some(status) = snapshot.controller {
            response.phase = Some(
                match status.phase {
                    Phase::Starting => "starting",
                    Phase::LocalReady => "local_ready",
                    Phase::RestartWaiting => "restart_waiting",
                    Phase::Stopped => "stopped",
                    Phase::Failed => "failed",
                }
                .into(),
            );
            response.attempt = Some(status.attempt);
            // Terminal workers do not advertise a stale active PID.
            if matches!(snapshot.state, WorkerState::Active | WorkerState::Stopping) {
                response.native_pid = status.pid;
            }
        }
        response.worker_error = snapshot.error.map(|error| {
            match error {
                WorkerError::Spawn => "spawn_failed",
                WorkerError::Panicked => "worker_panicked",
                WorkerError::StatusUnavailable => "status_unavailable",
                WorkerError::InvalidDeadline => "invalid_deadline",
                WorkerError::ShutdownDeadline => "shutdown_deadline",
                WorkerError::Package(_) => "package_refused",
                WorkerError::Profile(_) => "profile_refused",
                WorkerError::Native(_) => "native_refused",
                WorkerError::Controller(_) => "controller_failed",
            }
            .to_owned()
        });
    }
    #[cfg(not(windows))]
    {
        response.state = Some("unsupported_platform".into());
    }
    response
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
        || request.vault.is_some()
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
    } else if request.operation != "content.search" {
        Some("unsupported_operation")
    } else if request.query.is_none() {
        Some("missing_search_query")
    } else if request.capture.is_some()
        || request.capture_resolve.is_some()
        || request.vault.is_some()
    {
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

fn handle_timeline_until(
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
    } else if request.operation != "content.timeline" {
        Some("unsupported_operation")
    } else if request.query.is_none() {
        Some("missing_timeline_query")
    } else if request.capture.is_some()
        || request.capture_resolve.is_some()
        || request.vault.is_some()
    {
        Some("unexpected_timeline_body")
    } else {
        None
    };
    if let Some(error) = authorization_error {
        return timeline_rejected(Some(request_id), error);
    }
    let Some(context) = context else {
        return timeline_rejected(Some(request_id), "search_unavailable");
    };
    let Some(wire_query) = request.query else {
        return timeline_rejected(Some(request_id), "missing_timeline_query");
    };
    let query = match wire_query.validate_timeline_and_convert() {
        Ok(query) => query,
        Err(error) => return timeline_rejected(Some(request_id), error),
    };
    if deadline_expired(deadline) {
        return timeline_held(Some(request_id), "operation_deadline_exceeded");
    }
    let brain = match load_brain_identity(&context.runtime_root) {
        Ok(brain) => brain,
        Err(_) => return timeline_held(Some(request_id), "canonical_verification_unavailable"),
    };
    if deadline_expired(deadline) {
        return timeline_held(Some(request_id), "operation_deadline_exceeded");
    }
    let local_identity = match load_local_device_identity(&context.runtime_root, &brain.brain_id) {
        Ok(identity) => identity,
        Err(crate::local_device_identity::LocalDeviceIdentityError::EnrollmentRequired) => {
            return timeline_held(Some(request_id), "local_enrollment_required");
        }
        Err(_) => return timeline_held(Some(request_id), "canonical_verification_unavailable"),
    };
    if deadline_expired(deadline) {
        return timeline_held(Some(request_id), "operation_deadline_exceeded");
    }
    let device = match DeviceId::new(local_identity.device_id) {
        Ok(device) => device,
        Err(_) => return timeline_held(Some(request_id), "canonical_verification_unavailable"),
    };
    let mut resolver = match LocalDeviceKeyResolver::load(
        &context.runtime_root,
        &brain.brain_id,
        context.store.as_ref(),
        context.environment,
    ) {
        Ok(resolver) => resolver,
        Err(_) => return timeline_held(Some(request_id), "canonical_verification_unavailable"),
    };
    if deadline_expired(deadline) {
        return timeline_held(Some(request_id), "operation_deadline_exceeded");
    }
    let db = match ProjectionDb::open_or_create(&context.runtime_root) {
        Ok(db) => db,
        Err(_) => {
            request_projection_sync(context);
            return timeline_held(Some(request_id), "materialization_state_missing_or_stale");
        }
    };
    if deadline_expired(deadline) {
        return timeline_held(Some(request_id), "operation_deadline_exceeded");
    }
    match crate::knowledge_materialization::timeline_events_with_retrieval_safety_deadline(
        &context.runtime_root,
        &brain.brain_id,
        &[device],
        &mut resolver,
        &db,
        &query,
        deadline,
    ) {
        Ok(page) => timeline_results(Some(request_id), page),
        Err(GuardedSearchError::BrainWideHold(reason)) => {
            if matches!(
                reason,
                RetrievalSafetyHold::MaterializationStateMissingOrStale
                    | RetrievalSafetyHold::ProjectionDoesNotMatchCanonical
            ) {
                drop(db);
                request_projection_sync(context);
            }
            timeline_held(Some(request_id), hold_reason_code(reason))
        }
        Err(GuardedSearchError::Query(_)) => timeline_rejected(Some(request_id), "invalid_query"),
    }
}

fn handle_event_detail_until(
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
    } else if request.operation != "content.event.detail" {
        Some("unsupported_operation")
    } else if request.query.is_none() {
        Some("missing_event_detail_query")
    } else if request.capture.is_some()
        || request.capture_resolve.is_some()
        || request.vault.is_some()
    {
        Some("unexpected_event_detail_body")
    } else {
        None
    };
    if let Some(error) = authorization_error {
        return event_detail_rejected(Some(request_id), error);
    }
    let Some(context) = context else {
        return event_detail_rejected(Some(request_id), "search_unavailable");
    };
    let Some(wire_query) = request.query else {
        return event_detail_rejected(Some(request_id), "missing_event_detail_query");
    };
    let event_id = match wire_query.validate_event_detail_id() {
        Ok(event_id) => event_id.to_owned(),
        Err(error) => return event_detail_rejected(Some(request_id), error),
    };
    if deadline_expired(deadline) {
        return event_detail_held(Some(request_id), "operation_deadline_exceeded");
    }
    let brain = match load_brain_identity(&context.runtime_root) {
        Ok(brain) => brain,
        Err(_) => return event_detail_held(Some(request_id), "canonical_verification_unavailable"),
    };
    if deadline_expired(deadline) {
        return event_detail_held(Some(request_id), "operation_deadline_exceeded");
    }
    let local_identity = match load_local_device_identity(&context.runtime_root, &brain.brain_id) {
        Ok(identity) => identity,
        Err(crate::local_device_identity::LocalDeviceIdentityError::EnrollmentRequired) => {
            return event_detail_held(Some(request_id), "local_enrollment_required");
        }
        Err(_) => {
            return event_detail_held(Some(request_id), "canonical_verification_unavailable");
        }
    };
    if deadline_expired(deadline) {
        return event_detail_held(Some(request_id), "operation_deadline_exceeded");
    }
    let device = match DeviceId::new(local_identity.device_id) {
        Ok(device) => device,
        Err(_) => return event_detail_held(Some(request_id), "canonical_verification_unavailable"),
    };
    let mut resolver = match LocalDeviceKeyResolver::load(
        &context.runtime_root,
        &brain.brain_id,
        context.store.as_ref(),
        context.environment,
    ) {
        Ok(resolver) => resolver,
        Err(_) => return event_detail_held(Some(request_id), "canonical_verification_unavailable"),
    };
    if deadline_expired(deadline) {
        return event_detail_held(Some(request_id), "operation_deadline_exceeded");
    }
    let db = match ProjectionDb::open_or_create(&context.runtime_root) {
        Ok(db) => db,
        Err(_) => {
            request_projection_sync(context);
            return event_detail_held(Some(request_id), "materialization_state_missing_or_stale");
        }
    };
    if deadline_expired(deadline) {
        return event_detail_held(Some(request_id), "operation_deadline_exceeded");
    }
    match crate::knowledge_materialization::event_detail_with_retrieval_safety_deadline(
        &context.runtime_root,
        &brain.brain_id,
        &[device],
        &mut resolver,
        &db,
        &event_id,
        deadline,
    ) {
        Ok(detail) => event_detail_results(Some(request_id), detail.into_iter().collect()),
        Err(GuardedSearchError::BrainWideHold(reason)) => {
            if matches!(
                reason,
                RetrievalSafetyHold::MaterializationStateMissingOrStale
                    | RetrievalSafetyHold::ProjectionDoesNotMatchCanonical
            ) {
                drop(db);
                request_projection_sync(context);
            }
            event_detail_held(Some(request_id), hold_reason_code(reason))
        }
        Err(GuardedSearchError::Query(_)) => {
            event_detail_rejected(Some(request_id), "invalid_event_id")
        }
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
        next_cursor: None,
        has_more: false,
    }
}

fn timeline_results(
    request_id: Option<String>,
    page: cryptex_projection::TimelinePage,
) -> DesktopSearchResponse {
    let has_more = page.next_cursor.is_some();
    let hits = page
        .hits
        .into_iter()
        .map(|hit| {
            let mut hit = DesktopSearchHit::from(hit);
            hit.text = timeline_snippet(&hit.text);
            hit
        })
        .collect();
    DesktopSearchResponse {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        accepted: true,
        error: None,
        operation: Some("content.timeline.result".into()),
        daemon_pid: process::id(),
        hits: Some(hits),
        hold_reason: None,
        next_cursor: page.next_cursor.map(|cursor| DesktopTimelineCursor {
            created_at: cursor.created_at,
            event_id: cursor.event_id,
        }),
        has_more,
    }
}

fn event_detail_results(request_id: Option<String>, hits: Vec<SearchHit>) -> DesktopSearchResponse {
    DesktopSearchResponse {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        accepted: true,
        error: None,
        operation: Some("content.event.detail.result".into()),
        daemon_pid: process::id(),
        hits: Some(hits.into_iter().map(DesktopSearchHit::from).collect()),
        hold_reason: None,
        next_cursor: None,
        has_more: false,
    }
}

fn timeline_snippet(text: &str) -> String {
    if text.len() <= TIMELINE_SNIPPET_BYTES {
        return text.to_owned();
    }
    let end = text
        .char_indices()
        .map(|(index, ch)| index + ch.len_utf8())
        .take_while(|end| *end <= TIMELINE_SNIPPET_BYTES)
        .last()
        .unwrap_or(0);
    let mut snippet = text[..end].to_owned();
    snippet.push('…');
    snippet
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
        next_cursor: None,
        has_more: false,
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
        next_cursor: None,
        has_more: false,
    }
}

fn timeline_held(request_id: Option<String>, reason: &str) -> DesktopSearchResponse {
    DesktopSearchResponse {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        accepted: true,
        error: None,
        operation: Some("content.timeline.result".into()),
        daemon_pid: process::id(),
        hits: None,
        hold_reason: Some(reason.into()),
        next_cursor: None,
        has_more: false,
    }
}

fn timeline_rejected(request_id: Option<String>, error: &str) -> DesktopSearchResponse {
    DesktopSearchResponse {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        accepted: false,
        error: Some(error.into()),
        operation: None,
        daemon_pid: process::id(),
        hits: None,
        hold_reason: None,
        next_cursor: None,
        has_more: false,
    }
}

fn event_detail_held(request_id: Option<String>, reason: &str) -> DesktopSearchResponse {
    DesktopSearchResponse {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        accepted: true,
        error: None,
        operation: Some("content.event.detail.result".into()),
        daemon_pid: process::id(),
        hits: None,
        hold_reason: Some(reason.into()),
        next_cursor: None,
        has_more: false,
    }
}

fn event_detail_rejected(request_id: Option<String>, error: &str) -> DesktopSearchResponse {
    DesktopSearchResponse {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        accepted: false,
        error: Some(error.into()),
        operation: None,
        daemon_pid: process::id(),
        hits: None,
        hold_reason: None,
        next_cursor: None,
        has_more: false,
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
        || request.vault.is_some()
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

fn handle_vault_rebuild(
    request: Request,
    context: Option<&SearchContext>,
    peer_is_desktop: bool,
) -> DesktopVaultResponse {
    let request_id = request.request_id.clone();
    let authorization_error = if request.protocol_version != PROTOCOL_VERSION {
        Some("incompatible_protocol_version")
    } else if request.request_id.is_empty() || request.request_id.len() > 128 {
        Some("invalid_request_id")
    } else if request.client_role != "desktop" || !peer_is_desktop {
        Some("unauthorized_client_process")
    } else if request.capabilities.as_slice() != [CAPABILITY_CONTENT_VAULT] {
        Some("unauthorized_capability")
    } else if request.operation != "content.vault.rebuild" {
        Some("unsupported_operation")
    } else if request.query.is_some()
        || request.capture.is_some()
        || request.capture_resolve.is_some()
        || request.vault.is_none()
    {
        Some("invalid_vault_request")
    } else {
        None
    };
    if let Some(error) = authorization_error {
        return vault_rejected(Some(request_id), error);
    }
    let Some(context) = context else {
        return vault_rejected(Some(request_id), "vault_unavailable");
    };
    let Some(body) = request.vault else {
        return vault_rejected(Some(request_id), "invalid_vault_request");
    };
    if body.vault_root.is_empty()
        || body.vault_root.len() > MAX_VAULT_ROOT_BYTES
        || body.vault_root.contains('\0')
    {
        return vault_rejected(Some(request_id), "invalid_vault_destination");
    }
    let requested = PathBuf::from(&body.vault_root);
    if !requested.is_absolute() {
        return vault_rejected(Some(request_id), "invalid_vault_destination");
    }
    match std::fs::symlink_metadata(&requested) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        _ => return vault_rejected(Some(request_id), "invalid_vault_destination"),
    }
    let root = match std::fs::canonicalize(&requested) {
        Ok(root) => root,
        Err(_) => return vault_rejected(Some(request_id), "invalid_vault_destination"),
    };
    if is_inside_source_repository(&root) {
        return vault_rejected(Some(request_id), "unsafe_vault_destination");
    }
    let runtime = match std::fs::canonicalize(&context.runtime_root) {
        Ok(path) => path,
        Err(_) => return vault_rejected(Some(request_id), "vault_unavailable"),
    };
    if path_is_same_or_within(&root, &runtime) {
        return vault_rejected(Some(request_id), "unsafe_vault_destination");
    }
    if synchronize_local_derived_state(context).is_err() {
        return vault_held(Some(request_id), "materialization_state_missing_or_stale");
    }
    let brain = match load_brain_identity(&context.runtime_root) {
        Ok(brain) => brain,
        Err(_) => return vault_held(Some(request_id), "canonical_verification_unavailable"),
    };
    let identity = match load_local_device_identity(&context.runtime_root, &brain.brain_id) {
        Ok(identity) => identity,
        Err(_) => return vault_held(Some(request_id), "local_enrollment_required"),
    };
    let device = match DeviceId::new(identity.device_id) {
        Ok(device) => device,
        Err(_) => return vault_held(Some(request_id), "canonical_verification_unavailable"),
    };
    let mut resolver = match LocalDeviceKeyResolver::load(
        &context.runtime_root,
        &brain.brain_id,
        context.store.as_ref(),
        context.environment,
    ) {
        Ok(resolver) => resolver,
        Err(_) => return vault_held(Some(request_id), "canonical_verification_unavailable"),
    };
    let db = match ProjectionDb::open_or_create(&context.runtime_root) {
        Ok(db) => db,
        Err(_) => return vault_held(Some(request_id), "materialization_state_missing_or_stale"),
    };
    match crate::knowledge_materialization::rebuild_vault_with_retrieval_safety(
        &context.runtime_root,
        &brain.brain_id,
        &[device],
        &mut resolver,
        &db,
        &root,
    ) {
        Ok(summary) => DesktopVaultResponse {
            protocol_version: PROTOCOL_VERSION,
            request_id: Some(request_id),
            accepted: true,
            error: None,
            operation: Some("content.vault.rebuilt".into()),
            daemon_pid: process::id(),
            note_count: Some(summary.note_count),
            open_note_path: summary
                .open_note_path
                .map(|path| path.to_string_lossy().replace('\\', "/")),
            hold_reason: None,
        },
        Err(crate::knowledge_materialization::GuardedVaultError::BrainWideHold(reason)) => {
            vault_held(Some(request_id), retrieval_hold_code(reason))
        }
        Err(crate::knowledge_materialization::GuardedVaultError::Projection(_)) => {
            vault_rejected(Some(request_id), "vault_rebuild_failed")
        }
    }
}

fn is_inside_source_repository(path: &Path) -> bool {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let Ok(repository) = std::fs::canonicalize(repository) else {
        return false;
    };
    let path = normalize_path_for_comparison(path);
    let repository = normalize_path_for_comparison(&repository);
    path_is_same_or_within(&path, &repository)
}

fn normalize_path_for_comparison(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from(path.to_string_lossy().to_lowercase())
    }
    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}

fn path_is_same_or_within(path: &Path, root: &Path) -> bool {
    #[cfg(windows)]
    let (path, root) = (
        PathBuf::from(path.to_string_lossy().to_lowercase()),
        PathBuf::from(root.to_string_lossy().to_lowercase()),
    );
    path == root || path.starts_with(root)
}

fn vault_rejected(request_id: Option<String>, error: &str) -> DesktopVaultResponse {
    DesktopVaultResponse {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        accepted: false,
        error: Some(error.into()),
        operation: None,
        daemon_pid: process::id(),
        note_count: None,
        open_note_path: None,
        hold_reason: None,
    }
}

fn vault_held(request_id: Option<String>, reason: &str) -> DesktopVaultResponse {
    DesktopVaultResponse {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        accepted: true,
        error: None,
        operation: Some("content.vault.rebuilt".into()),
        daemon_pid: process::id(),
        note_count: None,
        open_note_path: None,
        hold_reason: Some(reason.into()),
    }
}

fn retrieval_hold_code(reason: RetrievalSafetyHold) -> &'static str {
    match reason {
        RetrievalSafetyHold::OperationDeadlineExceeded => "operation_deadline_exceeded",
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
        vault: None,
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
    use ed25519_dalek::{Signer, SigningKey};

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

    fn approval_request(operation: &str) -> BootstrapApprovalIpcRequest {
        BootstrapApprovalIpcRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: "11111111-1111-4111-8111-111111111111".into(),
            client_role: "desktop".into(),
            capabilities: vec![CAPABILITY_BOOTSTRAP_APPROVAL.into()],
            operation: operation.into(),
            role: Some("initiator".into()),
            transcript_messages: Some(vec!["{}".into(), "{}".into()]),
            approval_request_digest: None,
            capability: None,
            transport_device_id: None,
            offer_message: None,
            brain_identity_json: None,
            transaction_id: None,
            owner_operation: None,
        }
    }

    fn test_connected_process_identity() -> DesktopProcessIdentity {
        let pid = process::id();
        #[cfg(windows)]
        {
            use std::os::windows::io::FromRawHandle;
            use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE};

            let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
            assert!(!handle.is_null());
            DesktopProcessIdentity {
                pid,
                process: unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(handle) },
            }
        }
        #[cfg(not(windows))]
        {
            DesktopProcessIdentity { pid }
        }
    }

    fn test_bootstrap_hash(domain: &[u8], value: &serde_json::Value) -> String {
        let canonical = serde_json_canonicalizer::to_vec(value).unwrap();
        let mut hasher = blake3::Hasher::new();
        hasher.update(domain);
        hasher.update(&[0]);
        hasher.update(&canonical);
        hasher.finalize().to_hex().to_string()
    }

    fn test_hex(bytes: &[u8]) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut result = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            result.push(DIGITS[(byte >> 4) as usize] as char);
            result.push(DIGITS[(byte & 0x0f) as usize] as char);
        }
        result
    }

    fn sign_test_bootstrap_message(mut value: serde_json::Value, key: &SigningKey) -> Vec<u8> {
        let object = value.as_object_mut().unwrap();
        object.remove("signature");
        let canonical = serde_json_canonicalizer::to_vec(&value).unwrap();
        let mut preimage = b"cryptex-peer-bootstrap-sig-v1".to_vec();
        preimage.push(0);
        preimage.extend_from_slice(&canonical);
        let signature = key.sign(&preimage);
        value.as_object_mut().unwrap().insert(
            "signature".into(),
            serde_json::Value::String(test_hex(&signature.to_bytes())),
        );
        serde_json_canonicalizer::to_vec(&value).unwrap()
    }

    #[allow(clippy::too_many_arguments)]
    fn bootstrap_message(
        kind: &str,
        transaction_id: &str,
        sender_device_id: &str,
        sender_key: &SigningKey,
        sender_transport_device_id: &str,
        peer: Option<(&str, &SigningKey, &str)>,
        previous_hash: &str,
        payload: serde_json::Value,
    ) -> serde_json::Value {
        let sender_public_key = sender_key.verifying_key().to_bytes();
        let sender_signing_key_id = cryptex_events::integrity::signing_key_id_from_verifying_key(
            &sender_key.verifying_key(),
        )
        .unwrap();
        let (peer_device_id, peer_signing_key_id, peer_public_key, peer_transport_device_id) =
            if let Some((device_id, key, transport_id)) = peer {
                let key_id = cryptex_events::integrity::signing_key_id_from_verifying_key(
                    &key.verifying_key(),
                )
                .unwrap();
                (
                    serde_json::Value::String(device_id.into()),
                    serde_json::Value::String(key_id.as_str().into()),
                    serde_json::Value::String(test_hex(&key.verifying_key().to_bytes())),
                    serde_json::Value::String(transport_id.into()),
                )
            } else {
                (
                    serde_json::Value::Null,
                    serde_json::Value::Null,
                    serde_json::Value::Null,
                    serde_json::Value::Null,
                )
            };
        serde_json::json!({
            "protocol_version": 1,
            "message_type": kind,
            "transaction_id": transaction_id,
            "brain_id": TEST_BRAIN_ID,
            "sender_device_id": sender_device_id,
            "sender_signing_key_id": sender_signing_key_id.as_str(),
            "sender_public_key": test_hex(&sender_public_key),
            "sender_transport_device_id": sender_transport_device_id,
            "peer_device_id": peer_device_id,
            "peer_signing_key_id": peer_signing_key_id,
            "peer_public_key": peer_public_key,
            "peer_transport_device_id": peer_transport_device_id,
            "previous_hash": previous_hash,
            "payload": payload,
            "signature": "00".repeat(64)
        })
    }

    fn synthetic_initiator_prefix(fixture: &SearchFixture) -> Vec<String> {
        let peer_key = SigningKey::from_bytes(&[0x42; 32]);
        synthetic_initiator_prefix_with_peer_key(fixture, &peer_key)
    }

    fn synthetic_initiator_prefix_with_peer_key(
        fixture: &SearchFixture,
        peer_key: &SigningKey,
    ) -> Vec<String> {
        let ready = load_existing_event_signing_key(
            fixture.store.as_ref(),
            SecureStoreEnvironment::Dev,
            &fixture.root,
            &fixture.device,
        )
        .unwrap();
        let peer_device_id = "33333333-3333-4333-8333-333333333333";
        let local_transport = "AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA";
        let peer_transport = "BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB";
        let transaction_id = "ab".repeat(32);
        let offer = sign_test_bootstrap_message(
            bootstrap_message(
                "offer",
                &transaction_id,
                fixture.device.as_str(),
                &ready.signing_key,
                local_transport,
                None,
                &"00".repeat(32),
                serde_json::json!({"initiator_nonce":"11".repeat(32)}),
            ),
            &ready.signing_key,
        );
        let offer_value: serde_json::Value = serde_json::from_slice(&offer).unwrap();
        let offer_hash = test_bootstrap_hash(b"cryptex-peer-bootstrap-message-v1", &offer_value);
        let proof = sign_test_bootstrap_message(
            bootstrap_message(
                "proof",
                &transaction_id,
                peer_device_id,
                peer_key,
                peer_transport,
                Some((fixture.device.as_str(), &ready.signing_key, local_transport)),
                &offer_hash,
                serde_json::json!({"offer_hash":offer_hash,"responder_nonce":"22".repeat(32)}),
            ),
            peer_key,
        );
        vec![
            String::from_utf8(offer).unwrap(),
            String::from_utf8(proof).unwrap(),
        ]
    }

    #[test]
    fn bootstrap_approval_ipc_requires_exact_peer_role_capability_and_request_id() {
        let request = approval_request("bootstrap.approval.prepare");
        assert!(approval_request_is_authorized(&request));
        assert!(approval_request_has_prepare_shape(&request));
        assert!(!approval_request_has_prepare_shape(
            &BootstrapApprovalIpcRequest {
                capability: Some("a".repeat(64)),
                ..approval_request("bootstrap.approval.prepare")
            }
        ));
        assert!(!approval_request_has_followup_shape(
            &BootstrapApprovalIpcRequest {
                approval_request_digest: Some("A".repeat(64)),
                capability: Some("b".repeat(64)),
                ..approval_request("bootstrap.approval.confirm")
            }
        ));
        assert!(approval_request_has_followup_shape(
            &BootstrapApprovalIpcRequest {
                approval_request_digest: Some("a".repeat(64)),
                capability: Some("b".repeat(64)),
                ..approval_request("bootstrap.approval.confirm")
            }
        ));
        assert!(!approval_request_is_authorized(
            &BootstrapApprovalIpcRequest {
                client_role: "cli".into(),
                ..approval_request("bootstrap.approval.prepare")
            }
        ));
        assert!(!approval_request_is_authorized(
            &BootstrapApprovalIpcRequest {
                capabilities: vec![
                    CAPABILITY_BOOTSTRAP_APPROVAL.into(),
                    CAPABILITY_STATUS_READ.into()
                ],
                ..approval_request("bootstrap.approval.prepare")
            }
        ));
        assert!(!approval_request_is_authorized(
            &BootstrapApprovalIpcRequest {
                request_id: "bad request".into(),
                ..approval_request("bootstrap.approval.prepare")
            }
        ));
        assert!(serde_json::from_str::<BootstrapApprovalIpcRequest>(
            r#"{"protocolVersion":1,"requestId":"r","clientRole":"desktop","capabilities":["peer.approval.manage"],"operation":"bootstrap.approval.prepare","unexpected":true}"#
        ).is_err());
    }

    #[test]
    fn bootstrap_exchange_ipc_accepts_only_exact_offer_and_proof_shapes() {
        let mut read_identity = approval_request("bootstrap.exchange.brain_identity.read");
        read_identity.role = None;
        read_identity.transcript_messages = None;
        assert!(bootstrap_exchange_request_is_well_shaped(&read_identity));

        let mut offer = approval_request("bootstrap.exchange.offer.create");
        offer.role = None;
        offer.transcript_messages = None;
        offer.transport_device_id =
            Some("AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA".into());
        offer.transaction_id = Some("ab".repeat(32));
        assert!(approval_request_is_authorized(&offer));
        assert!(bootstrap_exchange_request_is_well_shaped(&offer));

        let mut proof = approval_request("bootstrap.exchange.proof.create");
        proof.role = None;
        proof.transcript_messages = None;
        proof.transport_device_id =
            Some("BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB".into());
        proof.offer_message = Some("{}".into());
        proof.brain_identity_json = Some("{}".into());
        assert!(bootstrap_exchange_request_is_well_shaped(&proof));

        proof.capability = Some("11".repeat(32));
        assert!(!bootstrap_exchange_request_is_well_shaped(&proof));
        offer.transaction_id = Some("AB".repeat(32));
        assert!(!bootstrap_exchange_request_is_well_shaped(&offer));
    }

    #[test]
    fn bootstrap_exchange_requires_the_current_adapter_identity() {
        let fixture = prepared_search_fixture("bootstrap-transport-identity-binding");
        let actual = "AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA";
        assert_eq!(
            require_current_transport_device_id(&fixture.context, actual),
            Ok(())
        );
        assert_eq!(
            require_current_transport_device_id(
                &fixture.context,
                "BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB"
            ),
            Err("transport_identity_mismatch")
        );
        fs::remove_dir_all(fixture.root).expect("remove synthetic fixture");

        let missing = SearchContext {
            runtime_root: exclusive_temp_dir("bootstrap-transport-identity-missing"),
            store: Arc::new(FakeSecureStore::new()),
            environment: SecureStoreEnvironment::Dev,
            expected_desktop: Arc::new(
                test_desktop_process_identity().expect("capture test Desktop identity"),
            ),
            projection_sync: None,
            #[cfg(windows)]
            transport_status: None,
            #[cfg(test)]
            transport_device_id_override: None,
        };
        assert_eq!(
            require_current_transport_device_id(&missing, actual),
            Err("transport_identity_unavailable")
        );
        fs::remove_dir_all(missing.runtime_root).expect("remove missing-identity fixture");
    }

    #[test]
    fn bootstrap_offer_is_authenticated_and_journaled_idempotently_before_response() {
        let fixture = prepared_search_fixture("bootstrap-offer-journaled");
        let mut request = approval_request("bootstrap.exchange.offer.create");
        request.role = None;
        request.transcript_messages = None;
        request.transport_device_id =
            Some("AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA".into());
        request.transaction_id = Some("ab".repeat(32));

        let (transaction_id, first, operation) =
            create_or_resume_bootstrap_offer(&fixture.context, &request).expect("create offer");
        assert_eq!(operation, "bootstrap.exchange.offer.created");
        assert_eq!(transaction_id, "ab".repeat(32));
        cryptex_events::verify_bootstrap_offer(first.as_bytes()).expect("signed offer");
        let journal = crate::bootstrap_journal::BootstrapJournalStore::new(&fixture.root)
            .expect("journal")
            .load()
            .expect("load journal");
        assert_eq!(journal.exchanges.len(), 1);
        assert_eq!(
            journal.exchanges[0].transcript_messages,
            vec![first.clone()]
        );

        let (_, replay, _) =
            create_or_resume_bootstrap_offer(&fixture.context, &request).expect("resume offer");
        assert_eq!(replay, first);
        assert_eq!(
            crate::bootstrap_journal::BootstrapJournalStore::new(&fixture.root)
                .expect("journal")
                .load()
                .expect("load journal")
                .exchanges
                .len(),
            1
        );
        fs::remove_dir_all(fixture.root).expect("remove synthetic fixture");
    }

    #[test]
    fn bootstrap_brain_identity_read_returns_exact_canonical_public_bytes() {
        let fixture = prepared_search_fixture("bootstrap-brain-identity-read");
        let expected = load_brain_identity(&fixture.root)
            .expect("load identity")
            .to_json_lf()
            .expect("canonical identity");
        assert_eq!(read_local_brain_identity(&fixture.context), Ok(expected));
        fs::remove_dir_all(fixture.root).expect("remove synthetic fixture");
    }

    #[test]
    fn bootstrap_proof_verifies_and_journals_the_exact_received_offer() {
        let mut fixture = prepared_search_fixture("bootstrap-proof-journaled");
        let peer_key = SigningKey::from_bytes(&[0x53; 32]);
        let peer_device_id = "33333333-3333-4333-8333-333333333333";
        let peer_transport = "BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB";
        let local_transport = "CCCCCCC-CCCCCCC-CCCCCCC-CCCCCCC-CCCCCCC-CCCCCCC-CCCCCCC-CCCCCCC";
        fixture.context.transport_device_id_override = Some(local_transport.into());
        let transaction_id = "cd".repeat(32);
        let offer = sign_test_bootstrap_message(
            bootstrap_message(
                "offer",
                &transaction_id,
                peer_device_id,
                &peer_key,
                peer_transport,
                None,
                &"00".repeat(32),
                serde_json::json!({"initiator_nonce":"53".repeat(32)}),
            ),
            &peer_key,
        );
        let offer_text = String::from_utf8(offer.clone()).expect("offer UTF-8");
        let mut request = approval_request("bootstrap.exchange.proof.create");
        request.role = None;
        request.transcript_messages = None;
        request.transport_device_id = Some(local_transport.into());
        request.offer_message = Some(offer_text);
        request.brain_identity_json = Some(
            load_brain_identity(&fixture.root)
                .unwrap()
                .to_json_lf()
                .unwrap(),
        );

        let (returned_transaction, proof, operation) =
            create_or_resume_bootstrap_proof(&fixture.context, &request).expect("create proof");
        assert_eq!(returned_transaction, transaction_id);
        assert_eq!(operation, "bootstrap.exchange.proof.created");
        let verified = cryptex_events::verify_bootstrap_approval_request(
            &[offer.as_slice(), proof.as_bytes()],
            cryptex_events::BootstrapApprovalRole::Initiator,
        )
        .expect("verified signed offer and proof");
        assert_eq!(verified.transaction_id(), transaction_id);

        let journal = crate::bootstrap_journal::BootstrapJournalStore::new(&fixture.root)
            .expect("journal")
            .load()
            .expect("load journal");
        assert_eq!(journal.exchanges.len(), 1);
        assert_eq!(
            journal.exchanges[0].transcript_messages[0].as_bytes(),
            offer
        );
        assert_eq!(journal.exchanges[0].transcript_messages[1], proof);

        let (_, replay, _) =
            create_or_resume_bootstrap_proof(&fixture.context, &request).expect("resume proof");
        assert_eq!(replay, proof);
        fs::remove_dir_all(fixture.root).expect("remove synthetic fixture");
    }

    #[test]
    fn bootstrap_proof_refuses_a_different_brain_without_journaling() {
        let mut fixture = prepared_search_fixture("bootstrap-proof-brain-mismatch");
        let peer_key = SigningKey::from_bytes(&[0x59; 32]);
        let peer_device_id = "33333333-3333-4333-8333-333333333333";
        let peer_transport = "BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB-BBBBBBB";
        let local_transport = "CCCCCCC-CCCCCCC-CCCCCCC-CCCCCCC-CCCCCCC-CCCCCCC-CCCCCCC-CCCCCCC";
        fixture.context.transport_device_id_override = Some(local_transport.into());
        let transaction_id = "ef".repeat(32);
        let mut body = bootstrap_message(
            "offer",
            &transaction_id,
            peer_device_id,
            &peer_key,
            peer_transport,
            None,
            &"00".repeat(32),
            serde_json::json!({"initiator_nonce":"59".repeat(32)}),
        );
        body.as_object_mut().unwrap().insert(
            "brain_id".into(),
            serde_json::Value::String("99999999-9999-4999-8999-999999999999".into()),
        );
        let offer = sign_test_bootstrap_message(body, &peer_key);
        let mut request = approval_request("bootstrap.exchange.proof.create");
        request.role = None;
        request.transcript_messages = None;
        request.transport_device_id = Some(local_transport.into());
        request.offer_message = Some(String::from_utf8(offer).unwrap());
        request.brain_identity_json = Some(
            load_brain_identity(&fixture.root)
                .unwrap()
                .to_json_lf()
                .unwrap(),
        );

        assert_eq!(
            create_or_resume_bootstrap_proof(&fixture.context, &request),
            Err("brain_mismatch")
        );
        let journal = crate::bootstrap_journal::BootstrapJournalStore::new(&fixture.root)
            .expect("journal")
            .load()
            .expect("load journal");
        assert!(journal.exchanges.is_empty());
        fs::remove_dir_all(fixture.root).expect("remove synthetic fixture");
    }

    #[test]
    fn bootstrap_approval_ipc_route_rejects_unauthenticated_peer_before_parsing_transcript() {
        let (name, listener) = isolated_ipc_listener("bootstrap-approval-auth");
        let server = thread::spawn(move || {
            let mut stream = listener.accept().unwrap();
            handle_connection_for_peer(&mut stream, None, None, None, None, false).unwrap();
        });
        let mut client = Stream::connect(name.borrow()).unwrap();
        let response = exchange_bootstrap_approval(
            &mut client,
            &approval_request("bootstrap.approval.prepare"),
            Instant::now() + IPC_RESPONSE_DEADLINE,
        )
        .unwrap();
        assert!(!response.accepted);
        assert_eq!(
            response.error.as_deref(),
            Some("desktop_authentication_required")
        );
        assert!(response.snapshot.is_none());
        assert!(response.capability.is_none());
        server.join().unwrap();
    }

    #[test]
    #[cfg_attr(
        target_os = "macos",
        ignore = "macOS process-instance binding remains fail-closed"
    )]
    fn owner_security_ipc_binds_confirmation_and_appends_self_retirement_once() {
        let fixture = prepared_search_fixture("owner-self-retirement-ipc");
        let transaction_id = "cd".repeat(32);
        let (name, listener) = isolated_ipc_listener("owner-self-retirement-ipc");
        let mut context = fixture.context.clone();
        context.expected_desktop = Arc::new(test_connected_process_identity());
        let server = thread::spawn(move || {
            let mut stream = listener.accept().unwrap();
            handle_connection_for_peer(&mut stream, Some(&context), None, None, None, true)
                .unwrap();
        });
        let mut client = Stream::connect(name.borrow()).unwrap();
        let mut prepare = approval_request("owner.security.prepare");
        prepare.role = None;
        prepare.transcript_messages = None;
        prepare.transaction_id = Some(transaction_id.clone());
        prepare.owner_operation = Some(OwnerSecurityOperationRequest::SelfRetirement);
        let prepared = exchange_bootstrap_approval(
            &mut client,
            &prepare,
            Instant::now() + IPC_RESPONSE_DEADLINE,
        )
        .unwrap();
        assert!(prepared.accepted, "{:?}", prepared.error);
        assert_eq!(prepared.operation, "owner.security.prepared");
        let snapshot = prepared.snapshot.expect("daemon-derived snapshot");
        assert_eq!(
            snapshot.operation,
            crate::bootstrap_journal::BootstrapOperation::SelfRetirement
        );
        assert_eq!(snapshot.local_identity.device_id, fixture.device.as_str());
        let digest = prepared.approval_request_digest.unwrap();
        let capability = prepared.capability.unwrap();

        let confirm = BootstrapApprovalIpcRequest {
            operation: "owner.security.confirm".into(),
            role: None,
            transcript_messages: None,
            transaction_id: Some(transaction_id.clone()),
            approval_request_digest: Some(digest),
            capability: Some(capability),
            ..approval_request("owner.security.confirm")
        };
        let confirmed = exchange_bootstrap_approval(
            &mut client,
            &confirm,
            Instant::now() + IPC_RESPONSE_DEADLINE,
        )
        .unwrap();
        assert!(confirmed.accepted, "{:?}", confirmed.error);
        assert_eq!(confirmed.operation, "owner.security.result");
        server.join().unwrap();

        let journal = crate::bootstrap_journal::BootstrapJournalStore::new(&fixture.root)
            .unwrap()
            .load()
            .unwrap();
        assert_eq!(journal.transactions.len(), 1);
        assert_eq!(
            journal.transactions[0].state,
            crate::bootstrap_journal::BootstrapState::Anchored
        );
        assert_eq!(journal.transactions[0].append_attempts.len(), 1);
        fs::remove_dir_all(fixture.root).unwrap();
    }

    #[test]
    #[cfg_attr(
        target_os = "macos",
        ignore = "macOS process-instance binding remains fail-closed"
    )]
    fn owner_security_ipc_cancel_is_terminal_and_does_not_append() {
        let fixture = prepared_search_fixture("owner-self-retirement-cancel");
        let transaction_id = "ce".repeat(32);
        let (name, listener) = isolated_ipc_listener("owner-self-retirement-cancel");
        let mut context = fixture.context.clone();
        context.expected_desktop = Arc::new(test_connected_process_identity());
        let server = thread::spawn(move || {
            let mut stream = listener.accept().unwrap();
            handle_connection_for_peer(&mut stream, Some(&context), None, None, None, true)
                .unwrap();
        });
        let mut client = Stream::connect(name.borrow()).unwrap();
        let mut prepare = approval_request("owner.security.prepare");
        prepare.role = None;
        prepare.transcript_messages = None;
        prepare.transaction_id = Some(transaction_id.clone());
        prepare.owner_operation = Some(OwnerSecurityOperationRequest::SelfRetirement);
        let prepared = exchange_bootstrap_approval(
            &mut client,
            &prepare,
            Instant::now() + IPC_RESPONSE_DEADLINE,
        )
        .unwrap();
        assert!(prepared.accepted, "{:?}", prepared.error);

        let cancel = BootstrapApprovalIpcRequest {
            operation: "owner.security.cancel".into(),
            role: None,
            transcript_messages: None,
            transaction_id: Some(transaction_id),
            approval_request_digest: prepared.approval_request_digest,
            capability: prepared.capability,
            ..approval_request("owner.security.cancel")
        };
        let cancelled = exchange_bootstrap_approval(
            &mut client,
            &cancel,
            Instant::now() + IPC_RESPONSE_DEADLINE,
        )
        .unwrap();
        assert!(cancelled.accepted, "{:?}", cancelled.error);
        assert_eq!(cancelled.operation, "owner.security.cancelled");
        assert!(cancelled.signed_message.is_none());
        server.join().unwrap();

        let journal = crate::bootstrap_journal::BootstrapJournalStore::new(&fixture.root)
            .unwrap()
            .load()
            .unwrap();
        assert_eq!(journal.transactions.len(), 1);
        assert_eq!(
            journal.transactions[0].state,
            crate::bootstrap_journal::BootstrapState::Cancelled
        );
        assert!(journal.transactions[0].append_attempts.is_empty());
        let canonical = crate::canonical_writer::load_verified_events(
            fixture.store.as_ref(),
            SecureStoreEnvironment::Dev,
            &fixture.root,
            &fixture.device,
        )
        .unwrap();
        assert_eq!(canonical.len(), 1, "cancel must not append canonical data");
        fs::remove_dir_all(fixture.root).unwrap();
    }

    #[test]
    #[cfg_attr(
        target_os = "macos",
        ignore = "macOS peer process-instance binding is not implemented"
    )]
    fn bootstrap_approval_ipc_freezes_displays_and_journals_before_signature_release() {
        let fixture = prepared_search_fixture("bootstrap-approval-round-trip");
        let peer_device = DeviceId::new("33333333-3333-4333-8333-333333333333").unwrap();
        let peer_key = crate::event_signing_key::ensure_event_signing_key(
            fixture.store.as_ref(),
            SecureStoreEnvironment::Dev,
            &fixture.root,
            &peer_device,
            &[],
            false,
        )
        .unwrap()
        .signing_key;
        let messages = synthetic_initiator_prefix_with_peer_key(&fixture, &peer_key);
        let (name, listener) = isolated_ipc_listener("bootstrap-approval-round-trip");
        let mut context = fixture.context.clone();
        context.expected_desktop = Arc::new(test_connected_process_identity());
        let server = thread::spawn(move || {
            for _ in 0..3 {
                let mut stream = listener.accept().unwrap();
                handle_connection_for_peer(&mut stream, Some(&context), None, None, None, true)
                    .unwrap();
            }
        });
        let mut client = Stream::connect(name.borrow()).unwrap();
        let prepare = BootstrapApprovalIpcRequest {
            transcript_messages: Some(messages.clone()),
            ..approval_request("bootstrap.approval.prepare")
        };
        let prepared = exchange_bootstrap_approval(
            &mut client,
            &prepare,
            Instant::now() + IPC_RESPONSE_DEADLINE,
        )
        .unwrap();
        assert!(prepared.accepted, "{:?}", prepared.error);
        assert_eq!(prepared.operation, "bootstrap.approval.prepared");
        let snapshot = prepared.snapshot.as_ref().expect("frozen snapshot");
        assert_eq!(snapshot.local_identity.device_id, fixture.device.as_str());
        assert_eq!(
            snapshot.peer_identity.as_ref().unwrap().device_id,
            "33333333-3333-4333-8333-333333333333"
        );
        let digest = prepared.approval_request_digest.clone().unwrap();
        let capability = prepared.capability.clone().unwrap();

        let confirm = BootstrapApprovalIpcRequest {
            operation: "bootstrap.approval.confirm".into(),
            transcript_messages: Some(messages.clone()),
            approval_request_digest: Some(digest),
            capability: Some(capability),
            ..approval_request("bootstrap.approval.confirm")
        };
        let confirmed = exchange_bootstrap_approval(
            &mut client,
            &confirm,
            Instant::now() + IPC_RESPONSE_DEADLINE,
        )
        .unwrap();
        assert!(confirmed.accepted, "{:?}", confirmed.error);
        assert_eq!(confirmed.operation, "bootstrap.approval.signed");
        let signed_message = confirmed.signed_message.expect("signed approval message");
        let complete_prefix = [
            messages[0].as_bytes(),
            messages[1].as_bytes(),
            signed_message.as_bytes(),
        ];
        cryptex_events::verify_bootstrap_approval_request(
            &complete_prefix,
            cryptex_events::BootstrapApprovalRole::Responder,
        )
        .expect("signed message verifies as the initiator approval");
        let responder_request = cryptex_events::verify_bootstrap_approval_request(
            &complete_prefix,
            cryptex_events::BootstrapApprovalRole::Responder,
        )
        .expect("responder approval prefix verifies");
        let peer_approval = cryptex_events::sign_bootstrap_approval(&responder_request, &peer_key)
            .expect("synthetic peer approval");
        let complete_transcript = vec![
            messages[0].clone(),
            messages[1].clone(),
            signed_message,
            String::from_utf8(peer_approval).unwrap(),
        ];
        drop(client);
        let mut completion_client = Stream::connect(name.borrow()).unwrap();
        let completed = exchange_bootstrap_approval(
            &mut completion_client,
            &BootstrapApprovalIpcRequest {
                operation: "bootstrap.approval.complete".into(),
                role: None,
                transcript_messages: Some(complete_transcript),
                approval_request_digest: None,
                capability: None,
                ..approval_request("bootstrap.approval.complete")
            },
            Instant::now() + IPC_RESPONSE_DEADLINE,
        )
        .unwrap();
        assert!(completed.accepted, "{:?}", completed.error);
        assert_eq!(completed.operation, "bootstrap.approval.anchored");

        let peer_event = append_memory_note(
            fixture.store.as_ref(),
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &fixture.root,
                brain_id: &fixture.brain_id,
                device_id: &peer_device,
                classification: Classification::Public,
                text: "synthetic approved-peer cutoff event",
                event_id: None,
                config: &CanonicalWriterConfig::default(),
            },
        )
        .expect("append one event to the approved synthetic peer stream");

        let owner_transaction_id = "de".repeat(32);
        let mut owner_client = Stream::connect(name.borrow()).unwrap();
        let owner_prepare = BootstrapApprovalIpcRequest {
            operation: "owner.security.prepare".into(),
            role: None,
            transcript_messages: None,
            transaction_id: Some(owner_transaction_id.clone()),
            owner_operation: Some(OwnerSecurityOperationRequest::DeviceRestriction {
                target_device_id: "33333333-3333-4333-8333-333333333333".into(),
                reason: crate::bootstrap_journal::RestrictionReason::Compromised,
                cutoff_seq: 1,
            }),
            ..approval_request("owner.security.prepare")
        };
        let owner_prepared = exchange_bootstrap_approval(
            &mut owner_client,
            &owner_prepare,
            Instant::now() + IPC_RESPONSE_DEADLINE,
        )
        .unwrap();
        assert!(owner_prepared.accepted, "{:?}", owner_prepared.error);
        let owner_snapshot = owner_prepared.snapshot.as_ref().unwrap();
        assert_eq!(
            owner_snapshot.restriction_target.as_deref(),
            Some("33333333-3333-4333-8333-333333333333")
        );
        assert_eq!(
            owner_snapshot.cutoff_content_hash.as_deref(),
            Some(peer_event.accepted.content_hash.as_str())
        );
        let owner_confirm = BootstrapApprovalIpcRequest {
            operation: "owner.security.confirm".into(),
            role: None,
            transcript_messages: None,
            transaction_id: Some(owner_transaction_id),
            approval_request_digest: owner_prepared.approval_request_digest,
            capability: owner_prepared.capability,
            ..approval_request("owner.security.confirm")
        };
        let owner_confirmed = exchange_bootstrap_approval(
            &mut owner_client,
            &owner_confirm,
            Instant::now() + IPC_RESPONSE_DEADLINE,
        )
        .unwrap();
        assert!(owner_confirmed.accepted, "{:?}", owner_confirmed.error);
        assert_eq!(owner_confirmed.operation, "owner.security.result");
        drop(owner_client);
        server.join().unwrap();

        let journal = crate::bootstrap_journal::BootstrapJournalStore::new(&fixture.root)
            .unwrap()
            .load()
            .unwrap();
        assert_eq!(journal.transactions.len(), 2);
        let transaction = &journal.transactions[0];
        assert!(transaction.local_approval_signature_generated);
        assert!(transaction.local_approval_signature_released);
        assert_eq!(
            transaction.state,
            crate::bootstrap_journal::BootstrapState::Anchored
        );
        let restriction = &journal.transactions[1];
        assert_eq!(
            restriction.state,
            crate::bootstrap_journal::BootstrapState::Anchored
        );
        assert_eq!(restriction.append_attempts.len(), 1);
        fs::remove_dir_all(fixture.root).unwrap();
    }

    #[test]
    fn bootstrap_completion_cannot_bypass_journaled_owner_approval() {
        let fixture = prepared_search_fixture("bootstrap-completion-no-owner-approval");
        let prefix = synthetic_initiator_prefix(&fixture);
        let local_key = load_existing_event_signing_key(
            fixture.store.as_ref(),
            SecureStoreEnvironment::Dev,
            &fixture.root,
            &fixture.device,
        )
        .unwrap();
        let refs = prefix.iter().map(String::as_bytes).collect::<Vec<_>>();
        let request = cryptex_events::verify_bootstrap_approval_request(
            &refs,
            cryptex_events::BootstrapApprovalRole::Initiator,
        )
        .unwrap();
        let local_approval =
            cryptex_events::sign_bootstrap_approval(&request, &local_key.signing_key).unwrap();
        let responder_prefix = [
            prefix[0].as_bytes(),
            prefix[1].as_bytes(),
            local_approval.as_slice(),
        ];
        let responder_request = cryptex_events::verify_bootstrap_approval_request(
            &responder_prefix,
            cryptex_events::BootstrapApprovalRole::Responder,
        )
        .unwrap();
        let peer_key = SigningKey::from_bytes(&[0x42; 32]);
        let peer_approval =
            cryptex_events::sign_bootstrap_approval(&responder_request, &peer_key).unwrap();
        let transcript = vec![
            prefix[0].clone(),
            prefix[1].clone(),
            String::from_utf8(local_approval).unwrap(),
            String::from_utf8(peer_approval).unwrap(),
        ];

        let (name, listener) = isolated_ipc_listener("bootstrap-completion-no-owner-approval");
        let context = fixture.context.clone();
        let server = thread::spawn(move || {
            let mut stream = listener.accept().unwrap();
            handle_connection_for_peer(&mut stream, Some(&context), None, None, None, true)
                .unwrap();
        });
        let mut client = Stream::connect(name.borrow()).unwrap();
        let response = exchange_bootstrap_approval(
            &mut client,
            &BootstrapApprovalIpcRequest {
                operation: "bootstrap.approval.complete".into(),
                role: None,
                transcript_messages: Some(transcript),
                approval_request_digest: None,
                capability: None,
                ..approval_request("bootstrap.approval.complete")
            },
            Instant::now() + IPC_RESPONSE_DEADLINE,
        )
        .unwrap();
        assert!(!response.accepted);
        assert_eq!(
            response.error.as_deref(),
            Some(if cfg!(target_os = "macos") {
                "platform_peer_identity_unavailable"
            } else {
                "approval_transaction_conflict"
            })
        );
        server.join().unwrap();

        let journal = crate::bootstrap_journal::BootstrapJournalStore::new(&fixture.root)
            .unwrap()
            .load()
            .unwrap();
        assert!(journal.transactions.is_empty());
        fs::remove_dir_all(fixture.root).unwrap();
    }

    #[test]
    fn bootstrap_approval_ipc_cancel_persists_without_generating_signature() {
        let fixture = prepared_search_fixture("bootstrap-approval-cancel");
        let messages = synthetic_initiator_prefix(&fixture);
        let (name, listener) = isolated_ipc_listener("bootstrap-approval-cancel");
        let context = fixture.context.clone();
        let server = thread::spawn(move || {
            let mut stream = listener.accept().unwrap();
            handle_connection_for_peer(&mut stream, Some(&context), None, None, None, true)
                .unwrap();
        });
        let mut client = Stream::connect(name.borrow()).unwrap();
        let prepared = exchange_bootstrap_approval(
            &mut client,
            &BootstrapApprovalIpcRequest {
                transcript_messages: Some(messages.clone()),
                ..approval_request("bootstrap.approval.prepare")
            },
            Instant::now() + IPC_RESPONSE_DEADLINE,
        )
        .unwrap();
        assert!(prepared.accepted);
        let cancelled = exchange_bootstrap_approval(
            &mut client,
            &BootstrapApprovalIpcRequest {
                operation: "bootstrap.approval.cancel".into(),
                transcript_messages: Some(messages),
                approval_request_digest: prepared.approval_request_digest,
                capability: prepared.capability,
                ..approval_request("bootstrap.approval.cancel")
            },
            Instant::now() + IPC_RESPONSE_DEADLINE,
        )
        .unwrap();
        assert!(cancelled.accepted, "{:?}", cancelled.error);
        assert_eq!(cancelled.operation, "bootstrap.approval.cancelled");
        assert!(cancelled.signed_message.is_none());
        server.join().unwrap();

        let journal = crate::bootstrap_journal::BootstrapJournalStore::new(&fixture.root)
            .unwrap()
            .load()
            .unwrap();
        let transaction = &journal.transactions[0];
        assert_eq!(
            transaction.state,
            crate::bootstrap_journal::BootstrapState::Cancelled
        );
        assert!(!transaction.local_approval_signature_generated);
        assert!(!transaction.local_approval_signature_released);
        fs::remove_dir_all(fixture.root).unwrap();
    }

    fn transport_request() -> Request {
        let mut request = search_request("desktop", vec![CAPABILITY_STATUS_READ.into()]);
        request.operation = "transport.status".into();
        request.query = None;
        request
    }

    #[test]
    fn transport_status_requires_exact_desktop_peer_role_capability_and_body() {
        let fixture = prepared_search_fixture("transport-auth");
        for (request, peer, expected) in [
            (transport_request(), false, "unauthorized_client_process"),
            (
                Request {
                    client_role: "mcp".into(),
                    ..transport_request()
                },
                true,
                "unauthorized_client_role",
            ),
            (
                Request {
                    client_role: "cli".into(),
                    ..transport_request()
                },
                true,
                "unauthorized_client_role",
            ),
            (
                Request {
                    protocol_version: PROTOCOL_VERSION + 1,
                    ..transport_request()
                },
                true,
                "incompatible_protocol_version",
            ),
            (
                Request {
                    capabilities: vec![],
                    ..transport_request()
                },
                true,
                "unauthorized_capability",
            ),
            (
                Request {
                    capabilities: vec![
                        CAPABILITY_STATUS_READ.into(),
                        CAPABILITY_CONTENT_SEARCH.into(),
                    ],
                    ..transport_request()
                },
                true,
                "unauthorized_capability",
            ),
            (
                Request {
                    query: Some(DesktopSearchQuery::default()),
                    ..transport_request()
                },
                true,
                "unexpected_request_payload",
            ),
            (
                Request {
                    request_id: String::new(),
                    ..transport_request()
                },
                true,
                "invalid_request_id",
            ),
        ] {
            let response = handle_transport_status(request, Some(&fixture.context), peer);
            assert!(!response.accepted);
            assert_eq!(response.error.as_deref(), Some(expected));
            assert!(!response.configured && !response.protected);
        }
        let response = handle_transport_status(transport_request(), None, true);
        assert_eq!(
            response.error.as_deref(),
            Some("transport_status_unavailable")
        );
        fs::remove_dir_all(fixture.root).unwrap();
    }

    #[test]
    fn transport_status_named_pipe_round_trip_is_small_and_unconfigured() {
        let fixture = prepared_search_fixture("transport-wire");
        let context = fixture.context.clone();
        let (name, listener) = isolated_ipc_listener("transport-status");
        let server = thread::spawn(move || {
            let mut stream = listener.accept().unwrap();
            // OS Desktop identity is exercised separately; this fixture injects
            // the decision to cover exact routing/framing and response bytes.
            handle_connection_for_peer(&mut stream, Some(&context), None, None, None, true)
                .unwrap();
        });
        let mut client = Stream::connect(name.borrow()).unwrap();
        let frame = exchange(&mut client, &transport_request(), 4096).unwrap();
        assert!(frame.len() < 2048);
        let response: DesktopTransportResponse = serde_json::from_slice(&frame).unwrap();
        assert!(response.accepted);
        assert_eq!(
            response.operation.as_deref(),
            Some("transport.status.result")
        );
        assert!(!response.configured && !response.protected);
        assert!(response.native_pid.is_none());
        server.join().unwrap();
        fs::remove_dir_all(fixture.root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn transport_status_reports_sanitized_worker_failure_without_paths() {
        use crate::managed_profile::{
            native::NativePorts,
            worker::{ManagedTransportWorker, WorkerState},
        };
        let mut fixture = prepared_search_fixture("transport-failed-worker");
        let private_path = fixture.root.join("private-package-path-must-not-cross-ipc");
        let mut worker = ManagedTransportWorker::spawn_existing(
            private_path.clone(),
            private_path,
            SecureStoreEnvironment::Dev,
            NativePorts::new(40400, 40401).unwrap(),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while worker.snapshot().unwrap().state != WorkerState::Failed {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        fixture.context.transport_status = Some(worker.status_reader());
        let response = handle_transport_status(transport_request(), Some(&fixture.context), true);
        assert!(response.accepted && response.configured && !response.protected);
        assert_eq!(response.worker_error.as_deref(), Some("package_refused"));
        let bytes = serde_json::to_string(&response).unwrap();
        assert!(!bytes.contains("private-package-path"));
        assert!(!bytes.contains("api_key"));
        assert!(response.native_pid.is_none());
        assert!(worker.shutdown(Duration::from_secs(1)).is_err());
        fs::remove_dir_all(fixture.root).unwrap();
    }

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
            vault: None,
        }
    }

    fn vault_request(role: &str, capabilities: Vec<String>) -> Request {
        Request {
            protocol_version: PROTOCOL_VERSION,
            request_id: "vault-test-request".into(),
            client_role: role.into(),
            capabilities,
            operation: "content.vault.rebuild".into(),
            query: None,
            capture: None,
            capture_resolve: None,
            vault: Some(DesktopVaultBody {
                vault_root: "C:\\Users\\Tarik\\Vault".into(),
            }),
        }
    }

    #[test]
    fn vault_rebuild_requires_authenticated_desktop_process_and_exact_capability() {
        let request = vault_request("desktop", vec![CAPABILITY_CONTENT_VAULT.into()]);
        let response = handle_vault_rebuild(request, None, false);
        assert_eq!(
            response.error.as_deref(),
            Some("unauthorized_client_process")
        );

        let request = vault_request("desktop", vec![CAPABILITY_CONTENT_SEARCH.into()]);
        let response = handle_vault_rebuild(request, None, true);
        assert_eq!(response.error.as_deref(), Some("unauthorized_capability"));

        let request = vault_request("mcp", vec![CAPABILITY_CONTENT_VAULT.into()]);
        let response = handle_vault_rebuild(request, None, true);
        assert_eq!(
            response.error.as_deref(),
            Some("unauthorized_client_process")
        );
    }

    #[test]
    fn vault_destination_guard_rejects_repository_and_accepts_external_directory() {
        let repository = std::fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .expect("repository path");
        assert!(is_inside_source_repository(&repository));
        #[cfg(windows)]
        assert!(is_inside_source_repository(&PathBuf::from(
            repository.to_string_lossy().to_uppercase()
        )));
        let external = exclusive_temp_dir("cryptex-vault-external-destination");
        let external = std::fs::canonicalize(external).expect("external path");
        assert!(!is_inside_source_repository(&external));
    }

    #[test]
    fn authenticated_vault_request_rebuilds_from_verified_history() {
        let fixture = prepared_search_fixture("vault-ipc-rebuild");
        let vault = exclusive_temp_dir("cryptex-ipc-vault-target");
        let mut request = vault_request("desktop", vec![CAPABILITY_CONTENT_VAULT.into()]);
        request.vault = Some(DesktopVaultBody {
            vault_root: vault.to_string_lossy().into_owned(),
        });
        let response = handle_vault_rebuild(request, Some(&fixture.context), true);
        assert!(response.accepted, "{:?}", response.error);
        assert_eq!(response.note_count, Some(1));
        let open_note_path = response
            .open_note_path
            .as_deref()
            .expect("generated note path");
        assert!(open_note_path.starts_with("notes/"));
        assert!(open_note_path.ends_with(".md"));
        assert!(vault.join("Cryptex").join(open_note_path).is_file());
        assert!(vault.join("Cryptex").join("notes").is_dir());
        fs::remove_dir_all(vault).expect("remove temporary vault");
        fs::remove_dir_all(fixture.root).expect("remove IPC fixture");
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
            #[cfg(windows)]
            transport_status: None,
            #[cfg(test)]
            transport_device_id_override: Some(
                "AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA".into(),
            ),
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
        let cleanup_deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match fs::remove_dir_all(&fixture.root) {
                Ok(()) => break,
                Err(error) if error.kind() == io::ErrorKind::NotFound => break,
                Err(error) if Instant::now() < cleanup_deadline => {
                    // The catch-up worker exits after its last sender drops;
                    // on Windows, wait briefly for SQLite's handle to close.
                    let _ = error;
                    thread::sleep(Duration::from_millis(20));
                }
                Err(error) => panic!("cleanup fixture after worker exit: {error}"),
            }
        }
    }

    fn content_search_ipc_round_trip(
        context: Option<SearchContext>,
        query: DesktopSearchQuery,
    ) -> DesktopSearchResponse {
        let (name, listener) = isolated_ipc_listener("content-search");
        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("accept content search client");
            handle_connection_for_peer(&mut stream, context.as_ref(), None, None, None, true)
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

    fn content_timeline_ipc_round_trip(
        context: Option<SearchContext>,
        query: DesktopSearchQuery,
    ) -> DesktopSearchResponse {
        let (name, listener) = isolated_ipc_listener("content-timeline");
        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("accept timeline client");
            handle_connection_for_peer(&mut stream, context.as_ref(), None, None, None, true)
                .expect("serve timeline request");
        });
        let mut client = Stream::connect(name.borrow()).expect("connect timeline client");
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut request = search_request("desktop", vec![CAPABILITY_CONTENT_SEARCH.into()]);
        request.operation = "content.timeline".into();
        request.query = Some(query);
        let mut frame = serde_json::to_vec(&request).expect("encode timeline request");
        frame.push(b'\n');
        write_ipc_all(&mut client, &frame, deadline).expect("write timeline request");
        let response_frame = read_ipc_frame(&mut client, MAX_SEARCH_RESPONSE_BYTES, deadline)
            .expect("read timeline response");
        let response = serde_json::from_slice(&response_frame).expect("decode timeline response");
        server.join().expect("timeline server thread");
        response
    }

    fn content_event_detail_ipc_round_trip(
        context: Option<SearchContext>,
        event_id: String,
    ) -> DesktopSearchResponse {
        let (name, listener) = isolated_ipc_listener("content-event-detail");
        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("accept event detail client");
            handle_connection_for_peer(&mut stream, context.as_ref(), None, None, None, true)
                .expect("serve event detail request");
        });
        let mut client = Stream::connect(name.borrow()).expect("connect event detail client");
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut request = search_request("desktop", vec![CAPABILITY_CONTENT_SEARCH.into()]);
        request.operation = "content.event.detail".into();
        request.query = Some(DesktopSearchQuery {
            event_id: Some(event_id),
            ..DesktopSearchQuery::default()
        });
        let mut frame = serde_json::to_vec(&request).expect("encode event detail request");
        frame.push(b'\n');
        write_ipc_all(&mut client, &frame, deadline).expect("write event detail request");
        let response_frame = read_ipc_frame(&mut client, MAX_DETAIL_RESPONSE_BYTES, deadline)
            .expect("read event detail response");
        let response =
            serde_json::from_slice(&response_frame).expect("decode event detail response");
        server.join().expect("event detail server thread");
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
        assert!(
            DesktopSearchQuery {
                text: Some("not allowed in a timeline request".into()),
                ..DesktopSearchQuery::default()
            }
            .validate_timeline_and_convert()
            .is_err()
        );
        assert!(
            DesktopSearchQuery {
                limit: Some(MAX_TIMELINE_PAGE_SIZE + 1),
                ..DesktopSearchQuery::default()
            }
            .validate_timeline_and_convert()
            .is_err()
        );
    }

    #[test]
    fn desktop_timeline_returns_verified_bounded_snippets_and_provenance() {
        let fixture = prepared_search_fixture("timeline-allowed");
        let response = content_timeline_ipc_round_trip(
            Some(fixture.context.clone()),
            DesktopSearchQuery {
                limit: Some(1),
                ..DesktopSearchQuery::default()
            },
        );
        assert!(response.accepted, "unexpected rejection: {response:?}");
        assert_eq!(
            response.operation.as_deref(),
            Some("content.timeline.result")
        );
        assert!(response.hold_reason.is_none());
        let hits = response
            .hits
            .expect("guarded timeline contains verified notes");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].brain_id, fixture.brain_id);
        assert_eq!(hits[0].device_id, fixture.device.as_str());
        assert_eq!(hits[0].text, "synthetic IPC search fixture note");
        assert!(!hits[0].content_hash.is_empty());
        assert!(!hits[0].signing_key_id.is_empty());
        assert!(!response.has_more);
        assert!(response.next_cursor.is_none());
        fs::remove_dir_all(fixture.root).expect("cleanup fixture");
    }

    #[test]
    fn desktop_timeline_withholds_every_row_when_materialization_is_stale() {
        let fixture = prepared_search_fixture("timeline-stale");
        let db = ProjectionDb::open_or_create(&fixture.root).expect("open projection db");
        db.connection()
            .execute_batch("DROP TABLE knowledge_meta;")
            .expect("remove M4 materialization metadata");
        drop(db);

        let response = content_timeline_ipc_round_trip(
            Some(fixture.context.clone()),
            DesktopSearchQuery::default(),
        );
        assert!(response.accepted);
        assert!(response.hits.is_none());
        assert_eq!(
            response.hold_reason.as_deref(),
            Some("materialization_state_missing_or_stale")
        );
        fs::remove_dir_all(fixture.root).expect("cleanup fixture");
    }

    #[test]
    fn timeline_ipc_requires_authenticated_desktop_and_exact_search_capability() {
        let mut non_desktop = search_request("cli", vec![CAPABILITY_CONTENT_SEARCH.into()]);
        non_desktop.operation = "content.timeline".into();
        let response = handle_timeline_until(non_desktop, None, true, None);
        assert_eq!(response.error.as_deref(), Some("unauthorized_client_role"));

        let mut wrong_capability = search_request(
            "desktop",
            vec![CAPABILITY_CONTENT_SEARCH.into(), "status.read".into()],
        );
        wrong_capability.operation = "content.timeline".into();
        let response = handle_timeline_until(wrong_capability, None, true, None);
        assert_eq!(response.error.as_deref(), Some("unauthorized_capability"));

        let mut wrong_peer = search_request("desktop", vec![CAPABILITY_CONTENT_SEARCH.into()]);
        wrong_peer.operation = "content.timeline".into();
        let response = handle_timeline_until(wrong_peer, None, false, None);
        assert_eq!(
            response.error.as_deref(),
            Some("unauthorized_client_process")
        );
    }

    #[test]
    fn desktop_event_detail_returns_full_note_only_after_guarded_verification() {
        let fixture = prepared_search_fixture("event-detail-allowed");
        let projection = ProjectionDb::open_or_create(&fixture.root).expect("projection db");
        let seeded =
            cryptex_projection::search_events(projection.connection(), &SearchQuery::default())
                .expect("read seeded event id");
        let event_id = seeded[0].event_id.clone();
        drop(projection);

        let response = content_event_detail_ipc_round_trip(Some(fixture.context.clone()), event_id);
        assert!(response.accepted, "unexpected rejection: {response:?}");
        assert_eq!(
            response.operation.as_deref(),
            Some("content.event.detail.result")
        );
        let hits = response.hits.expect("guarded detail contains the note");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].text, "synthetic IPC search fixture note");
        assert_eq!(hits[0].brain_id, fixture.brain_id);
        assert!(!hits[0].content_hash.is_empty());
        fs::remove_dir_all(fixture.root).expect("cleanup fixture");
    }

    #[test]
    fn desktop_event_detail_withholds_note_body_when_materialization_is_stale() {
        let fixture = prepared_search_fixture("event-detail-stale");
        let projection = ProjectionDb::open_or_create(&fixture.root).expect("projection db");
        let seeded =
            cryptex_projection::search_events(projection.connection(), &SearchQuery::default())
                .expect("read seeded event id");
        let event_id = seeded[0].event_id.clone();
        projection
            .connection()
            .execute_batch("DROP TABLE knowledge_meta;")
            .expect("remove M4 materialization metadata");
        drop(projection);

        let response = content_event_detail_ipc_round_trip(Some(fixture.context.clone()), event_id);
        assert!(response.accepted);
        assert!(response.hits.is_none());
        assert_eq!(
            response.hold_reason.as_deref(),
            Some("materialization_state_missing_or_stale")
        );
        fs::remove_dir_all(fixture.root).expect("cleanup fixture");
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
            #[cfg(windows)]
            transport_status: None,
            #[cfg(test)]
            transport_device_id_override: None,
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
            #[cfg(windows)]
            transport_status: None,
            #[cfg(test)]
            transport_device_id_override: None,
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
            #[cfg(windows)]
            transport_status: None,
            #[cfg(test)]
            transport_device_id_override: None,
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
            vault: None,
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
                vault: None,
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
            vault: None,
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

    #[cfg(windows)]
    struct ChildProcessGuard {
        process: std::process::Child,
        ready_path: std::path::PathBuf,
    }

    #[cfg(windows)]
    impl Drop for ChildProcessGuard {
        fn drop(&mut self) {
            if self.process.try_wait().ok().flatten().is_none() {
                let _ = self.process.kill();
            }
            let _ = self.process.wait();
            let _ = fs::remove_file(&self.ready_path);
        }
    }

    #[cfg(windows)]
    #[test]
    fn desktop_process_identity_rejects_exited_peer_with_matching_pid() {
        use std::io::Read;
        use std::os::windows::io::FromRawHandle;
        use std::process::{Command, Stdio};
        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE};

        const WORKER_ENV: &str = "CRYPTEX_TEST_IPC_PEER_IDENTITY_WORKER";
        const WORKER_PIPE_ENV: &str = "CRYPTEX_TEST_IPC_PEER_IDENTITY_PIPE";
        const READY_ENV: &str = "CRYPTEX_TEST_IPC_PEER_IDENTITY_READY";
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let pipe_name = format!("cryptex-ipc-peer-identity-{}-{suffix}", process::id());
        let name = std::ffi::OsString::from(&pipe_name)
            .to_ns_name::<GenericNamespaced>()
            .expect("test pipe name");
        let listener = ListenerOptions::new()
            .name(name.borrow())
            .create_sync()
            .expect("test listener");
        let ready_path = std::env::temp_dir().join(format!(
            "cryptex-ipc-peer-identity-{}-{suffix}.ready",
            process::id()
        ));
        let executable = std::env::current_exe().expect("test executable path");
        let mut child = ChildProcessGuard {
            process: Command::new(executable)
                .args([
                    "--exact",
                    "ipc::tests::desktop_process_identity_worker_entry",
                    "--nocapture",
                ])
                .env(WORKER_ENV, "1")
                .env(WORKER_PIPE_ENV, &pipe_name)
                .env(READY_ENV, &ready_path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn identity worker"),
            ready_path,
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        while !child.ready_path.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            child.ready_path.is_file(),
            "identity worker started; status={:?}",
            child.process.try_wait().expect("check worker status")
        );
        let pid = child.process.id();
        let mut peer = listener.accept().expect("accept identity worker pipe");
        let mut handshake = [0; 5];
        peer.read_exact(&mut handshake)
            .expect("read worker pipe handshake");
        assert_eq!(&handshake, b"READY");
        let peer_pid = peer_process_id(&peer).expect("read connected worker PID");
        assert_eq!(peer_pid, pid, "kernel pipe credential names exact worker");
        let process = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
        assert!(!process.is_null(), "open exact child process handle");
        let identity = DesktopProcessIdentity {
            pid,
            process: unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(process) },
        };

        assert!(
            identity.matches_peer(peer_pid),
            "live pinned process is accepted"
        );
        child.process.kill().expect("terminate identity worker");
        child.process.wait().expect("reap identity worker");
        assert!(
            !identity.matches_peer(peer_pid),
            "a dead pinned process is rejected even when the peer PID matches"
        );
    }

    #[cfg(windows)]
    #[test]
    fn desktop_process_identity_worker_entry() {
        if std::env::var_os("CRYPTEX_TEST_IPC_PEER_IDENTITY_WORKER").is_some()
            && let (Some(pipe_name), Some(ready_path)) = (
                std::env::var_os("CRYPTEX_TEST_IPC_PEER_IDENTITY_PIPE"),
                std::env::var_os("CRYPTEX_TEST_IPC_PEER_IDENTITY_READY"),
            )
        {
            use std::io::{Read, Write};
            let name = pipe_name
                .to_ns_name::<GenericNamespaced>()
                .expect("worker pipe name");
            let mut stream = Stream::connect(name.borrow()).expect("connect worker pipe");
            stream.write_all(b"READY").expect("write pipe handshake");
            fs::write(ready_path, b"READY").expect("write worker readiness marker");
            let mut byte = [0];
            while stream.read(&mut byte).expect("read worker pipe") != 0 {}
            loop {
                thread::sleep(Duration::from_secs(60));
            }
        }
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
            handle_connection(&mut stream, None, None, None, None).expect("health connection");
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
            vault: None,
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
            vault: None,
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
                vault: None,
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
                vault: None,
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
                vault: None,
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
