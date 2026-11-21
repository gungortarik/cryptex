//! Desktop may inspect or consume a verified removal already prepared by the daemon.
use super::*;
pub(super) const OPERATION: &str = "backup.removal.review";
const CAPABILITY: &str = "backup.removal.owner";

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Inspect,
    Keep { ticket: String },
    Remove { ticket: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub(crate) old_file: String,
    pub(crate) old_digest: String,
    pub(crate) replacement_file: String,
    pub(crate) replacement_digest: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    Empty,
    Review {
        ticket: String,
        selection: Selection,
    },
    Kept,
    Removed,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    Unavailable,
    InvalidRequest,
    Unauthorized,
    Expired,
    VerificationFailed,
}

pub(super) type Session = Arc<Mutex<Option<Pending>>>;

pub(super) struct Pending {
    ticket: String,
    selection: Selection,
    expires: Instant,
    remove: Box<dyn FnOnce() -> io::Result<bool> + Send>,
}

#[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
impl Pending {
    pub(super) fn from_review(
        ctx: &SearchContext,
        enrolled: cryptex_events::LocalTrustAnchor,
        review: crate::backup_scheduler::retention::removal::Review,
    ) -> io::Result<Self> {
        use crate::backup_scheduler::retention::{VerifiedBackups, removal::Decision};
        let (selection, expires) = review.details()?;
        let root = ctx.runtime_root.clone();
        let store = ctx.store.clone();
        let environment = ctx.environment;
        let file = review.selected_file();
        Ok(Self {
            ticket: new_bootstrap_event_id().map_err(|_| invalid_data("review ticket"))?,
            selection,
            expires,
            remove: Box::new(move || {
                let catalog = VerifiedBackups {
                    target: file.parent().ok_or_else(|| invalid_data("backup target"))?,
                    writer_profile: &root,
                    store: store.as_ref(),
                    environment,
                    designated: &enrolled,
                };
                catalog.decide_removal(&enrolled, review, Decision::Confirm(file.clone()))
            }),
        })
    }
}

pub(super) fn decide(ctx: &SearchContext, action: Action) -> Result<Outcome, Error> {
    let ctx = ctx.admitted().map_err(|_| Error::Unavailable)?;
    let mut slot = ctx.backup_removal.lock().map_err(|_| Error::Unavailable)?;
    if slot
        .as_ref()
        .is_some_and(|item| Instant::now() >= item.expires)
    {
        slot.take();
    }
    let (ticket, remove) = match action {
        Action::Inspect => {
            return Ok(match slot.as_ref() {
                None => Outcome::Empty,
                Some(item) => Outcome::Review {
                    ticket: item.ticket.clone(),
                    selection: item.selection.clone(),
                },
            });
        }
        Action::Keep { ticket } => (ticket, false),
        Action::Remove { ticket } => (ticket, true),
    };
    if slot.as_ref().is_none_or(|item| item.ticket != ticket) {
        return Err(Error::Expired);
    }
    let pending = slot.take().ok_or(Error::Expired)?;
    drop(slot);
    if !remove {
        return Ok(Outcome::Kept);
    }
    match (pending.remove)() {
        Ok(true) => Ok(Outcome::Removed),
        _ => Err(Error::VerificationFailed),
    }
}

pub(super) fn handle(
    bytes: &[u8],
    ctx: Option<&SearchContext>,
    desktop: bool,
) -> super::backup_request::Reply<Outcome, Error> {
    super::backup_request::handle(
        bytes,
        desktop,
        OPERATION,
        CAPABILITY,
        Error::InvalidRequest,
        Error::Unauthorized,
        |action| decide(ctx.ok_or(Error::Unavailable)?, action),
    )
}

pub fn desktop(action: Action) -> Result<Outcome, Error> {
    super::backup_request::exchange(OPERATION, CAPABILITY, action, IPC_RESPONSE_DEADLINE)
        .map_err(|_| Error::Unavailable)?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requires_authenticated_desktop_and_rejects_renderer_paths() {
        let mut frame = serde_json::json!({
            "request_id": new_bootstrap_event_id().unwrap(),
            "protocol_version": PROTOCOL_VERSION,
            "operation": OPERATION, "client_role": "desktop", "capabilities": [CAPABILITY],
            "action": {"kind": "remove", "ticket": "fixture"}
        });
        let send = |frame: &serde_json::Value, authenticated| {
            handle(&serde_json::to_vec(frame).unwrap(), None, authenticated).result
        };
        assert!(matches!(send(&frame, false), Err(Error::Unauthorized)));
        frame["client_role"] = "ai".into();
        assert!(matches!(send(&frame, true), Err(Error::Unauthorized)));
        frame["client_role"] = "desktop".into();
        frame["capabilities"] = serde_json::json!(["content.search"]);
        assert!(matches!(send(&frame, true), Err(Error::Unauthorized)));
        frame["capabilities"] = serde_json::json!([CAPABILITY]);
        frame["action"]["path"] = "some-other-backup.pgp".into();
        assert!(matches!(send(&frame, true), Err(Error::InvalidRequest)));
    }
}
