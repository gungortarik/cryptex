use super::*;
use cryptex_events::{
    AuthenticatedSecurityRecord as Record, EventPayload, TrustedDaemonKeyResolver,
};
use std::sync::OnceLock;

static PROFILE: OnceLock<PathBuf> = OnceLock::new();

pub(crate) fn register(root: &Path) {
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("cryptex-ipc-search-lifecycle-")
    );
    PROFILE.set(root.to_owned()).unwrap();
}

fn registered(root: &Path) -> bool {
    PROFILE.get().is_some_and(|p| p == root)
}

pub(super) fn execute(ctx: Option<&SearchContext>, action: Action) -> Result<Outcome, Error> {
    let ctx = ctx
        .filter(|ctx| registered(&ctx.runtime_root))
        .ok_or(Error::Disabled)?
        .admitted()
        .map_err(|_| Error::Unavailable)?;
    let root = &ctx.runtime_root;
    let _profile_lock = crate::import_staging::open_existing_profile_lock(
        &root.join("daemon.lock"),
        "reader-m9.pid",
    )
    .map_err(|_| Error::Disabled)?;
    let brain = crate::identity_runtime::load_brain_identity(root)
        .map_err(|_| Error::VerificationFailed)?;
    let resolver =
        LocalDeviceKeyResolver::load(root, &brain.brain_id, ctx.store.as_ref(), ctx.environment)
            .map_err(|_| Error::VerificationFailed)?;
    let anchor = resolver
        .local_trust_anchor()
        .ok_or(Error::VerificationFailed)?;
    let snapshot = crate::knowledge_materialization::lock_local_lifecycle_history(
        root,
        &brain.brain_id,
        &anchor,
        Instant::now() + IPC_REQUEST_DEADLINE,
    )
    .map_err(|_| Error::VerificationFailed)?;
    let (history, trust) = snapshot
        .verified_trust_context()
        .ok_or(Error::VerificationFailed)?;
    let device = anchor.device_id();
    if device != &history.local_device
        || history.restricted(trust, device)
        || trust.effective_cutoffs.get(device) != Some(&u64::MAX)
        || !snapshot.locks_runtime(root)
        || !snapshot.locks_devices(std::slice::from_ref(device))
    {
        return Err(Error::Unauthorized);
    }
    let chain = snapshot.reader_device_chain(device.as_str());
    let mut notes = std::collections::BTreeMap::new();
    let mut revisions = std::collections::BTreeMap::new();
    for record in chain {
        match record {
            Record::V1(note) => {
                notes.insert(note.event_id.as_str(), note);
            }
            Record::Lifecycle(raw, change) => {
                revisions.insert(change.note_event_id(), &raw.content_hash);
            }
            _ => {}
        }
    }
    let selection = |note: &cryptex_events::AcceptedEvent| Selection {
        note_id: note.event_id.clone(),
        note_hash: note.content_hash.as_str().to_owned(),
        state_hash: revisions
            .get(note.event_id.as_str())
            .copied()
            .unwrap_or(&note.content_hash)
            .as_str()
            .to_owned(),
    };
    let forget = matches!(action, Action::Forget { .. });
    let outcome = match action {
        Action::Inspect { note_id } => Outcome::Ready {
            target: note_id
                .as_deref()
                .filter(|id| !snapshot.is_forgotten(id))
                .and_then(|id| notes.get(id))
                .map(|note| selection(note)),
        },
        Action::List { after } => {
            let mut choices = Vec::new();
            let mut next = None;
            for (id, note) in notes {
                if after.as_deref().is_some_and(|cursor| id <= cursor) || !snapshot.is_forgotten(id)
                {
                    continue;
                }
                if choices.len() == 20 {
                    next = choices.last().map(|c: &Choice| c.target.note_id.clone());
                    break;
                }
                let EventPayload::MemoryNote(body) = &note.payload;
                choices.push(Choice {
                    target: selection(note),
                    preview: timeline_snippet(&body.text),
                    created_at: note.created_at.clone(),
                });
            }
            Outcome::Forgotten { choices, next }
        }
        Action::Forget { target } | Action::Restore { target } => {
            let note = notes
                .get(target.note_id.as_str())
                .ok_or(Error::StaleSelection)?;
            if note.content_hash.as_str() != target.note_hash
                || selection(note).state_hash != target.state_hash
                || snapshot.is_forgotten(&target.note_id) == forget
            {
                return Err(Error::StaleSelection);
            }
            let key = crate::event_signing_key::load_existing_event_signing_key(
                ctx.store.as_ref(),
                ctx.environment,
                root,
                device,
            )
            .map_err(|_| Error::VerificationFailed)?;
            if key.signing_key.verifying_key().as_bytes() != anchor.public_key()
                || key.public.signing_key_id != note.signing_key_id.as_str()
            {
                return Err(Error::VerificationFailed);
            }
            crate::canonical_writer::isolated_attachment::append_note_control(root, device, note, &key,
                if forget { "memory.forget" } else { "memory.restore" },
                serde_json::json!({"note_event_id":target.note_id,"note_content_hash":target.note_hash,"prior_state_hash":target.state_hash}))
                .map_err(|err| match err {
                    attachment::AttachError::VerificationFailed => Error::VerificationFailed,
                    attachment::AttachError::Uncertain => Error::Uncertain,
                    _ => Error::Unavailable,
                })?;
            Outcome::Changed
        }
    };
    ctx.admission
        .check_namespace()
        .map_err(|_| Error::Uncertain)?;
    Ok(outcome)
}
