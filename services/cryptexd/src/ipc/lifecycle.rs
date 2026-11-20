//! Authenticated Desktop-only reversible actions. Only a registered test daemon can write.
use super::*;

pub(super) const OPERATION: &str = "note.lifecycle";
const CAPABILITY: &str = "note.lifecycle.owner";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    note_id: String,
    note_hash: String,
    state_hash: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Inspect { note_id: Option<String> },
    List { after: Option<String> },
    Forget { target: Selection },
    Restore { target: Selection },
}

#[derive(Serialize, Deserialize)]
pub struct Choice {
    target: Selection,
    preview: String,
    created_at: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outcome {
    Ready {
        target: Option<Selection>,
    },
    Forgotten {
        choices: Vec<Choice>,
        next: Option<String>,
    },
    Changed,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    Disabled,
    Unauthorized,
    InvalidRequest,
    StaleSelection,
    VerificationFailed,
    Unavailable,
    Uncertain,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    protocol_version: u32,
    request_id: String,
    client_role: String,
    operation: String,
    capabilities: Vec<String>,
    action: Action,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Reply {
    request_id: Option<String>,
    result: Result<Outcome, Error>,
}

pub(super) fn handle(bytes: &[u8], ctx: Option<&SearchContext>, desktop: bool) -> Reply {
    let Ok(frame) = serde_json::from_slice::<Frame>(bytes) else {
        return Reply {
            request_id: None,
            result: Err(Error::InvalidRequest),
        };
    };
    let result = if !desktop || frame.client_role != "desktop" || frame.capabilities != [CAPABILITY]
    {
        Err(Error::Unauthorized)
    } else if frame.operation != OPERATION
        || frame.protocol_version != PROTOCOL_VERSION
        || cryptex_core::validate::parse_uuid_str(&frame.request_id).is_err()
    {
        Err(Error::InvalidRequest)
    } else {
        execute(ctx, frame.action)
    };
    Reply {
        request_id: Some(frame.request_id),
        result,
    }
}

#[cfg(not(all(
    test,
    any(windows, all(target_os = "macos", feature = "m8-local-startup-check"))
)))]
fn execute(_: Option<&SearchContext>, _: Action) -> Result<Outcome, Error> {
    Err(Error::Disabled)
}

#[cfg(all(
    test,
    any(windows, all(target_os = "macos", feature = "m8-local-startup-check"))
))]
mod trial;
#[cfg(all(
    test,
    any(windows, all(target_os = "macos", feature = "m8-local-startup-check"))
))]
use trial::execute;
#[cfg(all(
    test,
    any(windows, all(target_os = "macos", feature = "m8-local-startup-check"))
))]
pub(crate) use trial::register;

pub fn desktop(action: Action) -> Result<Outcome, Error> {
    let request = Frame {
        protocol_version: PROTOCOL_VERSION,
        request_id: new_bootstrap_event_id().map_err(|_| Error::Unavailable)?,
        client_role: "desktop".into(),
        operation: OPERATION.into(),
        capabilities: vec![CAPABILITY.into()],
        action,
    };
    let mut stream = Stream::connect(socket_name().map_err(|_| Error::Unavailable)?)
        .map_err(|_| Error::Unavailable)?;
    let bytes = exchange_until(
        &mut stream,
        &request,
        MAX_RESPONSE_BYTES,
        Instant::now() + IPC_RESPONSE_DEADLINE,
    )
    .map_err(|_| Error::Uncertain)?;
    let reply: Reply = serde_json::from_slice(&bytes).map_err(|_| Error::Uncertain)?;
    if reply.request_id.as_deref() != Some(&request.request_id) {
        return Err(Error::Uncertain);
    }
    reply.result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forgotten_picker_and_writes_require_the_authenticated_owner_desktop() {
        for action in [
            serde_json::json!({"kind":"list"}),
            serde_json::json!({"kind":"restore","target":{"note_id":"id","note_hash":"hash","state_hash":"hash"}}),
        ] {
            let mut frame = serde_json::json!({"protocol_version":PROTOCOL_VERSION,"request_id":"7a8ff688-ad0d-458f-9834-8bcc21aa9cb8",
                "client_role":"desktop","operation":OPERATION,"capabilities":[CAPABILITY],"action":action});
            let call = |frame: &serde_json::Value, desktop| {
                handle(&serde_json::to_vec(frame).unwrap(), None, desktop).result
            };
            assert!(matches!(call(&frame, false), Err(Error::Unauthorized)));
            assert!(matches!(call(&frame, true), Err(Error::Disabled)));
            frame["client_role"] = "mcp".into();
            assert!(matches!(call(&frame, true), Err(Error::Unauthorized)));
            frame["client_role"] = "desktop".into();
            frame["capabilities"] = serde_json::json!(["content.search"]);
            assert!(matches!(call(&frame, true), Err(Error::Unauthorized)));
        }
    }
}
