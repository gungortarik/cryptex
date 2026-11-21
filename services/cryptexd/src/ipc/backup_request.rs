//! Authenticated Desktop-only backup commands with request-bound replies.
use super::*;
use serde::de::DeserializeOwned;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame<A> {
    protocol_version: u32,
    request_id: String,
    client_role: String,
    capabilities: Vec<String>,
    operation: String,
    action: A,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Reply<O, E> {
    request_id: Option<String>,
    pub(super) result: Result<O, E>,
}

pub(super) fn handle<A: DeserializeOwned, O, E>(
    bytes: &[u8],
    desktop: bool,
    operation: &str,
    capability: &str,
    invalid: E,
    unauthorized: E,
    execute: impl FnOnce(A) -> Result<O, E>,
) -> Reply<O, E> {
    let Ok(frame) = serde_json::from_slice::<Frame<A>>(bytes) else {
        return Reply {
            request_id: None,
            result: Err(invalid),
        };
    };
    let result = if !desktop || frame.client_role != "desktop" || frame.capabilities != [capability]
    {
        Err(unauthorized)
    } else if frame.protocol_version != PROTOCOL_VERSION
        || frame.operation != operation
        || cryptex_core::validate::parse_uuid_str(&frame.request_id).is_err()
    {
        Err(invalid)
    } else {
        execute(frame.action)
    };
    Reply {
        request_id: Some(frame.request_id),
        result,
    }
}

pub(super) fn exchange<A: Serialize, O: DeserializeOwned, E: DeserializeOwned>(
    operation: &str,
    capability: &str,
    action: A,
    timeout: Duration,
) -> io::Result<Result<O, E>> {
    let request = Frame {
        protocol_version: PROTOCOL_VERSION,
        request_id: new_bootstrap_event_id().map_err(|_| invalid_data("request_id"))?,
        client_role: "desktop".into(),
        capabilities: vec![capability.into()],
        operation: operation.into(),
        action,
    };
    let mut stream = Stream::connect(socket_name()?)?;
    arm_ipc_stream(&stream)?;
    let deadline = Instant::now() + timeout;
    let mut frame = serde_json::to_vec(&request)?;
    frame.push(b'\n');
    write_ipc_all(&mut stream, &frame, deadline)?;
    let reply: Reply<O, E> =
        serde_json::from_slice(&read_ipc_frame(&mut stream, MAX_RESPONSE_BYTES, deadline)?)?;
    if reply.request_id.as_deref() != Some(&request.request_id) {
        return Err(invalid_data("backup_reply"));
    }
    Ok(reply.result)
}
