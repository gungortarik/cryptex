//! HTTP over an already connected, OS-attested owned-job socket. The HTTP
//! engine receives that exact socket, never a URL connector/DNS/proxy/pool.
//! Owning a job is not a package pin, Cryptex peer trust or incoming admission.

use std::fmt;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use bytes::Bytes;
use cryptex_events::reject_duplicate_json_keys;
use futures_util::future::{Either, select};
use http_body_util::{BodyExt, Empty};
use hyper::header::{ACCEPT, CONNECTION, CONTENT_ENCODING, CONTENT_LENGTH, HOST};
use hyper::{Method, Request};
use hyper_util::rt::TokioIo;
use serde::Deserialize;

use crate::owned_process::OwnedProcess;
pub use crate::owned_process::endpoint::EndpointError;
use crate::owned_process::endpoint::validate_loopback;
use crate::syncthing_rest::{ApiCredential, TransportObservation, native_id_shape};

const BUDGET: Duration = Duration::from_secs(3);
const MAX_BODY: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnedRestError {
    Endpoint(EndpointError),
    InvalidIdentity,
    RuntimeUnavailable,
    Deadline,
    Unavailable,
    InvalidResponse,
    ResponseTooLarge,
    Unauthorized(u16),
    RedirectRefused,
    HttpStatus(u16),
    VersionMismatch,
    IdentityMismatch,
}

impl fmt::Display for OwnedRestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "owned_rest_{self:?}")
    }
}
impl std::error::Error for OwnedRestError {}

/// No public arbitrary operation/path/body API. No Clone/Serialize support.
/// All authenticated requests first attest the new established connection.
pub struct OwnedSyncthingRest<'a> {
    owner: &'a OwnedProcess,
    endpoint: SocketAddr,
    expected_identity: String,
    credential: ApiCredential,
}

impl fmt::Debug for OwnedSyncthingRest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OwnedSyncthingRest(<backend-owned connection>)")
    }
}

#[derive(Clone, Copy)]
enum Operation {
    Version,
    Status,
    Shutdown,
    #[cfg(test)]
    Config,
}
impl Operation {
    fn path(self) -> &'static str {
        match self {
            Self::Version => "/rest/system/version",
            Self::Status => "/rest/system/status",
            Self::Shutdown => "/rest/system/shutdown",
            #[cfg(test)]
            Self::Config => "/rest/config",
        }
    }
}

impl<'a> OwnedSyncthingRest<'a> {
    pub fn new(
        owner: &'a OwnedProcess,
        endpoint: SocketAddr,
        expected_identity: &str,
        credential: ApiCredential,
    ) -> Result<Self, OwnedRestError> {
        validate_loopback(endpoint).map_err(OwnedRestError::Endpoint)?;
        if !native_id_shape(expected_identity) {
            return Err(OwnedRestError::InvalidIdentity);
        }
        Ok(Self {
            owner,
            endpoint,
            expected_identity: expected_identity.to_owned(),
            credential,
        })
    }

    pub fn observe(&self) -> Result<TransportObservation, OwnedRestError> {
        self.observe_with_budget(BUDGET)
    }

    pub(crate) fn observe_with_budget(
        &self,
        budget: Duration,
    ) -> Result<TransportObservation, OwnedRestError> {
        let deadline = Instant::now()
            .checked_add(budget)
            .ok_or(OwnedRestError::Deadline)?;
        self.observe_before(deadline)
    }

    fn observe_before(&self, deadline: Instant) -> Result<TransportObservation, OwnedRestError> {
        let version: Version = json(&self.request(Operation::Version, deadline)?)?;
        if version.version != "v2.1.5" {
            return Err(OwnedRestError::VersionMismatch);
        }
        let status: Status = json(&self.request(Operation::Status, deadline)?)?;
        if status.my_id != self.expected_identity {
            return Err(OwnedRestError::IdentityMismatch);
        }
        Ok(TransportObservation {
            engine_version: "v2.1.5",
            expected_native_identity_matches: true,
        })
    }

    /// Request shutdown only through the owned connection. A successful HTTP
    /// response is NOT proof of exit: controller must separately wait on the
    /// owned job, retaining forced cleanup as failure.
    pub fn request_shutdown(&self) -> Result<(), OwnedRestError> {
        let deadline = Instant::now() + BUDGET;
        self.observe_before(deadline)?;
        self.request(Operation::Shutdown, deadline).map(|_| ())
    }

    #[cfg(test)]
    pub(crate) fn test_read_config(&self) -> Result<Vec<u8>, OwnedRestError> {
        self.request(Operation::Config, Instant::now() + BUDGET)
    }

    fn request(&self, operation: Operation, deadline: Instant) -> Result<Vec<u8>, OwnedRestError> {
        // No credential bytes are passed to TCP until its established peer's
        // live handle has been proved a member of our retained owned job.
        let verified = self
            .owner
            .connect_owned_loopback(self.endpoint, deadline)
            .map_err(OwnedRestError::Endpoint)?;
        let (stream, _guard) = verified.into_parts();
        let _remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|v| !v.is_zero())
            .ok_or(OwnedRestError::Deadline)?;
        // Synchronous controller API; refuse nested runtimes rather than panic.
        if tokio::runtime::Handle::try_current().is_ok() {
            return Err(OwnedRestError::RuntimeUnavailable);
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| OwnedRestError::RuntimeUnavailable)?;
        runtime.block_on(async {
            tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), async {
                let stream = tokio::net::TcpStream::from_std(stream)
                    .map_err(|_| OwnedRestError::Unavailable)?;
                let (mut sender, connection) = hyper::client::conn::http1::Builder::new()
                    .max_headers(64)
                    .max_buf_size(16 * 1024)
                    .handshake(TokioIo::new(stream))
                    .await
                    .map_err(|_| OwnedRestError::InvalidResponse)?;
                let method = if matches!(operation, Operation::Shutdown) {
                    Method::POST
                } else {
                    Method::GET
                };
                let request = Request::builder()
                    .method(method)
                    .uri(operation.path())
                    .header(HOST, self.endpoint.to_string())
                    .header(ACCEPT, "application/json")
                    .header(CONNECTION, "close")
                    .header("X-API-Key", self.credential.0.clone())
                    .body(Empty::<Bytes>::new())
                    .map_err(|_| OwnedRestError::Unavailable)?;
                let response = async move {
                    let response = sender
                        .send_request(request)
                        .await
                        .map_err(|_| OwnedRestError::InvalidResponse)?;
                    drop(sender);
                    let code = response.status().as_u16();
                    if (300..400).contains(&code) {
                        return Err(OwnedRestError::RedirectRefused);
                    }
                    if matches!(code, 401 | 403) {
                        return Err(OwnedRestError::Unauthorized(code));
                    }
                    if code != 200 {
                        return Err(OwnedRestError::HttpStatus(code));
                    }
                    if response.headers().contains_key(CONTENT_ENCODING) {
                        return Err(OwnedRestError::InvalidResponse);
                    }
                    if let Some(value) = response.headers().get(CONTENT_LENGTH) {
                        let size = value
                            .to_str()
                            .ok()
                            .and_then(|v| v.parse::<u64>().ok())
                            .ok_or(OwnedRestError::InvalidResponse)?;
                        if size > MAX_BODY as u64 {
                            return Err(OwnedRestError::ResponseTooLarge);
                        }
                    }
                    let mut body = response.into_body();
                    let mut bytes = Vec::new();
                    while let Some(frame) = body.frame().await {
                        let frame = frame.map_err(|_| OwnedRestError::InvalidResponse)?;
                        if let Some(chunk) = frame.data_ref() {
                            if chunk.len() > MAX_BODY - bytes.len() {
                                return Err(OwnedRestError::ResponseTooLarge);
                            }
                            bytes.extend_from_slice(chunk);
                        }
                    }
                    Ok(bytes)
                };
                // Drive standard HTTP parsing on the SAME verified socket.
                // If the driver completes first, finish consuming its queued
                // response/body result under the shared deadline. Never reconnect.
                match select(Box::pin(response), Box::pin(connection)).await {
                    Either::Left((result, _driver)) => result,
                    Either::Right((_driver_result, response)) => response.await,
                }
            })
            .await
            .map_err(|_| OwnedRestError::Deadline)?
        })
    }
}

fn json<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, OwnedRestError> {
    let text = std::str::from_utf8(bytes).map_err(|_| OwnedRestError::InvalidResponse)?;
    reject_duplicate_json_keys(text).map_err(|_| OwnedRestError::InvalidResponse)?;
    serde_json::from_str(text).map_err(|_| OwnedRestError::InvalidResponse)
}
#[derive(Deserialize)]
struct Version {
    version: String,
}
#[derive(Deserialize)]
struct Status {
    #[serde(rename = "myID")]
    my_id: String,
}
