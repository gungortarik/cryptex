use super::*;
use crate::knowledge_materialization::{ai_context, recovered_search};

pub(super) const OPERATION: &str = "context.public.search";
const CAPABILITY: &str = "context.public.read";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    protocol_version: u32,
    request_id: String,
    client_role: String,
    capabilities: Vec<String>,
    operation: String,
    phrase: String,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Error {
    InvalidRequest,
    Unauthorized,
    ReadHeld,
}

fn read(
    ctx: &SearchContext,
    phrase: &str,
    deadline: Instant,
) -> Result<Vec<ai_context::PublicNoteExcerpt>, Error> {
    let root = &ctx.runtime_root;
    let brain = load_brain_identity(root)
        .map_err(|_| Error::ReadHeld)?
        .brain_id;
    let identity = load_local_device_identity(root, &brain).map_err(|_| Error::ReadHeld)?;
    let device = DeviceId::new(identity.device_id).map_err(|_| Error::ReadHeld)?;
    let store = ctx.store.as_ref();
    if recovered_search::for_profile(root, &brain, &device, store, ctx.environment)
        .map_err(|_| Error::ReadHeld)?
        .is_some()
    {
        // This reader does not yet verify both recovered histories as a unit.
        return Err(Error::ReadHeld);
    }
    let mut resolver = LocalDeviceKeyResolver::load(root, &brain, store, ctx.environment)
        .map_err(|_| Error::ReadHeld)?;
    let db = ProjectionDb::open_existing_read_only(root).map_err(|_| Error::ReadHeld)?;
    let mut notes = ai_context::read_public_note_context(
        root,
        &brain,
        &[device],
        &mut resolver,
        &db,
        phrase,
        deadline,
    )
    .map_err(|_| Error::ReadHeld)?;
    for note in &mut notes {
        if let Some((end, _)) = note.text.char_indices().nth(240) {
            note.text.truncate(end);
        }
    }
    Ok(notes)
}

pub(super) fn respond(
    stream: &mut Stream,
    ctx: Option<&SearchContext>,
    bytes: &[u8],
    owner_approved_stream: bool,
) -> io::Result<()> {
    let deadline = Instant::now() + IPC_RESPONSE_DEADLINE;
    let frame = serde_json::from_slice::<Frame>(bytes).ok();
    let valid = frame
        .as_ref()
        .filter(|f| cryptex_core::validate::parse_uuid_str(&f.request_id).is_ok());
    let id = valid.map(|f| &f.request_id);
    // Keep this profile's operation lease alive through the last response byte.
    let mut admitted = None;
    let result = (|| {
        let f = valid.ok_or(Error::InvalidRequest)?;
        if f.operation != OPERATION
            || f.protocol_version != PROTOCOL_VERSION
            || f.phrase.trim().is_empty()
            || f.phrase.len() > MAX_SEARCH_TEXT_BYTES
        {
            return Err(Error::InvalidRequest);
        }
        if f.client_role != "mcp" || f.capabilities != [CAPABILITY] {
            return Err(Error::Unauthorized);
        }
        let ctx = ctx.ok_or(Error::Unauthorized)?;
        if !owner_approved_stream {
            return Err(Error::Unauthorized);
        }
        admitted = Some(ctx.admitted().map_err(|_| Error::ReadHeld)?);
        let notes = read(ctx, &f.phrase, deadline)?;
        ctx.admission
            .check_namespace()
            .map_err(|_| Error::ReadHeld)?;
        if Instant::now() >= deadline {
            return Err(Error::ReadHeld);
        }
        Ok(notes)
    })();
    let response = serde_json::json!({
        "protocol_version": PROTOCOL_VERSION, "request_id": id, "operation": OPERATION, "result": result
    });
    write_response(
        stream,
        &serde_json::to_vec(&response)?,
        Instant::now() + IPC_REQUEST_DEADLINE,
    )
}
