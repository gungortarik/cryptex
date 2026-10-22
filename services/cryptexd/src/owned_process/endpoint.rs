//! Verify the server side of the actual connected socket, not a listening-port
//! snapshot followed by an unrelated connection. No application bytes are sent
//! until this boundary succeeds; HTTP must use this SAME stream exactly once.

use std::fmt;
#[cfg(windows)]
use std::net::IpAddr;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::time::Instant;

use super::OwnedProcess;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointError {
    InvalidEndpoint,
    UnsupportedPlatform,
    Unavailable,
    Deadline,
    OwnerExited,
    PeerNotOwned,
    TableUnavailable,
    TableTooLarge,
    InvalidTable,
}

impl fmt::Display for EndpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "owned_endpoint_{self:?}")
    }
}

impl std::error::Error for EndpointError {}

pub(crate) fn validate_loopback(endpoint: SocketAddr) -> Result<(), EndpointError> {
    let ordinary = match endpoint {
        SocketAddr::V4(address) => *address.ip() == Ipv4Addr::LOCALHOST,
        SocketAddr::V6(address) => {
            *address.ip() == Ipv6Addr::LOCALHOST
                && address.scope_id() == 0
                && address.flowinfo() == 0
        }
    };
    if endpoint.port() == 0 || !ordinary {
        return Err(EndpointError::InvalidEndpoint);
    }
    Ok(())
}

pub(crate) struct EndpointGuard<'a> {
    _owner: &'a OwnedProcess,
    #[cfg(windows)]
    _peer: super::windows::PeerHandle,
}

pub(crate) struct VerifiedConnection<'a> {
    stream: TcpStream,
    guard: EndpointGuard<'a>,
}

impl<'a> VerifiedConnection<'a> {
    pub(crate) fn into_parts(self) -> (TcpStream, EndpointGuard<'a>) {
        (self.stream, self.guard)
    }
}

impl OwnedProcess {
    pub(crate) fn connect_owned_loopback(
        &self,
        endpoint: SocketAddr,
        deadline: Instant,
    ) -> Result<VerifiedConnection<'_>, EndpointError> {
        validate_loopback(endpoint)?;
        #[cfg(windows)]
        {
            self.native.require_root_alive()?;
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .filter(|value| !value.is_zero())
                .ok_or(EndpointError::Deadline)?;
            let stream = TcpStream::connect_timeout(&endpoint, remaining).map_err(|error| {
                if error.kind() == std::io::ErrorKind::TimedOut {
                    EndpointError::Deadline
                } else {
                    EndpointError::Unavailable
                }
            })?;
            let client = stream
                .local_addr()
                .map_err(|_| EndpointError::Unavailable)?;
            let peer = stream.peer_addr().map_err(|_| EndpointError::Unavailable)?;
            if peer != endpoint
                || client.ip()
                    != match endpoint.ip() {
                        IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::LOCALHOST),
                        IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::LOCALHOST),
                    }
            {
                return Err(EndpointError::InvalidEndpoint);
            }
            let pid = super::windows::connected_server_pid(client, peer)?;
            // Hold a process handle and prove its membership in our retained
            // job, including the native monitor's child. No caller PID adoption.
            let peer = self.native.retain_owned_peer(pid)?;
            self.native.require_root_alive()?;
            if Instant::now() >= deadline {
                return Err(EndpointError::Deadline);
            }
            stream
                .set_nonblocking(true)
                .map_err(|_| EndpointError::Unavailable)?;
            Ok(VerifiedConnection {
                stream,
                guard: EndpointGuard {
                    _owner: self,
                    _peer: peer,
                },
            })
        }
        #[cfg(not(windows))]
        {
            let _ = deadline;
            Err(EndpointError::UnsupportedPlatform)
        }
    }
}
