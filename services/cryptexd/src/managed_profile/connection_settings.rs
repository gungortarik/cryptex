//! Non-authoritative public LAN preferences. No paths, API ports or credentials
//! cross this wire. Only the authenticated owner adapter may call the backend.
use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, SocketAddrV4};

pub const MAX_FRAME: usize = 64 * 1024;
pub(super) const MAX_PEERS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    InvalidRequest,
    Unauthorized,
    Unavailable,
    NotEnrolled,
    StaleRevision,
    UnknownDevice,
    InvalidEndpoint,
    Storage,
    PendingSettings,
    Authority,
    LocalRestricted,
    Shutdown,
    Start,
    StartRestDeadline,
    #[cfg(all(windows, feature = "m8-start-inspection"))]
    StartFailure {
        stage: StartStage,
        code: u16,
        detail: Option<i64>,
    },
    Held,
}
#[cfg(windows)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartStage {
    Bootstrap,
    Exchange,
    IdentityRead,
    IdentityMismatch,
    Monitor,
}

impl Error {
    pub fn public_error(self) -> Self {
        #[cfg(all(windows, feature = "m8-start-inspection"))]
        if let Self::StartFailure { stage, code, .. } = self {
            return if stage == StartStage::Exchange && code == 402 {
                Self::StartRestDeadline
            } else {
                Self::Start
            };
        }
        self
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "connection_settings_{self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PeerAddress {
    pub device_id: String,
    pub endpoint: SocketAddrV4,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Update {
    pub expected_revision: String,
    pub paused: bool,
    pub listen_address: Option<Ipv4Addr>,
    pub peers: Vec<PeerAddress>,
}
impl Update {
    pub fn validate(&self) -> Result<u64, Error> {
        let revision = self
            .expected_revision
            .parse::<u64>()
            .map_err(|_| Error::InvalidRequest)?;
        if self.expected_revision != revision.to_string() || self.peers.len() > MAX_PEERS {
            return Err(Error::InvalidRequest);
        }
        if self.listen_address.is_some_and(|ip| !ip.is_private()) {
            return Err(Error::InvalidEndpoint);
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut addresses = std::collections::BTreeSet::new();
        for peer in &self.peers {
            if cryptex_core::DeviceId::new(peer.device_id.clone()).is_err()
                || !ids.insert(peer.device_id.as_str())
            {
                return Err(Error::InvalidRequest);
            }
            if !peer.endpoint.ip().is_private()
                || peer.endpoint.port() == 0
                || !addresses.insert(peer.endpoint)
            {
                return Err(Error::InvalidEndpoint);
            }
        }
        Ok(revision)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub schema_version: u32,
    pub request_id: String,
    pub operation: String,
    pub client_role: String,
    pub update: Option<Update>,
}
impl Request {
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.is_empty() || bytes.len() > MAX_FRAME {
            return Err(Error::InvalidRequest);
        }
        let text = std::str::from_utf8(bytes).map_err(|_| Error::InvalidRequest)?;
        cryptex_events::reject_duplicate_json_keys(text).map_err(|_| Error::InvalidRequest)?;
        let request: Self = serde_json::from_str(text).map_err(|_| Error::InvalidRequest)?;
        if request.schema_version != 1
            || request.client_role != "desktop"
            || request.request_id.is_empty()
            || request.request_id.len() > 64
            || !request.request_id.bytes().all(|b| b.is_ascii_hexdigit())
            || !matches!(
                (request.operation.as_str(), request.update.is_some()),
                ("connections.read", false) | ("connections.update", true)
            )
        {
            return Err(Error::InvalidRequest);
        }
        if let Some(update) = &request.update {
            update.validate()?;
        }
        Ok(request)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    NotEnrolled,
    Paused,
    AddressNeeded,
    Applying,
    ReadyLocally,
    Attention,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Device {
    pub device_id: String,
    pub transport_id: String,
    pub endpoint: Option<SocketAddrV4>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct View {
    pub revision: String,
    pub paused: bool,
    pub listen_address: Option<Ipv4Addr>,
    pub local_transport_port: Option<u16>,
    pub devices: Vec<Device>,
    pub state: State,
    pub error: Option<Error>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Response {
    pub schema_version: u32,
    pub request_id: String,
    pub accepted: bool,
    pub error: Option<Error>,
    pub view: Option<View>,
}

#[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
pub(crate) mod backend;
#[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
pub use backend::{Handle, Location, Owner};

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> serde_json::Value {
        serde_json::json!({"schemaVersion":1,"requestId":"a1","operation":"connections.update",
            "clientRole":"desktop","update":{"expectedRevision":"0","paused":false,
            "listenAddress":"192.168.1.2","peers":[]}})
    }
    #[test]
    fn closed_wire_refuses_unknown_roles_fields_duplicates_and_public_networks() {
        let good = request();
        assert!(Request::decode(&serde_json::to_vec(&good).unwrap()).is_ok());
        for (key, value) in [
            ("schemaVersion", serde_json::json!(2)),
            ("clientRole", serde_json::json!("ai")),
            ("operation", serde_json::json!("helper.start")),
            ("path", serde_json::json!("/tmp/x")),
        ] {
            let mut bad = good.clone();
            bad[key] = value;
            assert!(Request::decode(&serde_json::to_vec(&bad).unwrap()).is_err());
        }
        for ip in ["8.8.8.8", "127.0.0.1", "0.0.0.0", "169.254.1.1"] {
            let mut bad = good.clone();
            bad["update"]["listenAddress"] = serde_json::json!(ip);
            assert!(Request::decode(&serde_json::to_vec(&bad).unwrap()).is_err());
        }
        let text = serde_json::to_string(&good).unwrap();
        assert!(
            Request::decode(text.replacen('{', "{\"schemaVersion\":1,", 1).as_bytes()).is_err()
        );
    }
}
