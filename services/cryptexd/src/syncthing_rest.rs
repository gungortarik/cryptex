//! Shared native API credentials and observations. The earlier URL-based
//! read-only adapter is now test-only: product HTTP uses syncthing_owned_rest's
//! already-connected owned socket, with no reconnect/proxy/pool fallback.

use hyper::header::HeaderValue;
use std::fmt;
#[cfg(test)]
use {
    cryptex_events::reject_duplicate_json_keys,
    reqwest::blocking::Client,
    reqwest::header::{ACCEPT, CONNECTION, HeaderMap},
    reqwest::redirect::Policy,
    serde::Deserialize,
    std::io::Read,
    std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    std::time::{Duration, Instant},
};

use crate::secure_store::{SecureStore, SecureStoreEnvironment};

#[cfg(test)]
const PINNED_VERSION: &str = "v2.1.5";
#[cfg(test)]
const MAX_RESPONSE_BYTES: u64 = 64 * 1024;
#[cfg(test)]
const OBSERVATION_BUDGET: Duration = Duration::from_secs(3);
const MAX_API_KEY_BYTES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncthingRestError {
    InvalidEndpoint,
    InvalidExpectedIdentity,
    InvalidCredential,
    CredentialUnavailable,
    ClientUnavailable,
    Unavailable,
    Deadline,
    Unauthorized,
    RedirectRefused,
    HttpStatus(u16),
    ResponseTooLarge,
    InvalidResponse,
    VersionMismatch,
    IdentityMismatch,
}

impl fmt::Display for SyncthingRestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Never embed reqwest errors (which can carry a URL), bodies or keys.
        write!(f, "syncthing_rest_{self:?}")
    }
}

impl std::error::Error for SyncthingRestError {}

/// Secret held only in the native backend; no Serialize/Clone/Display support.
pub struct ApiCredential(pub(crate) HeaderValue);

impl ApiCredential {
    /// Read an existing managed-profile credential. Never create a replacement
    /// on missing/corrupt/denied SecureStore state. Profile IDs are opaque,
    /// bounded app-assigned identifiers, not paths or user-provided service names.
    pub fn load_existing<S: SecureStore + ?Sized>(
        store: &S,
        environment: SecureStoreEnvironment,
        profile_id: &str,
    ) -> Result<Self, SyncthingRestError> {
        let (service, account) = credential_labels(environment, profile_id)?;
        let key = store
            .get(&service, &account)
            .map_err(|_| SyncthingRestError::CredentialUnavailable)?;
        Self::from_bytes(&key)
    }

    pub(crate) fn from_bytes(key: &[u8]) -> Result<Self, SyncthingRestError> {
        if key.is_empty()
            || key.len() > MAX_API_KEY_BYTES
            || !key.iter().all(|byte| byte.is_ascii_graphic())
        {
            return Err(SyncthingRestError::InvalidCredential);
        }
        let mut header =
            HeaderValue::from_bytes(key).map_err(|_| SyncthingRestError::InvalidCredential)?;
        header.set_sensitive(true);
        Ok(Self(header))
    }
}

pub(crate) fn credential_labels(
    environment: SecureStoreEnvironment,
    profile_id: &str,
) -> Result<(String, String), SyncthingRestError> {
    if profile_id.is_empty()
        || profile_id.len() > 64
        || !profile_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(SyncthingRestError::InvalidCredential);
    }
    Ok((
        format!("{}-ManagedSyncthing", environment.service_prefix()),
        format!("api:{profile_id}"),
    ))
}

impl fmt::Debug for ApiCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiCredential(<redacted>)")
    }
}

/// Successful observation of the expected pinned REST instance only. Does not
/// claim process ownership, authenticated Cryptex peers or protected replication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportObservation {
    pub engine_version: &'static str,
    pub expected_native_identity_matches: bool,
}

#[cfg(test)]
pub struct SyncthingRest {
    client: Client,
    endpoint: SocketAddr,
    expected_identity: String,
}

#[cfg(test)]
impl SyncthingRest {
    pub fn for_owned_profile(
        endpoint: SocketAddr,
        expected_native_identity: &str,
        credential: ApiCredential,
    ) -> Result<Self, SyncthingRestError> {
        // Numeric literal only: no DNS, userinfo, path, query or arbitrary host.
        if endpoint.port() == 0
            || !matches!(endpoint.ip(), IpAddr::V4(ip) if ip == Ipv4Addr::LOCALHOST)
                && !matches!(endpoint.ip(), IpAddr::V6(ip) if ip == Ipv6Addr::LOCALHOST)
        {
            return Err(SyncthingRestError::InvalidEndpoint);
        }
        // Native DeviceID syntax only. The future owned-process adapter must
        // obtain/validate the actual identity from its generated profile. A
        // response echoing this value alone is not process authentication.
        if !native_id_shape(expected_native_identity) {
            return Err(SyncthingRestError::InvalidExpectedIdentity);
        }
        let mut headers = HeaderMap::new();
        headers.insert("X-API-Key", credential.0);
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(CONNECTION, HeaderValue::from_static("close"));
        let client = Client::builder()
            .default_headers(headers)
            .no_proxy()
            .redirect(Policy::none())
            // Remain explicit even if another workspace member enables these
            // features through Cargo feature unification in a Desktop build.
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .timeout(OBSERVATION_BUDGET)
            .connect_timeout(OBSERVATION_BUDGET)
            .pool_max_idle_per_host(0)
            .build()
            .map_err(|_| SyncthingRestError::ClientUnavailable)?;
        Ok(Self {
            client,
            endpoint,
            expected_identity: expected_native_identity.to_owned(),
        })
    }

    pub fn observe(&self) -> Result<TransportObservation, SyncthingRestError> {
        self.observe_with_budget(OBSERVATION_BUDGET)
    }

    fn observe_with_budget(
        &self,
        budget: Duration,
    ) -> Result<TransportObservation, SyncthingRestError> {
        let deadline = Instant::now() + budget;
        let version: VersionResponse = self.get_json("/rest/system/version", deadline)?;
        if version.version != PINNED_VERSION {
            return Err(SyncthingRestError::VersionMismatch);
        }
        let status: StatusResponse = self.get_json("/rest/system/status", deadline)?;
        if status.my_id != self.expected_identity {
            return Err(SyncthingRestError::IdentityMismatch);
        }
        if Instant::now() >= deadline {
            return Err(SyncthingRestError::Deadline);
        }
        Ok(TransportObservation {
            engine_version: PINNED_VERSION,
            expected_native_identity_matches: true,
        })
    }

    // Paths are selected internally above; no arbitrary endpoint/path API.
    fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &'static str,
        deadline: Instant,
    ) -> Result<T, SyncthingRestError> {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or(SyncthingRestError::Deadline)?;
        let response = self
            .client
            .get(format!("http://{}{path}", self.endpoint))
            .timeout(remaining)
            .send()
            .map_err(|error| {
                if error.is_timeout() {
                    SyncthingRestError::Deadline
                } else {
                    SyncthingRestError::Unavailable
                }
            })?;
        let status = response.status();
        if status.is_redirection() {
            return Err(SyncthingRestError::RedirectRefused);
        }
        if matches!(status.as_u16(), 401 | 403) {
            return Err(SyncthingRestError::Unauthorized);
        }
        if status.as_u16() != 200 {
            return Err(SyncthingRestError::HttpStatus(status.as_u16()));
        }
        if response
            .content_length()
            .is_some_and(|bytes| bytes > MAX_RESPONSE_BYTES)
        {
            return Err(SyncthingRestError::ResponseTooLarge);
        }
        // Limit while streaming, including chunked/unknown-length responses;
        // never buffer an unbounded body and check its size afterward.
        let mut body = Vec::new();
        response
            .take(MAX_RESPONSE_BYTES + 1)
            .read_to_end(&mut body)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::TimedOut || Instant::now() >= deadline {
                    SyncthingRestError::Deadline
                } else {
                    SyncthingRestError::InvalidResponse
                }
            })?;
        if Instant::now() >= deadline {
            return Err(SyncthingRestError::Deadline);
        }
        if body.len() as u64 > MAX_RESPONSE_BYTES {
            return Err(SyncthingRestError::ResponseTooLarge);
        }
        let text = std::str::from_utf8(&body).map_err(|_| SyncthingRestError::InvalidResponse)?;
        reject_duplicate_json_keys(text).map_err(|_| SyncthingRestError::InvalidResponse)?;
        serde_json::from_str(text).map_err(|_| SyncthingRestError::InvalidResponse)
    }
}

#[cfg(test)]
impl fmt::Debug for SyncthingRest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SyncthingRest(<backend profile redacted>)")
    }
}

#[cfg(test)]
#[derive(Deserialize)]
struct VersionResponse {
    version: String,
}

#[cfg(test)]
#[derive(Deserialize)]
struct StatusResponse {
    #[serde(rename = "myID")]
    my_id: String,
}

pub(crate) fn native_id_shape(id: &str) -> bool {
    id.len() == 63
        && id.split('-').count() == 8
        && id.split('-').all(|group| {
            group.len() == 7
                && group
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || (b'2'..=b'7').contains(&b))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::thread::{self, JoinHandle};

    use crate::secure_store::{FakeSecureStore, SecureStoreError};

    // Synthetic syntax fixture; not claimed as a checksum-valid native identity.
    const ID: &str = "AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA";
    const SECRET: &[u8] = b"SYNTHETIC_TEST_API_KEY_NOT_REAL";

    fn credential() -> ApiCredential {
        ApiCredential::from_bytes(SECRET).unwrap()
    }

    fn response(status: &str, extra_headers: &str, body: &[u8]) -> Vec<u8> {
        let mut bytes = format!(
            "HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: {}\r\n{extra_headers}\r\n",
            body.len()
        )
        .into_bytes();
        bytes.extend(body);
        bytes
    }

    fn json(body: &str) -> Vec<u8> {
        response(
            "200 OK",
            "Content-Type: application/json\r\n",
            body.as_bytes(),
        )
    }

    struct Reply {
        bytes: Vec<u8>,
        delay: Duration,
        body_delay: Duration,
    }

    fn serve(replies: Vec<Reply>) -> (SocketAddr, JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let worker = thread::spawn(move || {
            let mut requests = Vec::new();
            for reply in replies {
                let deadline = Instant::now() + Duration::from_secs(10);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "fixture accept timeout");
                            thread::sleep(Duration::from_millis(2));
                        }
                        Err(error) => panic!("fixture accept: {error}"),
                    }
                };
                // Accepted sockets may inherit a nonblocking listener's mode
                // on Windows. The fixture uses bounded blocking header reads.
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                requests.push(read_headers(&mut stream));
                thread::sleep(reply.delay);
                // Expected timeout/size-refusal tests may close the client early.
                if reply.body_delay.is_zero() {
                    let _ = stream.write_all(&reply.bytes);
                } else {
                    let split = reply
                        .bytes
                        .windows(4)
                        .position(|w| w == b"\r\n\r\n")
                        .unwrap()
                        + 4;
                    let _ = stream.write_all(&reply.bytes[..split]);
                    thread::sleep(reply.body_delay);
                    let _ = stream.write_all(&reply.bytes[split..]);
                }
            }
            requests
        });
        (address, worker)
    }

    fn read_headers(stream: &mut TcpStream) -> String {
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"\r\n\r\n") {
            assert!(bytes.len() < 16 * 1024);
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            bytes.push(byte[0]);
        }
        String::from_utf8(bytes).unwrap()
    }

    fn replies(bytes: Vec<Vec<u8>>) -> Vec<Reply> {
        bytes
            .into_iter()
            .map(|bytes| Reply {
                bytes,
                delay: Duration::ZERO,
                body_delay: Duration::ZERO,
            })
            .collect()
    }

    fn observe(
        bytes: Vec<Vec<u8>>,
    ) -> (
        Result<TransportObservation, SyncthingRestError>,
        Vec<String>,
    ) {
        let (endpoint, worker) = serve(replies(bytes));
        let client = SyncthingRest::for_owned_profile(endpoint, ID, credential()).unwrap();
        let result = client.observe();
        (result, worker.join().unwrap())
    }

    #[test]
    fn exact_pin_and_identity_pass_over_real_loopback_http() {
        let (result, requests) = observe(vec![
            json(r#"{"version":"v2.1.5","extra":true}"#),
            json(&format!(r#"{{"myID":"{ID}"}}"#)),
        ]);
        assert_eq!(
            result.unwrap(),
            TransportObservation {
                engine_version: "v2.1.5",
                expected_native_identity_matches: true
            }
        );
        assert_eq!(requests.len(), 2);
        assert!(requests[0].starts_with("GET /rest/system/version HTTP/1.1"));
        assert!(requests[1].starts_with("GET /rest/system/status HTTP/1.1"));
        for request in requests {
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("x-api-key: synthetic_test_api_key_not_real")
            );
        }
    }

    #[test]
    fn version_pin_mismatch_stops_before_second_request() {
        for version in ["v2.1.2", "v2.1.5-rc.1", "v2.1.5-dev", "2.1.5", "v2.1.50"] {
            let (result, requests) = observe(vec![json(&format!(r#"{{"version":"{version}"}}"#))]);
            assert_eq!(result, Err(SyncthingRestError::VersionMismatch));
            assert_eq!(requests.len(), 1);
        }
    }

    #[test]
    fn unexpected_native_identity_is_not_accepted() {
        let (result, _) = observe(vec![
            json(r#"{"version":"v2.1.5"}"#),
            json(r#"{"myID":"WRONG_NATIVE_ID"}"#),
        ]);
        assert_eq!(result, Err(SyncthingRestError::IdentityMismatch));
    }

    #[test]
    fn rejects_unauthorized_and_error_status_without_exposing_body() {
        for (status, expected) in [
            ("401 Unauthorized", SyncthingRestError::Unauthorized),
            ("403 Forbidden", SyncthingRestError::Unauthorized),
            (
                "500 Internal Server Error",
                SyncthingRestError::HttpStatus(500),
            ),
            ("204 No Content", SyncthingRestError::HttpStatus(204)),
        ] {
            let (result, _) = observe(vec![response(status, "", b"PRIVATE_ERROR_BODY")]);
            assert_eq!(result, Err(expected));
            assert!(!format!("{result:?}").contains("PRIVATE_ERROR_BODY"));
        }
    }

    #[test]
    fn redirect_is_refused_and_destination_receives_no_key() {
        let destination = TcpListener::bind("127.0.0.1:0").unwrap();
        destination.set_nonblocking(true).unwrap();
        let header = format!(
            "Location: http://{}/steal-key\r\n",
            destination.local_addr().unwrap()
        );
        let (result, _) = observe(vec![response("302 Found", &header, b"")]);
        assert_eq!(result, Err(SyncthingRestError::RedirectRefused));
        assert_eq!(
            destination.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn known_length_oversize_is_refused_before_body_allocation() {
        let reply = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            MAX_RESPONSE_BYTES + 1
        )
        .into_bytes();
        assert_eq!(
            observe(vec![reply]).0,
            Err(SyncthingRestError::ResponseTooLarge)
        );
    }

    #[test]
    fn chunked_and_unknown_length_oversize_are_stream_bounded() {
        let body = "X".repeat(MAX_RESPONSE_BYTES as usize + 1);
        let chunked = format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n{body}\r\n0\r\n\r\n", body.len()).into_bytes();
        let unbounded = format!("HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{body}").into_bytes();
        for reply in [chunked, unbounded] {
            assert_eq!(
                observe(vec![reply]).0,
                Err(SyncthingRestError::ResponseTooLarge)
            );
        }
    }

    #[test]
    fn malformed_duplicate_missing_and_non_utf8_responses_fail_closed() {
        for body in [
            b"not json".as_slice(),
            br#"{"version":"v2.1.5","version":"v2.1.2"}"#,
            br#"{"wrong":"v2.1.5"}"#,
            br#"{"version":215}"#,
            b"\xff",
        ] {
            assert_eq!(
                observe(vec![response("200 OK", "", body)]).0,
                Err(SyncthingRestError::InvalidResponse)
            );
        }
    }

    #[test]
    fn partial_response_is_not_valid_json_success() {
        let reply = b"HTTP/1.1 200 OK\r\nContent-Length: 50\r\nConnection: close\r\n\r\n{\"version\":\"v2.1.5\"}".to_vec();
        assert_eq!(
            observe(vec![reply]).0,
            Err(SyncthingRestError::InvalidResponse)
        );
    }

    #[test]
    fn total_budget_covers_body_reads_and_both_requests() {
        let (endpoint, worker) = serve(vec![
            Reply {
                bytes: json(r#"{"version":"v2.1.5"}"#),
                delay: Duration::from_millis(120),
                body_delay: Duration::ZERO,
            },
            Reply {
                bytes: json(&format!(r#"{{"myID":"{ID}"}}"#)),
                delay: Duration::from_millis(250),
                body_delay: Duration::ZERO,
            },
        ]);
        let client = SyncthingRest::for_owned_profile(endpoint, ID, credential()).unwrap();
        let started = Instant::now();
        assert_eq!(
            client.observe_with_budget(Duration::from_millis(250)),
            Err(SyncthingRestError::Deadline)
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        worker.join().unwrap();
    }

    #[test]
    fn body_after_immediate_headers_still_has_a_deadline() {
        let (endpoint, worker) = serve(vec![Reply {
            bytes: json(r#"{"version":"v2.1.5"}"#),
            delay: Duration::ZERO,
            body_delay: Duration::from_millis(500),
        }]);
        let client = SyncthingRest::for_owned_profile(endpoint, ID, credential()).unwrap();
        let started = Instant::now();
        assert_eq!(
            client.observe_with_budget(Duration::from_millis(100)),
            Err(SyncthingRestError::Deadline)
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        worker.join().unwrap();
    }

    #[test]
    fn exactly_the_response_limit_is_allowed_for_valid_json() {
        let mut version = r#"{"version":"v2.1.5"}"#.to_owned();
        version.push_str(&" ".repeat(MAX_RESPONSE_BYTES as usize - version.len()));
        assert_eq!(version.len(), MAX_RESPONSE_BYTES as usize);
        assert!(
            observe(vec![json(&version), json(&format!(r#"{{"myID":"{ID}"}}"#))])
                .0
                .is_ok()
        );
    }

    #[test]
    fn zero_budget_performs_no_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let client =
            SyncthingRest::for_owned_profile(listener.local_addr().unwrap(), ID, credential())
                .unwrap();
        assert_eq!(
            client.observe_with_budget(Duration::ZERO),
            Err(SyncthingRestError::Deadline)
        );
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn only_exact_numeric_loopback_and_nonzero_port_are_allowed() {
        for address in [
            "192.0.2.1:8384",
            "0.0.0.0:8384",
            "127.0.0.2:8384",
            "127.0.0.1:0",
            "[::]:8384",
            "[::ffff:127.0.0.1]:8384",
        ] {
            assert_eq!(
                SyncthingRest::for_owned_profile(address.parse().unwrap(), ID, credential())
                    .unwrap_err(),
                SyncthingRestError::InvalidEndpoint
            );
        }
        assert!(
            SyncthingRest::for_owned_profile("[::1]:8384".parse().unwrap(), ID, credential())
                .is_ok()
        );
    }

    #[test]
    fn invalid_expected_identity_and_credentials_fail_before_network() {
        for id in ["", "http://127.0.0.1/", "PRIVATE_ID", &"a".repeat(63)] {
            assert_eq!(
                SyncthingRest::for_owned_profile(
                    "127.0.0.1:8384".parse().unwrap(),
                    id,
                    credential()
                )
                .unwrap_err(),
                SyncthingRestError::InvalidExpectedIdentity
            );
        }
        for key in [
            vec![],
            b"key\r\nInjected: yes".to_vec(),
            b" space ".to_vec(),
            vec![b'X'; 257],
            vec![0xff],
        ] {
            assert_eq!(
                ApiCredential::from_bytes(&key).unwrap_err(),
                SyncthingRestError::InvalidCredential
            );
        }
    }

    #[test]
    fn secure_store_get_only_missing_and_invalid_items_never_replace() {
        struct Missing;
        impl SecureStore for Missing {
            fn put(&self, _: &str, _: &str, _: &[u8]) -> Result<(), SecureStoreError> {
                panic!("must not mint secret")
            }
            fn delete(&self, _: &str, _: &str) -> Result<(), SecureStoreError> {
                panic!("must not delete secret")
            }
            fn get(&self, _: &str, _: &str) -> Result<Vec<u8>, SecureStoreError> {
                Err(SecureStoreError::Missing)
            }
        }
        assert_eq!(
            ApiCredential::load_existing(&Missing, SecureStoreEnvironment::Dev, "profile-1")
                .unwrap_err(),
            SyncthingRestError::CredentialUnavailable
        );
        assert_eq!(
            ApiCredential::load_existing(&Missing, SecureStoreEnvironment::Dev, "../profile")
                .unwrap_err(),
            SyncthingRestError::InvalidCredential
        );
        let store = FakeSecureStore::new();
        store
            .put("Cryptex-Dev-ManagedSyncthing", "api:profile-1", SECRET)
            .unwrap();
        assert!(
            ApiCredential::load_existing(&store, SecureStoreEnvironment::Dev, "profile-1").is_ok()
        );
        assert_eq!(store.put_count(), 1);
        assert_eq!(
            ApiCredential::load_existing(&store, SecureStoreEnvironment::Prod, "profile-1")
                .unwrap_err(),
            SyncthingRestError::CredentialUnavailable
        );
    }

    #[test]
    fn backend_debug_and_errors_do_not_reveal_key_endpoint_or_identity() {
        let credential = credential();
        assert!(!format!("{credential:?}").contains(std::str::from_utf8(SECRET).unwrap()));
        let client =
            SyncthingRest::for_owned_profile("127.0.0.1:8384".parse().unwrap(), ID, credential)
                .unwrap();
        let debug = format!("{client:?}");
        for sensitive in ["127.0.0.1", ID, std::str::from_utf8(SECRET).unwrap()] {
            assert!(!debug.contains(sensitive));
        }
    }

    #[test]
    fn ambient_proxy_settings_cannot_redirect_the_api_key() {
        let (endpoint, worker) = serve(replies(vec![
            json(r#"{"version":"v2.1.5"}"#),
            json(&format!(r#"{{"myID":"{ID}"}}"#)),
        ]));
        let proxy = TcpListener::bind("127.0.0.1:0").unwrap();
        proxy.set_nonblocking(true).unwrap();
        let proxy_url = format!("http://{}", proxy.local_addr().unwrap());
        // Isolated child environment: never mutate the parallel test process's
        // proxy environment, Windows settings or any user-global proxy config.
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "syncthing_rest::tests::proxy_worker_entry",
                "--nocapture",
            ])
            .env("CRYPTEX_TEST_REST_ENDPOINT", endpoint.to_string())
            .env("HTTP_PROXY", &proxy_url)
            .env("HTTPS_PROXY", &proxy_url)
            .env("ALL_PROXY", &proxy_url)
            .env("http_proxy", &proxy_url)
            .env("https_proxy", &proxy_url)
            .env("all_proxy", &proxy_url)
            .env("NO_PROXY", "")
            .env("no_proxy", "")
            .output()
            .unwrap();
        assert!(output.status.success(), "isolated proxy worker failed");
        assert_eq!(worker.join().unwrap().len(), 2);
        assert_eq!(
            proxy.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn proxy_worker_entry() {
        let Ok(endpoint) = std::env::var("CRYPTEX_TEST_REST_ENDPOINT") else {
            return;
        };
        let client =
            SyncthingRest::for_owned_profile(endpoint.parse().unwrap(), ID, credential()).unwrap();
        assert_eq!(client.observe().unwrap().engine_version, PINNED_VERSION);
    }
}
