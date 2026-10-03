use std::{
    env,
    io::{self, BufRead, BufReader, Read, Write},
    process, thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[cfg(windows)]
use interprocess::local_socket::GenericNamespaced;
use interprocess::local_socket::{ListenerOptions, Stream, prelude::*};
use serde::{Deserialize, Serialize};

const PROTOCOL_VERSION: u32 = 1;
const MAX_FRAME_BYTES: usize = 64 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const RECONNECT_INTERVAL: Duration = Duration::from_millis(50);
const CAPABILITY_STATUS_READ: &str = "status.read";

#[derive(Debug, Deserialize, Serialize)]
struct Request {
    protocol_version: u32,
    request_id: String,
    client_role: String,
    capabilities: Vec<String>,
    operation: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Response {
    protocol_version: u32,
    request_id: Option<String>,
    accepted: bool,
    error: Option<String>,
    operation: Option<String>,
    daemon_pid: u32,
}

fn main() {
    if let Err(error) = run(env::args().skip(1).collect()) {
        eprintln!("m0-ipc-spike: {error}");
        process::exit(1);
    }
}

fn run(args: Vec<String>) -> io::Result<()> {
    match args.first().map(String::as_str) {
        Some("serve") => {
            let id = arg_value(&args, "--id")?;
            validate_id(id)?;
            let once = args.iter().any(|arg| arg == "--once");
            serve(id, once)
        }
        Some("client") => {
            let id = arg_value(&args, "--id")?;
            let role = arg_value(&args, "--role")?;
            let capability = args
                .iter()
                .position(|arg| arg == "--capability")
                .and_then(|index| args.get(index + 1))
                .map(String::as_str)
                .unwrap_or(CAPABILITY_STATUS_READ);
            let version = args
                .iter()
                .position(|arg| arg == "--version")
                .and_then(|index| args.get(index + 1))
                .map(|value| value.parse::<u32>())
                .transpose()
                .map_err(|_| invalid_input("--version must be an unsigned integer"))?
                .unwrap_or(PROTOCOL_VERSION);
            client(id, role, capability, version)
        }
        _ => Err(invalid_input(
            "usage: m0-ipc-spike serve --id <short-id> [--once] | client --id <short-id> --role <desktop|cli|mcp> [--capability status.read] [--version N]",
        )),
    }
}

fn arg_value<'a>(args: &'a [String], flag: &str) -> io::Result<&'a str> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .map(String::as_str)
        .ok_or_else(|| invalid_input(&format!("{flag} requires a value")))
}

fn validate_id(id: &str) -> io::Result<()> {
    if id.is_empty()
        || id.len() > 32
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(invalid_input(
            "IPC id must be 1-32 ASCII letters, digits, or hyphens",
        ));
    }
    Ok(())
}

fn serve(id: &str, once: bool) -> io::Result<()> {
    let listener = create_listener(id)?;
    println!("ready id={id} protocol_version={PROTOCOL_VERSION}");
    io::stdout().flush()?;
    let mut handled = 0usize;
    for connection in listener.incoming() {
        let mut stream = match connection {
            Ok(stream) => stream,
            Err(_) => continue,
        };
        handle_connection(&mut stream)?;
        handled += 1;
        if once && handled == 1 {
            break;
        }
    }
    Ok(())
}

fn create_listener(id: &str) -> io::Result<interprocess::local_socket::Listener> {
    let name = socket_name(id)?;
    let options = ListenerOptions::new().name(name).try_overwrite(true);

    #[cfg(windows)]
    {
        use interprocess::os::windows::{
            local_socket::ListenerOptionsExt, security_descriptor::SecurityDescriptor,
        };
        use widestring::U16CString;

        let sid = current_user_sid()?;
        let sddl = U16CString::from_str(&format!("O:{sid}D:P(A;;GA;;;{sid})"))
            .map_err(|_| invalid_input("invalid current-user pipe security descriptor"))?;
        let descriptor = SecurityDescriptor::deserialize(sddl.as_ucstr())?;
        options.security_descriptor(descriptor).create_sync()
    }

    #[cfg(unix)]
    {
        options.create_sync()
    }
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
        unsafe {
            GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut required);
        }
        if required == 0 {
            return Err(io::Error::last_os_error());
        }
        let word_size = mem::size_of::<usize>();
        let word_count = (required as usize).div_ceil(word_size);
        let mut storage = vec![0usize; word_count];
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
        .map_err(|_| invalid_input("current account SID is not valid UTF-16"));
        unsafe { LocalFree(sid_text.cast()) };
        sid
    })();

    unsafe { CloseHandle(token) };
    result
}

fn socket_name(id: &str) -> io::Result<interprocess::local_socket::Name<'static>> {
    #[cfg(windows)]
    {
        let name = std::ffi::OsString::from(format!("cryptex-m0-p0-3-{id}"));
        name.to_ns_name::<GenericNamespaced>()
    }

    #[cfg(unix)]
    {
        use interprocess::local_socket::GenericFilePath;
        use std::{
            fs,
            os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
        };

        // macOS sockaddr_un paths are short. A private directory under /tmp
        // keeps this proof endpoint short and inaccessible to other users.
        let parent = socket_parent(id);
        match fs::symlink_metadata(&parent) {
            Ok(metadata) => {
                if !metadata.file_type().is_dir() {
                    return Err(invalid_input(
                        "IPC runtime directory is not a real directory",
                    ));
                }
                if metadata.uid() != unsafe { libc::getuid() }
                    || metadata.permissions().mode() & 0o077 != 0
                {
                    return Err(invalid_input(
                        "IPC runtime directory is accessible to other users",
                    ));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let mut builder = fs::DirBuilder::new();
                builder.mode(0o700);
                builder.create(&parent)?;
            }
            Err(error) => return Err(error),
        }
        let path = parent.join("s");
        let path_string = path
            .to_str()
            .ok_or_else(|| invalid_input("socket path is not valid UTF-8"))?;
        if cfg!(target_os = "macos") && path_string.len() >= 104 {
            return Err(invalid_input(
                "macOS socket path exceeds sockaddr_un capacity",
            ));
        }
        path.to_fs_name::<GenericFilePath>()
    }
}

#[cfg(unix)]
fn socket_parent(id: &str) -> std::path::PathBuf {
    if cfg!(target_os = "macos") {
        std::path::PathBuf::from("/tmp").join(format!("cpx-{id}"))
    } else {
        env::temp_dir().join(format!("cpx-{id}"))
    }
}

fn handle_connection(stream: &mut Stream) -> io::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut frame = Vec::with_capacity(1024);
    let count = reader
        .by_ref()
        .take((MAX_FRAME_BYTES + 1) as u64)
        .read_until(b'\n', &mut frame)?;
    if count == 0 {
        return Ok(());
    }
    if count > MAX_FRAME_BYTES {
        return write_response(reader.get_mut(), rejected(None, "frame_too_large"));
    }
    if frame.last() != Some(&b'\n') {
        return write_response(reader.get_mut(), rejected(None, "malformed_frame"));
    }

    let response = match serde_json::from_slice::<Request>(&frame) {
        Ok(request) => authorize(request),
        Err(_) => rejected(None, "malformed_request"),
    };
    write_response(reader.get_mut(), response)
}

fn authorize(request: Request) -> Response {
    let request_id = request.request_id.clone();
    if request.protocol_version != PROTOCOL_VERSION {
        return rejected(Some(request_id), "incompatible_protocol_version");
    }
    if !matches!(request.client_role.as_str(), "desktop" | "cli" | "mcp") {
        return rejected(Some(request_id), "unauthorized_client_role");
    }
    if request.request_id.is_empty() || request.request_id.len() > 128 {
        return rejected(Some(request_id), "invalid_request_id");
    }
    if request.capabilities.len() != 1 || request.capabilities[0] != CAPABILITY_STATUS_READ {
        return rejected(Some(request_id), "unauthorized_capability");
    }
    if request.operation != "health.ping" {
        return rejected(Some(request_id), "unsupported_operation");
    }
    Response {
        protocol_version: PROTOCOL_VERSION,
        request_id: Some(request.request_id),
        accepted: true,
        error: None,
        operation: Some("health.pong".into()),
        daemon_pid: process::id(),
    }
}

fn rejected(request_id: Option<String>, error: &str) -> Response {
    Response {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        accepted: false,
        error: Some(error.into()),
        operation: None,
        daemon_pid: process::id(),
    }
}

fn write_response(stream: &mut Stream, response: Response) -> io::Result<()> {
    serde_json::to_writer(&mut *stream, &response)?;
    stream.write_all(b"\n")?;
    stream.flush()
}

fn client(id: &str, role: &str, capability: &str, version: u32) -> io::Result<()> {
    validate_id(id)?;
    let start = Instant::now();
    let name = socket_name(id)?;
    let mut stream = loop {
        match Stream::connect(name.clone()) {
            Ok(stream) => break stream,
            Err(error) if start.elapsed() < CONNECT_TIMEOUT => {
                let _ = error;
                thread::sleep(RECONNECT_INTERVAL);
            }
            Err(error) => return Err(error),
        }
    };
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let request = Request {
        protocol_version: version,
        request_id: format!("{}-{nonce}", process::id()),
        client_role: role.to_owned(),
        capabilities: vec![capability.to_owned()],
        operation: "health.ping".into(),
    };
    serde_json::to_writer(&mut stream, &request)?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    let mut response_line = Vec::with_capacity(256);
    let count = BufReader::new(stream)
        .take((MAX_FRAME_BYTES + 1) as u64)
        .read_until(b'\n', &mut response_line)?;
    if count == 0 || count > MAX_FRAME_BYTES || response_line.last() != Some(&b'\n') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid IPC response frame",
        ));
    }
    let response: Response = serde_json::from_slice(&response_line)?;
    println!("{}", serde_json::to_string(&response)?);
    if !response.accepted {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            response.error.unwrap_or_else(|| "request rejected".into()),
        ));
    }
    Ok(())
}

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(role: &str, capability: &str, version: u32) -> Request {
        Request {
            protocol_version: version,
            request_id: "test-1".into(),
            client_role: role.into(),
            capabilities: vec![capability.into()],
            operation: "health.ping".into(),
        }
    }

    #[test]
    fn all_required_roles_can_negotiate_health_read() {
        for role in ["desktop", "cli", "mcp"] {
            assert!(authorize(request(role, CAPABILITY_STATUS_READ, PROTOCOL_VERSION)).accepted);
        }
    }

    #[test]
    fn protocol_mismatch_and_unauthorized_identity_or_capability_are_rejected() {
        assert_eq!(
            authorize(request("cli", CAPABILITY_STATUS_READ, PROTOCOL_VERSION + 1)).error,
            Some("incompatible_protocol_version".into())
        );
        assert_eq!(
            authorize(request("unknown", CAPABILITY_STATUS_READ, PROTOCOL_VERSION)).error,
            Some("unauthorized_client_role".into())
        );
        assert_eq!(
            authorize(request("mcp", "memory.write", PROTOCOL_VERSION)).error,
            Some("unauthorized_capability".into())
        );
    }

    #[test]
    fn request_ids_and_operations_are_validated() {
        let mut malformed = request("desktop", CAPABILITY_STATUS_READ, PROTOCOL_VERSION);
        malformed.request_id = "x".repeat(129);
        assert_eq!(
            authorize(malformed).error,
            Some("invalid_request_id".into())
        );

        let mut unsupported = request("desktop", CAPABILITY_STATUS_READ, PROTOCOL_VERSION);
        unsupported.operation = "memory.purge".into();
        assert_eq!(
            authorize(unsupported).error,
            Some("unsupported_operation".into())
        );
    }

    #[test]
    fn endpoint_id_rejects_paths_and_oversized_names() {
        assert!(validate_id("desktop-01").is_ok());
        assert!(validate_id("../pipe").is_err());
        assert!(validate_id(&"x".repeat(33)).is_err());
    }

    #[test]
    fn local_transport_negotiates_current_user_connection() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let id = format!("test-{}-{nonce:x}", process::id());
        validate_id(&id).expect("generated ID is valid");

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let parent = socket_parent(&id);
            let metadata = std::fs::symlink_metadata(&parent).expect("private directory exists");
            assert!(metadata.file_type().is_dir());
            assert_eq!(metadata.uid(), unsafe { libc::getuid() });
            assert_eq!(metadata.mode() & 0o077, 0);
        }

        let listener = create_listener(&id).expect("listener creation succeeds");
        let server = thread::spawn(move || {
            let connection = listener
                .incoming()
                .next()
                .expect("listener accepts a connection")
                .expect("connection succeeds");
            let mut connection = connection;
            handle_connection(&mut connection).expect("request handled");
        });

        let mut connection = Stream::connect(socket_name(&id).expect("valid endpoint"))
            .expect("same-user client connects");
        let request = request("desktop", CAPABILITY_STATUS_READ, PROTOCOL_VERSION);
        serde_json::to_writer(&mut connection, &request).expect("request serializes");
        connection.write_all(b"\n").expect("frame terminates");
        connection.flush().expect("request flushes");
        let mut response_line = Vec::new();
        BufReader::new(connection)
            .take((MAX_FRAME_BYTES + 1) as u64)
            .read_until(b'\n', &mut response_line)
            .expect("response reads");
        let response: Response = serde_json::from_slice(&response_line).expect("response parses");
        assert!(response.accepted);
        assert_eq!(response.operation.as_deref(), Some("health.pong"));
        server.join().expect("server worker completes");

        #[cfg(unix)]
        std::fs::remove_dir(socket_parent(&id)).expect("test socket directory is empty");
    }
}
