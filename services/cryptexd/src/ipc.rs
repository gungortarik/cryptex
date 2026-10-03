//! Narrow M0 local IPC transport. The only operation exposed here is
//! read-only health ping; application memory APIs are deliberately absent.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::process;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::compat::DAEMON_PROTOCOL_VERSION;
#[cfg(windows)]
use interprocess::local_socket::GenericNamespaced;
use interprocess::local_socket::{ListenerOptions, Stream, prelude::*};
use serde::{Deserialize, Serialize};

const PROTOCOL_VERSION: u32 = DAEMON_PROTOCOL_VERSION;
const MAX_FRAME_BYTES: usize = 64 * 1024;
const CAPABILITY_STATUS_READ: &str = "status.read";
const ENDPOINT_ID: &str = "cryptexd-v1";

#[derive(Debug, Serialize, Deserialize)]
struct Request {
    protocol_version: u32,
    request_id: String,
    client_role: String,
    capabilities: Vec<String>,
    operation: String,
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
    let listener = create_listener()?;
    thread::Builder::new()
        .name("cryptex-ipc".into())
        .spawn(move || {
            for incoming in listener.incoming() {
                match incoming {
                    Ok(mut stream) => {
                        if let Err(error) = handle_connection(&mut stream) {
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
    };
    serde_json::to_writer(&mut stream, &request)?;
    stream.write_all(b"\n")?;
    stream.flush()?;

    let mut frame = Vec::with_capacity(256);
    let count = BufReader::new(stream)
        .take((MAX_FRAME_BYTES + 1) as u64)
        .read_until(b'\n', &mut frame)?;
    if count == 0 || count > MAX_FRAME_BYTES || frame.last() != Some(&b'\n') {
        return Err(invalid_data("invalid IPC response frame"));
    }
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

fn handle_connection(stream: &mut Stream) -> io::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut frame = Vec::with_capacity(1024);
    let count = reader
        .by_ref()
        .take((MAX_FRAME_BYTES + 1) as u64)
        .read_until(b'\n', &mut frame)?;
    let response = if count == 0 {
        return Ok(());
    } else if count > MAX_FRAME_BYTES {
        rejected(None, "frame_too_large")
    } else if frame.last() != Some(&b'\n') {
        rejected(None, "malformed_frame")
    } else {
        match serde_json::from_slice::<Request>(&frame) {
            Ok(request) => authorize(request),
            Err(_) => rejected(None, "malformed_request"),
        }
    };
    serde_json::to_writer(&mut *reader.get_mut(), &response)?;
    reader.get_mut().write_all(b"\n")?;
    reader.get_mut().flush()
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
    } else if request.operation != "health.ping" {
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
