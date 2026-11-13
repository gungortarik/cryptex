use super::*;
use std::cell::Cell;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

struct Check {
    before: Option<EndpointError>,
    after: Option<EndpointError>,
    sent: Arc<AtomicBool>,
    calls: Cell<(usize, usize)>,
}
impl ConnectionCheck for Check {
    fn before_send(&self) -> Result<(), EndpointError> {
        self.calls.set((self.calls.get().0 + 1, self.calls.get().1));
        self.before.map_or(Ok(()), Err)
    }
    fn observation_complete(&self) -> Result<(), EndpointError> {
        assert!(self.sent.load(Ordering::SeqCst));
        self.calls.set((self.calls.get().0, self.calls.get().1 + 1));
        self.after.map_or(Ok(()), Err)
    }
}

fn exchange(
    check: &Check,
    operation: Operation,
    budget: Duration,
) -> (Result<Vec<u8>, OwnedRestError>, Vec<u8>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap();
    let stream = TcpStream::connect(endpoint).unwrap();
    let (mut server, _) = listener.accept().unwrap();
    stream.set_nonblocking(true).unwrap();
    server
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let sent = check.sent.clone();
    let worker = std::thread::spawn(move || {
        let mut request = Vec::new();
        let mut block = [0; 512];
        while !request.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = server.read(&mut block).unwrap();
            if n == 0 {
                break;
            }
            request.extend_from_slice(&block[..n]);
            assert!(request.len() <= 16384);
        }
        if !request.is_empty() {
            sent.store(true, Ordering::SeqCst);
            server
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
                .unwrap();
        }
        request
    });
    let result = request_on_connection(
        stream,
        check,
        operation,
        endpoint,
        &ApiCredential::from_bytes(b"SYNTHETIC_BOUNDARY_KEY").unwrap(),
        Instant::now() + budget,
    );
    (result, worker.join().unwrap())
}

fn check(before: Option<EndpointError>, after: Option<EndpointError>) -> Check {
    Check {
        before,
        after,
        sent: Arc::new(AtomicBool::new(false)),
        calls: Cell::new((0, 0)),
    }
}

#[test]
fn stale_or_changed_peer_refuses_before_any_http_bytes() {
    for error in [
        EndpointError::OwnerExited,
        EndpointError::PeerNotOwned,
        EndpointError::PeerChanged,
        EndpointError::TableUnavailable,
    ] {
        let check = check(Some(error), None);
        let (result, request) = exchange(&check, Operation::Version, BUDGET);
        assert_eq!(result, Err(OwnedRestError::Endpoint(error)));
        assert!(request.is_empty());
        assert_eq!(check.calls.get(), (1, 0));
    }
}

#[test]
fn queued_valid_body_cannot_authorize_a_lost_generation() {
    let check = check(None, Some(EndpointError::OwnerExited));
    let (result, request) = exchange(&check, Operation::Status, BUDGET);
    assert_eq!(result, Err(OwnedRestError::Endpoint(EndpointError::OwnerExited)));
    assert!(!request.is_empty());
    assert_eq!(check.calls.get(), (1, 1));
}

#[test]
fn ordinary_http_close_preserves_live_observation() {
    let check = check(None, None);
    let (result, request) = exchange(&check, Operation::Version, BUDGET);
    assert_eq!(result, Ok(b"{}".to_vec()));
    assert!(
        request
            .windows(b"SYNTHETIC_BOUNDARY_KEY".len())
            .any(|w| w == b"SYNTHETIC_BOUNDARY_KEY")
    );
    assert_eq!(check.calls.get(), (1, 1));
}

#[test]
fn shutdown_ack_does_not_claim_live_identity_after_exit() {
    let check = check(None, Some(EndpointError::OwnerExited));
    let (result, request) = exchange(&check, Operation::Shutdown, BUDGET);
    assert_eq!(result, Ok(b"{}".to_vec()));
    assert!(request.starts_with(b"POST /rest/system/shutdown "));
    assert_eq!(check.calls.get(), (1, 0));
}

#[test]
fn synchronous_validation_never_resets_the_request_deadline() {
    let check = check(None, None);
    // Invoke the same pre-send boundary directly with a known expired budget.
    // No thread scheduling or runtime setup can bypass the intended callback.
    let expired = Instant::now() - Duration::from_secs(1);
    assert_eq!(check_before_send(&check, expired), Err(OwnedRestError::Deadline));
    assert_eq!(check.calls.get(), (1, 0));
}
