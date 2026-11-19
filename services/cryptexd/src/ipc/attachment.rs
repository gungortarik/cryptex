use super::*;

// Fits the existing authenticated desktop frame even with decimal JSON bytes.
pub const MAX_BYTES: usize = 512 * 1024;
const OPERATION: &str = "attachment.add";
const CAPABILITY: &str = "content.attach";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectedFile {
    pub note_id: String,
    pub note_hash: String,
    pub bytes: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AttachmentRequest {
    protocol_version: u32,
    request_id: String,
    client_role: String,
    capabilities: Vec<String>,
    operation: String,
    body: SelectedFile,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachError {
    Disabled,
    Unauthorized,
    InvalidRequest,
    TooLarge,
    TargetUnavailable,
    Busy,
    VerificationFailed,
    Storage,
    Uncertain,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Reply {
    request_id: Option<String>,
    result: Result<(), AttachError>,
}

pub(super) fn enabled(context: &SearchContext) -> bool {
    #[cfg(test)]
    {
        context.isolated_attachment_writer
            && cfg!(any(
                windows,
                all(target_os = "macos", feature = "m8-local-startup-check")
            ))
    }
    #[cfg(not(test))]
    {
        let _ = context;
        false
    }
}

pub(super) fn handle(
    frame: &[u8],
    context: Option<&SearchContext>,
    desktop: bool,
    workers: Option<&Arc<AtomicUsize>>,
) -> Reply {
    let request: AttachmentRequest = match serde_json::from_slice(frame) {
        Ok(req) => req,
        Err(_) => {
            return Reply {
                request_id: None,
                result: Err(AttachError::InvalidRequest),
            };
        }
    };
    let result = (|| {
        if !desktop
            || request.client_role != "desktop"
            || request.capabilities.as_slice() != [CAPABILITY]
        {
            return Err(AttachError::Unauthorized);
        }
        if request.operation != OPERATION
            || request.protocol_version != PROTOCOL_VERSION
            || cryptex_core::validate::parse_uuid_str(&request.request_id).is_err()
        {
            return Err(AttachError::InvalidRequest);
        }
        let ctx = context
            .filter(|ctx| enabled(ctx))
            .ok_or(AttachError::Disabled)?;
        let _slot = match workers {
            Some(pool) => Some(
                try_reserve_ipc_worker(pool, MAX_DESKTOP_CAPTURE_WORKERS)
                    .ok_or(AttachError::Busy)?,
            ),
            None => None,
        };
        let ctx = ctx.admitted().map_err(|_| AttachError::Busy)?;
        if request.body.bytes.len() > MAX_BYTES {
            return Err(AttachError::TooLarge);
        }
        if cryptex_core::validate::parse_uuid_str(&request.body.note_id).is_err()
            || cryptex_events::ContentHashHex::parse(&request.body.note_hash).is_err()
        {
            return Err(AttachError::InvalidRequest);
        }
        add(&ctx, &request.body)?;
        request_projection_sync(&ctx);
        ctx.admission
            .check_namespace()
            .map_err(|_| AttachError::Uncertain)
    })();
    Reply {
        request_id: Some(request.request_id),
        result,
    }
}

fn add(ctx: &SearchContext, file: &SelectedFile) -> Result<(), AttachError> {
    #[cfg(all(
        test,
        any(windows, all(target_os = "macos", feature = "m8-local-startup-check"))
    ))]
    {
        crate::canonical_writer::isolated_attachment::append(
            &ctx.runtime_root,
            ctx.store.as_ref(),
            ctx.environment,
            file,
        )
    }
    #[cfg(not(all(
        test,
        any(windows, all(target_os = "macos", feature = "m8-local-startup-check"))
    )))]
    {
        let _ = (ctx, file);
        Err(AttachError::Disabled)
    }
}

pub fn desktop_attach(file: SelectedFile) -> Result<(), AttachError> {
    if file.bytes.len() > MAX_BYTES {
        return Err(AttachError::TooLarge);
    }
    let name = socket_name().map_err(|_| AttachError::Busy)?;
    let mut stream = Stream::connect(name).map_err(|_| AttachError::Busy)?;
    arm_ipc_stream(&stream).map_err(|_| AttachError::Busy)?;
    let request = AttachmentRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: new_bootstrap_event_id().map_err(|_| AttachError::Busy)?,
        client_role: "desktop".into(),
        capabilities: vec![CAPABILITY.into()],
        operation: OPERATION.into(),
        body: file,
    };
    let mut frame = serde_json::to_vec(&request).map_err(|_| AttachError::InvalidRequest)?;
    frame.push(b'\n');
    let deadline = Instant::now() + IPC_CAPTURE_RESPONSE_DEADLINE;
    write_ipc_all(&mut stream, &frame, deadline).map_err(|_| AttachError::Uncertain)?;
    let frame = read_ipc_frame(&mut stream, MAX_RESPONSE_BYTES, deadline)
        .map_err(|_| AttachError::Uncertain)?;
    let reply: Reply = serde_json::from_slice(&frame).map_err(|_| AttachError::Uncertain)?;
    if reply.request_id.as_deref() != Some(&request.request_id) {
        return Err(AttachError::Uncertain);
    }
    reply.result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_bytes_need_the_desktop_process_role_and_attachment_capability() {
        let request = serde_json::json!({
            "protocol_version": PROTOCOL_VERSION, "request_id": "ac123928-a245-4251-a141-cda32a57cdd8",
            "client_role": "desktop", "capabilities": [CAPABILITY], "operation": OPERATION,
            "body": { "noteId": "b83040d5-7529-4c75-a289-e9399ea8a0ba", "noteHash": "a".repeat(64), "bytes": [0,255] }
        });
        let call = |value: &serde_json::Value, desktop| {
            handle(&serde_json::to_vec(value).unwrap(), None, desktop, None).result
        };
        assert_eq!(call(&request, true), Err(AttachError::Disabled));
        assert_eq!(call(&request, false), Err(AttachError::Unauthorized));
        for (field, wrong) in [
            ("client_role", serde_json::json!("cli")),
            ("capabilities", serde_json::json!(["content.search"])),
        ] {
            let mut other = request.clone();
            other[field] = wrong;
            assert_eq!(call(&other, true), Err(AttachError::Unauthorized));
        }
        let mut extra = request;
        extra["body"]["path"] = serde_json::json!("file.bin");
        assert_eq!(call(&extra, true), Err(AttachError::InvalidRequest));
    }
}
