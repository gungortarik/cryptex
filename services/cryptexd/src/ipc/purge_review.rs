//! Review of owner intent. Only the isolated daemon can persist a confirmation.
#[cfg(all(
    test,
    any(windows, all(target_os = "macos", feature = "m8-local-startup-check"))
))]
pub(crate) mod physical;
#[cfg(all(
    test,
    any(windows, all(target_os = "macos", feature = "m8-local-startup-check"))
))]
pub(crate) mod receipt;
use super::*;

pub(super) const OPERATION: &str = "purge.review";
const CAPABILITY: &str = "purge.review.owner";

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Prepare { note_id: String, note_hash: String },
    Cancel { ticket: String },
    Confirm { ticket: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    Prepared {
        ticket: String,
        remove_locally: bool,
    },
    Cancelled,
    Purged {
        note_id: String,
        note_hash: String,
    },
    Confirmed {
        note_id: String,
        note_hash: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewError {
    Disabled,
    Unauthorized,
    InvalidRequest,
    ReviewExpired,
    TargetUnavailable,
    Unavailable,
    ReceiptIntegrity,
    ReceiptStorage,
    PackIntegrity,
    PackStorage,
    CleanupPending,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    protocol_version: u32,
    request_id: String,
    client_role: String,
    capabilities: Vec<String>,
    operation: String,
    action: Action,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Reply {
    request_id: Option<String>,
    result: Result<Outcome, ReviewError>,
}

#[cfg(test)]
pub(super) struct Pending {
    ticket: String,
    note_id: String,
    note_hash: String,
    expires: Instant,
}

pub(super) fn enabled(ctx: &SearchContext) -> bool {
    #[cfg(test)]
    {
        cfg!(any(
            windows,
            all(target_os = "macos", feature = "m8-local-startup-check")
        )) && ctx.purge_reviews.is_some()
    }
    #[cfg(not(test))]
    {
        let _ = ctx;
        false
    }
}

pub(super) fn handle(bytes: &[u8], ctx: Option<&SearchContext>, desktop: bool) -> Reply {
    let Ok(frame) = serde_json::from_slice::<Frame>(bytes) else {
        return Reply {
            request_id: None,
            result: Err(ReviewError::InvalidRequest),
        };
    };
    let result = if !desktop || frame.client_role != "desktop" || frame.capabilities != [CAPABILITY]
    {
        Err(ReviewError::Unauthorized)
    } else if frame.protocol_version != PROTOCOL_VERSION
        || frame.operation != OPERATION
        || cryptex_core::validate::parse_uuid_str(&frame.request_id).is_err()
    {
        Err(ReviewError::InvalidRequest)
    } else {
        match ctx.filter(|context| enabled(context)) {
            #[cfg(test)]
            Some(context) => decide(context, frame.action),
            _ => Err(ReviewError::Disabled),
        }
    };
    Reply {
        request_id: Some(frame.request_id),
        result,
    }
}

#[cfg(test)]
fn verify_target(ctx: &SearchContext, id: &str, hash: &str) -> Result<(), ReviewError> {
    let request = Request {
        protocol_version: PROTOCOL_VERSION,
        request_id: new_bootstrap_event_id().map_err(|_| ReviewError::Unavailable)?,
        client_role: "desktop".into(),
        capabilities: vec![CAPABILITY_CONTENT_SEARCH.into()],
        operation: "content.event.detail".into(),
        query: Some(DesktopSearchQuery {
            event_id: Some(id.into()),
            ..DesktopSearchQuery::default()
        }),
        capture: None,
        capture_resolve: None,
        vault: None,
    };
    let result = handle_event_detail_until(
        request,
        Some(ctx),
        true,
        Some(Instant::now() + IPC_REQUEST_DEADLINE),
    );
    let Some([hit]) = result.hits.as_deref() else {
        return Err(ReviewError::TargetUnavailable);
    };
    if result.accepted && hit.can_review_purge && hit.event_id == id && hit.content_hash == hash {
        Ok(())
    } else {
        Err(ReviewError::TargetUnavailable)
    }
}

#[cfg(test)]
fn decide(ctx: &SearchContext, action: Action) -> Result<Outcome, ReviewError> {
    let ctx = ctx.admitted().map_err(|_| ReviewError::Unavailable)?;
    let state = ctx.purge_reviews.as_ref().ok_or(ReviewError::Disabled)?;
    let mut pending = state.try_lock().map_err(|_| ReviewError::Unavailable)?;
    match action {
        Action::Prepare { note_id, note_hash } => {
            verify_target(&ctx, &note_id, &note_hash)?;
            let ticket = new_bootstrap_event_id().map_err(|_| ReviewError::Unavailable)?;
            *pending = Some(Pending {
                ticket: ticket.clone(),
                note_id,
                note_hash,
                expires: Instant::now() + Duration::from_secs(300),
            });
            #[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
            let remove_locally = physical::enabled(&ctx.runtime_root);
            #[cfg(not(any(
                windows,
                all(target_os = "macos", feature = "m8-local-startup-check")
            )))]
            let remove_locally = false;
            Ok(Outcome::Prepared {
                ticket,
                remove_locally,
            })
        }
        Action::Cancel { ref ticket } | Action::Confirm { ref ticket } => {
            if !pending.as_ref().is_some_and(|p| p.ticket == *ticket) {
                return Err(ReviewError::ReviewExpired);
            }
            let current = pending.take().ok_or(ReviewError::ReviewExpired)?;
            if matches!(action, Action::Cancel { .. }) {
                return Ok(Outcome::Cancelled);
            }
            if Instant::now() >= current.expires {
                return Err(ReviewError::ReviewExpired);
            }
            verify_target(&ctx, &current.note_id, &current.note_hash)?;
            #[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
            {
                receipt::persist(&ctx, &current.note_id, &current.note_hash, current.expires)?;
                if physical::enabled(&ctx.runtime_root) {
                    return Ok(Outcome::Purged {
                        note_id: current.note_id,
                        note_hash: current.note_hash,
                    });
                }
                Ok(Outcome::Confirmed {
                    note_id: current.note_id,
                    note_hash: current.note_hash,
                })
            }
            #[cfg(not(any(
                windows,
                all(target_os = "macos", feature = "m8-local-startup-check")
            )))]
            Err(ReviewError::Disabled)
        }
    }
}

pub fn desktop(action: Action) -> Result<Outcome, ReviewError> {
    let exchange = || -> io::Result<Reply> {
        let request = Frame {
            protocol_version: PROTOCOL_VERSION,
            request_id: new_bootstrap_event_id().map_err(|_| invalid_data("request_id"))?,
            client_role: "desktop".into(),
            capabilities: vec![CAPABILITY.into()],
            operation: OPERATION.into(),
            action,
        };
        let mut stream = Stream::connect(socket_name()?)?;
        arm_ipc_stream(&stream)?;
        let deadline = Instant::now() + IPC_RESPONSE_DEADLINE;
        let mut frame = serde_json::to_vec(&request)?;
        frame.push(b'\n');
        write_ipc_all(&mut stream, &frame, deadline)?;
        let reply: Reply =
            serde_json::from_slice(&read_ipc_frame(&mut stream, MAX_RESPONSE_BYTES, deadline)?)?;
        if reply.request_id.as_deref() != Some(&request.request_id) {
            return Err(invalid_data("review_reply"));
        }
        Ok(reply)
    };
    exchange().map_err(|_| ReviewError::Unavailable)?.result
}
