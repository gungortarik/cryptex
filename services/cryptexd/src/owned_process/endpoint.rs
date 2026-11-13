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
    /// The exact socket tuple exists, but the OS does not attest ESTABLISHED.
    PeerNotEstablished,
    PeerNotOwned,
    /// A discovered endpoint changed during task retention; never retry it as
    /// an ordinary accept() installation delay on the same request.
    PeerChanged,
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
    #[cfg(all(target_os = "macos", feature = "m8-local-startup-check"))]
    _peer: super::macos::PeerPin,
}

impl EndpointGuard<'_> {
    pub(crate) fn before_send(&self) -> Result<(), EndpointError> {
        #[cfg(all(target_os = "macos", feature = "m8-local-startup-check"))]
        {
            self._owner.native.validate_peer(&self._peer, true)
        }
        #[cfg(not(all(target_os = "macos", feature = "m8-local-startup-check")))]
        {
            Ok(())
        }
    }

    pub(crate) fn observation_complete(&self) -> Result<(), EndpointError> {
        #[cfg(all(target_os = "macos", feature = "m8-local-startup-check"))]
        {
            self._owner.native.validate_peer(&self._peer, false)
        }
        #[cfg(not(all(target_os = "macos", feature = "m8-local-startup-check")))]
        {
            Ok(())
        }
    }
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
        #[cfg(all(target_os = "macos", feature = "m8-local-startup-check"))]
        {
            self.native.require_root_alive()?;
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .filter(|d| !d.is_zero())
                .ok_or(EndpointError::Deadline)?;
            let stream = TcpStream::connect_timeout(&endpoint, remaining)
                .map_err(|_| EndpointError::Unavailable)?;
            if stream.peer_addr().map_err(|_| EndpointError::Unavailable)? != endpoint {
                return Err(EndpointError::InvalidEndpoint);
            }
            // Darwin exposes the serving socket only after accept() installs
            // its FD. Reobserve this SAME untouched connection briefly; never
            // reconnect, send credentials, or accept a listener-only row.
            let accept_until = deadline.min(Instant::now() + std::time::Duration::from_millis(250));
            let peer = loop {
                match self.native.retain_socket_peer(&stream) {
                    Ok(peer) => break peer,
                    Err(EndpointError::PeerNotOwned) if Instant::now() < accept_until => {
                        std::thread::sleep(std::time::Duration::from_millis(2));
                    }
                    Err(error) => return Err(error),
                }
            };
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
        #[cfg(not(any(windows, all(target_os = "macos", feature = "m8-local-startup-check"))))]
        {
            let _ = deadline;
            Err(EndpointError::UnsupportedPlatform)
        }
    }
}
