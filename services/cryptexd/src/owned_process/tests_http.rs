use super::*;
use crate::owned_process::endpoint::EndpointError;
use crate::syncthing_owned_rest::{OwnedRestError, OwnedSyncthingRest};
use crate::syncthing_rest::ApiCredential;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};

const HTTP_ID: &str = "AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA-AAAAAAA";
const HTTP_KEY: &[u8] = b"SYNTHETIC_OWNED_SOCKET_API_KEY_0123456789";

#[derive(serde::Serialize, serde::Deserialize)]
struct HttpPlan {
    ipv6: bool,
    delay_ms: u64,
    responses: Vec<String>,
}

pub(super) fn http_worker(root: &Path) {
    let plan: HttpPlan =
        serde_json::from_slice(&fs::read(root.join("http-plan.json")).unwrap()).unwrap();
    let listener = TcpListener::bind(if plan.ipv6 { "[::1]:0" } else { "127.0.0.1:0" }).unwrap();
    listener.set_nonblocking(true).unwrap();
    publish(
        root,
        "server.addr",
        listener.local_addr().unwrap().to_string().as_bytes(),
    );
    for (index, response) in plan.responses.iter().enumerate() {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        std::process::exit(98);
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => std::process::exit(98),
            }
        };
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(4)))
            .unwrap();
        let mut request = Vec::new();
        let mut block = [0; 1024];
        while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            let n = match stream.read(&mut block) {
                Ok(n) => n,
                Err(_) => break,
            };
            if n == 0 {
                break;
            }
            request.extend_from_slice(&block[..n]);
            assert!(request.len() <= 16384);
        }
        fs::write(root.join(format!("http-request-{index}.txt")), request).unwrap();
        std::thread::sleep(Duration::from_millis(plan.delay_ms));
        let _ = stream.write_all(response.as_bytes());
    }
    wait_for_stop(root);
}

fn http_response(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}
fn start_http(
    fixture: &Fixture,
    responses: Vec<String>,
    ipv6: bool,
    tree: bool,
    delay_ms: u64,
) -> (OwnedProcess, SocketAddr) {
    fs::write(
        fixture.0.join("http-plan.json"),
        serde_json::to_vec(&HttpPlan {
            ipv6,
            delay_ms,
            responses,
        })
        .unwrap(),
    )
    .unwrap();
    let owned =
        OwnedProcess::spawn(&fixture.spec(if tree { "http-tree" } else { "http" })).unwrap();
    wait_file(&fixture.0.join("server.addr"));
    let address = fs::read_to_string(fixture.0.join("server.addr"))
        .unwrap()
        .parse()
        .unwrap();
    (owned, address)
}
fn owned_rest(owner: &OwnedProcess, address: SocketAddr) -> OwnedSyncthingRest<'_> {
    OwnedSyncthingRest::new(
        owner,
        address,
        HTTP_ID,
        ApiCredential::from_bytes(HTTP_KEY).unwrap(),
    )
    .unwrap()
}
fn stop_http(fixture: &Fixture, owned: &mut OwnedProcess, tree: bool) {
    fs::write(
        if tree {
            fixture.0.join("http-child/stop")
        } else {
            fixture.0.join("stop")
        },
        b"stop",
    )
    .unwrap();
    assert!(owned.wait_for_exit(TEST_TIMEOUT).unwrap().is_clean());
}

#[test]
fn owned_http_ipv4_ipv6_and_owned_monitor_child() {
    for (ipv6, tree) in [(false, false), (true, false), (false, true)] {
        let fixture = Fixture::new();
        let responses = vec![
            http_response(r#"{"version":"v2.1.5"}"#),
            http_response(&format!(r#"{{"myID":"{HTTP_ID}"}}"#)),
            http_response(r#"{"version":"v2.1.5"}"#),
            http_response(&format!(r#"{{"myID":"{HTTP_ID}"}}"#)),
            http_response("{}"),
        ];
        let (mut owner, address) = start_http(&fixture, responses, ipv6, tree, 0);
        let rest = owned_rest(&owner, address);
        assert!(rest.observe().unwrap().expected_native_identity_matches);
        rest.request_shutdown().unwrap();
        drop(rest);
        let requests = if tree {
            fixture.0.join("http-child")
        } else {
            fixture.0.clone()
        };
        for (index, path) in [
            "GET /rest/system/version",
            "GET /rest/system/status",
            "GET /rest/system/version",
            "GET /rest/system/status",
            "POST /rest/system/shutdown",
        ]
        .iter()
        .enumerate()
        {
            let request =
                fs::read_to_string(requests.join(format!("http-request-{index}.txt"))).unwrap();
            assert!(request.starts_with(path));
            assert!(request.contains(std::str::from_utf8(HTTP_KEY).unwrap()));
        }
        stop_http(&fixture, &mut owner, tree);
    }
}

#[test]
fn owned_http_unowned_server_receives_zero_application_bytes() {
    let fixture = Fixture::new();
    let mut owner = OwnedProcess::spawn(&fixture.spec("hold")).unwrap();
    wait_file(&fixture.0.join("ready"));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    assert_eq!(
        owned_rest(&owner, listener.local_addr().unwrap())
            .observe()
            .unwrap_err(),
        OwnedRestError::Endpoint(EndpointError::PeerNotOwned)
    );
    let (mut stream, _) = listener.accept().unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).unwrap();
    assert!(bytes.is_empty());
    fs::write(fixture.0.join("stop"), b"stop").unwrap();
    assert!(owner.wait_for_exit(TEST_TIMEOUT).unwrap().is_clean());
}

#[test]
fn owned_http_exited_owner_never_connects() {
    let fixture = Fixture::new();
    let mut owner = OwnedProcess::spawn(&fixture.spec("exit-0")).unwrap();
    assert!(owner.wait_for_exit(TEST_TIMEOUT).unwrap().is_clean());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    assert_eq!(
        owned_rest(&owner, listener.local_addr().unwrap())
            .observe()
            .unwrap_err(),
        OwnedRestError::Endpoint(EndpointError::OwnerExited)
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn owned_http_rejects_redirect_auth_version_duplicate_oversize_truncation_encoding() {
    let cases = [
        (
            "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/\r\nContent-Length: 0\r\n\r\n"
                .to_owned(),
            OwnedRestError::RedirectRefused,
        ),
        (
            "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n".to_owned(),
            OwnedRestError::Unauthorized(403),
        ),
        (
            http_response(r#"{"version":"v0.0.0"}"#),
            OwnedRestError::VersionMismatch,
        ),
        (
            http_response(r#"{"version":"v2.1.5","version":"v2.1.5"}"#),
            OwnedRestError::InvalidResponse,
        ),
        (
            "HTTP/1.1 200 OK\r\nContent-Length: 65537\r\n\r\n".to_owned(),
            OwnedRestError::ResponseTooLarge,
        ),
        (
            "HTTP/1.1 200 OK\r\nContent-Length: 20\r\n\r\n{}".to_owned(),
            OwnedRestError::InvalidResponse,
        ),
        (
            "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: 0\r\n\r\n".to_owned(),
            OwnedRestError::InvalidResponse,
        ),
        (
            format!(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n10001\r\n{}\r\n0\r\n\r\n",
                "x".repeat(65537)
            ),
            OwnedRestError::ResponseTooLarge,
        ),
    ];
    for (response, expected) in cases {
        let fixture = Fixture::new();
        let (mut owner, address) = start_http(&fixture, vec![response], false, false, 0);
        assert_eq!(owned_rest(&owner, address).observe().unwrap_err(), expected);
        stop_http(&fixture, &mut owner, false);
    }
}

#[test]
fn owned_http_identity_mismatch_and_total_deadline() {
    let fixture = Fixture::new();
    let (mut owner, address) = start_http(
        &fixture,
        vec![
            http_response(r#"{"version":"v2.1.5"}"#),
            http_response(r#"{"myID":"wrong"}"#),
        ],
        false,
        false,
        0,
    );
    assert_eq!(
        owned_rest(&owner, address).observe().unwrap_err(),
        OwnedRestError::IdentityMismatch
    );
    stop_http(&fixture, &mut owner, false);
    let fixture = Fixture::new();
    let (mut owner, address) = start_http(
        &fixture,
        vec![http_response(r#"{"version":"v2.1.5"}"#)],
        false,
        false,
        300,
    );
    assert_eq!(
        owned_rest(&owner, address)
            .observe_with_budget(Duration::from_millis(60))
            .unwrap_err(),
        OwnedRestError::Deadline
    );
    stop_http(&fixture, &mut owner, false);
}

#[test]
fn owned_http_shutdown_refuses_wrong_identity_before_post() {
    let fixture = Fixture::new();
    let (mut owner, address) = start_http(
        &fixture,
        vec![
            http_response(r#"{"version":"v2.1.5"}"#),
            http_response(r#"{"myID":"wrong"}"#),
        ],
        false,
        false,
        0,
    );
    assert_eq!(
        owned_rest(&owner, address).request_shutdown().unwrap_err(),
        OwnedRestError::IdentityMismatch
    );
    assert!(!fixture.0.join("http-request-2.txt").exists());
    stop_http(&fixture, &mut owner, false);
}

#[test]
fn owned_http_rejects_remote_zero_scoped_or_noncanonical_loopback() {
    let fixture = Fixture::new();
    let mut owner = OwnedProcess::spawn(&fixture.spec("hold")).unwrap();
    wait_file(&fixture.0.join("ready"));
    for endpoint in [
        "127.0.0.1:0",
        "127.0.0.2:8384",
        "192.0.2.1:8384",
        "[::ffff:127.0.0.1]:8384",
        "[::1%3]:8384",
    ] {
        assert_eq!(
            OwnedSyncthingRest::new(
                &owner,
                endpoint.parse().unwrap(),
                HTTP_ID,
                ApiCredential::from_bytes(HTTP_KEY).unwrap()
            )
            .unwrap_err(),
            OwnedRestError::Endpoint(EndpointError::InvalidEndpoint)
        );
    }
    fs::write(fixture.0.join("stop"), b"stop").unwrap();
    assert!(owner.wait_for_exit(TEST_TIMEOUT).unwrap().is_clean());
}

#[test]
fn owned_http_dead_root_with_live_job_child_is_not_ready() {
    let fixture = Fixture::new();
    fs::write(
        fixture.0.join("http-plan.json"),
        serde_json::to_vec(&HttpPlan {
            ipv6: false,
            delay_ms: 0,
            responses: Vec::new(),
        })
        .unwrap(),
    )
    .unwrap();
    let mut owner = OwnedProcess::spawn(&fixture.spec("http-root-exit")).unwrap();
    wait_file(&fixture.0.join("server.addr"));
    wait_stopped(owner.pid());
    assert!(
        owner.try_exit().unwrap().is_none(),
        "child must still be alive"
    );
    let address = fs::read_to_string(fixture.0.join("server.addr"))
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(
        owned_rest(&owner, address).observe().unwrap_err(),
        OwnedRestError::Endpoint(EndpointError::OwnerExited)
    );
    assert!(!fixture.0.join("http-child/http-request-0.txt").exists());
    owner.force_cleanup(TEST_TIMEOUT).unwrap();
    assert!(owner.wait_for_exit(TEST_TIMEOUT).unwrap().forced);
}
