//! One human decision authorizes one read on the requesting connection.
use super::*;
use std::sync::mpsc;

pub(super) const REQUEST: &str = "context.public.request";
pub(super) const REVIEW: &str = "context.public.review";
const OWNER: &str = "context.public.owner";
const WINDOW: Duration = Duration::from_secs(120);

#[derive(Default, Clone)]
pub(crate) struct Session(Arc<Mutex<Access>>);
#[derive(Default)]
struct Access {
    pending: Option<Pending>,
    paused: bool,
    maintenance: bool,
    generation: Arc<()>,
}
impl Access {
    fn revoke_grants(&mut self) {
        self.pending = None;
        self.generation = Arc::new(());
    }
}
struct Pending {
    ticket: String,
    peer_pid: u32,
    until: Instant,
    answer: mpsc::Sender<bool>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Inspect,
    Status,
    Pause,
    Resume,
    Allow { ticket: String },
    Deny { ticket: String },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outcome {
    Empty,
    Requests { paused: bool, maintenance: bool },
    Review { ticket: String, peer_pid: u32 },
    Allowed,
    Denied,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    Unavailable,
    InvalidRequest,
    Unauthorized,
    Expired,
}

impl Session {
    #[cfg(any(
        windows,
        all(target_os = "macos", feature = "m8-local-startup-check"),
        test
    ))]
    pub(crate) fn set_maintenance(&self, enabled: bool) -> Result<(), Error> {
        let mut access = self.0.lock().map_err(|_| Error::Unavailable)?;
        if enabled {
            access.revoke_grants();
        }
        access.maintenance = enabled;
        Ok(())
    }

    pub(super) fn decide(&self, action: Action) -> Result<Outcome, Error> {
        let mut access = self.0.lock().map_err(|_| Error::Unavailable)?;
        let slot = &mut access.pending;
        if slot.as_ref().is_some_and(|p| Instant::now() >= p.until) {
            *slot = None;
        }
        let allow = matches!(action, Action::Allow { .. });
        match action {
            Action::Status => Ok(Outcome::Requests {
                paused: access.paused || access.maintenance,
                maintenance: access.maintenance,
            }),
            Action::Pause | Action::Resume => {
                if access.maintenance {
                    return Err(Error::Unavailable);
                }
                if matches!(action, Action::Pause) {
                    access.revoke_grants();
                    access.paused = true;
                } else {
                    access.paused = false;
                }
                Ok(Outcome::Requests {
                    paused: access.paused || access.maintenance,
                    maintenance: access.maintenance,
                })
            }
            Action::Inspect => Ok(slot.as_ref().map_or(Outcome::Empty, |p| Outcome::Review {
                ticket: p.ticket.clone(),
                peer_pid: p.peer_pid,
            })),
            Action::Allow { ticket } | Action::Deny { ticket } => {
                if !slot.as_ref().is_some_and(|p| p.ticket == ticket) {
                    return Err(Error::Expired);
                }
                let pending = slot.take().ok_or(Error::Expired)?;
                pending.answer.send(allow).map_err(|_| Error::Expired)?;
                Ok(if allow {
                    Outcome::Allowed
                } else {
                    Outcome::Denied
                })
            }
        }
    }
}

pub(super) fn review_frame(
    bytes: &[u8],
    ctx: Option<&SearchContext>,
    desktop: bool,
) -> backup_request::Reply<Outcome, Error> {
    backup_request::handle(
        bytes,
        desktop,
        REVIEW,
        OWNER,
        Error::InvalidRequest,
        Error::Unauthorized,
        |action| ctx.ok_or(Error::Unavailable)?.public_consent.decide(action),
    )
}

pub fn desktop(action: Action) -> Result<Outcome, Error> {
    backup_request::exchange(REVIEW, OWNER, action, IPC_RESPONSE_DEADLINE)
        .map_err(|_| Error::Unavailable)?
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestFrame {
    protocol_version: u32,
    request_id: String,
    operation: String,
    client_role: String,
    capabilities: Vec<String>,
}

pub(super) fn request(
    stream: &mut Stream,
    ctx: Option<&SearchContext>,
    bytes: &[u8],
) -> io::Result<()> {
    let f: RequestFrame =
        serde_json::from_slice(bytes).map_err(|_| invalid_data("invalid consent request"))?;
    if f.protocol_version != PROTOCOL_VERSION
        || f.operation != REQUEST
        || f.client_role != "mcp"
        || f.capabilities != ["context.public.read"]
        || cryptex_core::validate::parse_uuid_str(&f.request_id).is_err()
    {
        return Err(invalid_data("invalid consent request"));
    }
    let state = |stream: &mut Stream, status: &str, ticket: Option<&str>| {
        let reply = serde_json::json!({"protocol_version":PROTOCOL_VERSION,"operation":REQUEST,
            "request_id": f.request_id, "status":status,"ticket":ticket});
        write_response(
            stream,
            &serde_json::to_vec(&reply)?,
            Instant::now() + IPC_REQUEST_DEADLINE,
        )
    };
    let Some(ctx) = ctx else {
        return state(stream, "denied", None);
    };
    let peer_pid = peer_process_id(stream)?;
    let ticket = new_bootstrap_event_id().map_err(invalid_data)?;
    let until = Instant::now() + WINDOW;
    let (answer, receive) = mpsc::channel();
    let generation = {
        let mut access = ctx
            .public_consent
            .0
            .lock()
            .map_err(|_| invalid_data("consent unavailable"))?;
        if access.paused || access.maintenance {
            drop(access);
            return state(stream, "denied", None);
        }
        if access
            .pending
            .as_ref()
            .is_some_and(|p| Instant::now() < p.until)
        {
            drop(access);
            return state(stream, "busy", None);
        }
        access.pending = Some(Pending {
            ticket: ticket.clone(),
            peer_pid,
            until,
            answer,
        });
        access.generation.clone()
    };
    let sent = state(stream, "pending", Some(&ticket));
    let allowed = sent.is_ok()
        && receive.recv_timeout(until.saturating_duration_since(Instant::now())) == Ok(true);
    {
        let mut access = ctx
            .public_consent
            .0
            .lock()
            .map_err(|_| invalid_data("consent unavailable"))?;
        if access.pending.as_ref().is_some_and(|p| p.ticket == ticket) {
            access.pending = None;
        }
    }
    sent?;
    if !allowed || Instant::now() >= until {
        return state(stream, "denied", Some(&ticket));
    }
    state(stream, "allowed", Some(&ticket))?;
    // No grant can be presented on a new socket or carried to another profile.
    let frame = read_ipc_frame(stream, MAX_FRAME_BYTES, until)?;
    if frame.is_empty() || Instant::now() >= until {
        return Ok(());
    }
    // Serialize pause with this bounded response. Once pause is acknowledged,
    // a previously granted socket cannot return more data, even after resume.
    let access = ctx
        .public_consent
        .0
        .lock()
        .map_err(|_| invalid_data("consent unavailable"))?;
    let valid = !access.paused
        && !access.maintenance
        && Arc::ptr_eq(&access.generation, &generation)
        && Instant::now() < until;
    public_context::respond(stream, Some(ctx), &frame, valid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maintenance_revokes_an_outstanding_grant_even_after_exit() {
        let session = Session::default();
        let generation = session.0.lock().unwrap().generation.clone();
        session.set_maintenance(true).unwrap();
        assert!(matches!(
            session.decide(Action::Resume),
            Err(Error::Unavailable)
        ));
        session.set_maintenance(false).unwrap();
        let access = session.0.lock().unwrap();
        assert!(!access.paused && !access.maintenance);
        assert!(!Arc::ptr_eq(&generation, &access.generation));
    }

    #[test]
    fn pause_cancels_the_pending_ticket_and_resume_requires_a_new_review() {
        let session = Session::default();
        let (answer, receiver) = mpsc::channel();
        let generation = {
            let mut access = session.0.lock().unwrap();
            access.pending = Some(Pending {
                ticket: "first".into(),
                peer_pid: 42,
                until: Instant::now() + WINDOW,
                answer,
            });
            access.generation.clone()
        };
        assert!(matches!(
            session.decide(Action::Pause),
            Ok(Outcome::Requests { paused: true, .. })
        ));
        assert!(receiver.recv().is_err());
        assert!(matches!(
            session.decide(Action::Resume),
            Ok(Outcome::Requests { paused: false, .. })
        ));
        assert!(!Arc::ptr_eq(
            &generation,
            &session.0.lock().unwrap().generation
        ));
        assert!(matches!(
            session.decide(Action::Allow {
                ticket: "first".into()
            }),
            Err(Error::Expired)
        ));
    }

    #[test]
    fn expired_ticket_releases_the_waiter_and_cannot_authorize_a_read() {
        let state = Session::default();
        let (answer, receiver) = mpsc::channel();
        state.0.lock().unwrap().pending = Some(Pending {
            ticket: "expired".into(),
            peer_pid: 42,
            until: Instant::now(),
            answer,
        });
        assert!(matches!(
            state.decide(Action::Allow {
                ticket: "expired".into()
            }),
            Err(Error::Expired)
        ));
        assert!(matches!(state.decide(Action::Inspect), Ok(Outcome::Empty)));
        assert!(receiver.recv().is_err());
    }
}
