//! Feature-gated, one-note rehearsal. The daemon allocates the only writable profile.
use super::{LifecycleError, random_uuid_v4};
use cryptex_core::{BrainIdentity, DeviceId, validate::parse_uuid_str};
use cryptex_cortex_inventory::reviewed_payload;
use cryptex_events::{Classification, ContentHashHex, EventPayload};
use cryptexd::{
    FakeSecureStore, SecureStoreEnvironment,
    canonical_writer::{CanonicalWriterConfig, load_verified_events},
    capture::{CaptureOutcome, CaptureRequest, capture_memory_note},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, BufRead, Read, Write},
    path::Path,
};

const DEV: SecureStoreEnvironment = SecureStoreEnvironment::Dev;
const MAX_APPROVAL: u64 = 8192;

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Approval {
    capture_id: String,
    source_path: String,
    source_sha256: String,
    classification: Classification,
}

fn fault(code: &str) -> LifecycleError {
    LifecycleError::Attention(code.into())
}

// Store the owner decision before capture. Its capture ID also identifies the
// daemon's idempotent capture receipt, including if its first reply gets lost.
fn bind(root: &Path, approval: &Approval) -> io::Result<bool> {
    let path = root.join("cortex-note-approval.json");
    let mut pending = tempfile::NamedTempFile::new_in(root)?;
    serde_json::to_writer(pending.as_file_mut(), approval)?;
    pending.as_file().sync_all()?;
    match pending.persist_noclobber(&path) {
        Ok(_) => Ok(true),
        Err(e) if e.error.kind() == io::ErrorKind::AlreadyExists => {
            let stored: Approval = serde_json::from_reader(fs::File::open(path)?)?;
            Ok(&stored == approval)
        }
        Err(e) => Err(e.error),
    }
}

fn capture(
    source: &Path,
    root: &Path,
    store: &FakeSecureStore,
    bytes: &[u8],
) -> Result<Value, LifecycleError> {
    let Ok(approval) = serde_json::from_slice::<Approval>(bytes) else {
        return Ok(json!({"status":"rejected","issue":"invalid_approval"}));
    };
    if parse_uuid_str(&approval.capture_id).is_err()
        || ContentHashHex::parse(&approval.source_sha256).is_err()
    {
        return Ok(json!({"status":"rejected","issue":"invalid_approval"}));
    }
    let payload = match reviewed_payload(source, &approval.source_path, &approval.source_sha256) {
        Ok(payload) => payload,
        Err(issue) => return Ok(json!({"status":"rejected","issue":issue})),
    };
    if !bind(root, &approval).map_err(|_| fault("approval_storage_failed"))? {
        return Ok(json!({"status":"rejected","issue":"approval_conflict"}));
    }
    let request = CaptureRequest {
        capture_id: approval.capture_id.clone(),
        text: payload.text,
        classification: approval.classification,
    };
    Ok(
        match capture_memory_note(store, DEV, root, request, &CanonicalWriterConfig::default()) {
            CaptureOutcome::Committed(commit) => json!({
                "status":"committed", "approval":approval, "event_id":commit.event_id,
                "brain_id":commit.brain_id, "content_hash":commit.content_hash
            }),
            CaptureOutcome::Rejected { code, .. } => {
                json!({"status":"rejected","issue":format!("{code:?}")})
            }
            CaptureOutcome::Uncertain { reason, .. } => {
                json!({"status":"uncertain","issue":format!("{reason:?}")})
            }
        },
    )
}

fn emit(output: &mut impl Write, value: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut *output, value)?;
    writeln!(output)?;
    output.flush()
}

pub(super) fn run(args: &[String]) -> Result<(), LifecycleError> {
    let [source] = args else {
        return Err(LifecycleError::Usage(
            "cortex-import-disposable SOURCE_DIRECTORY".into(),
        ));
    };
    let temp = fs::canonicalize(std::env::temp_dir())?;
    cryptexd::paths::assert_outside_repository(&temp)?;
    if temp.starts_with(fs::canonicalize(source)?) {
        return Err(fault("temporary_profile_would_modify_source"));
    }
    let profile = tempfile::Builder::new()
        .prefix("cryptex-m15-")
        .tempdir_in(temp)?;
    let root = profile.path();
    let store = FakeSecureStore::new();
    let _instance = cryptexd::instance::try_acquire_instance(root)?;
    cryptexd::install_runtime::prepare_m1_runtime_foundation(root)
        .map_err(|_| fault("trial_install_failed"))?;
    let brain = BrainIdentity::new(random_uuid_v4()?, "2026-11-22T00:00:00Z")
        .map_err(|_| fault("trial_brain_failed"))?;
    cryptexd::identity_runtime::write_brain_identity(root, &brain)
        .map_err(|_| fault("trial_brain_failed"))?;
    let local = cryptexd::local_device_identity::enroll_local_device(root, &store, DEV)
        .map_err(|_| fault("trial_enrollment_failed"))?;
    let device = DeviceId::new(local.device_id).map_err(|_| fault("trial_device_failed"))?;
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    loop {
        let mut line = Vec::new();
        if (&mut input)
            .take(MAX_APPROVAL + 1)
            .read_until(b'\n', &mut line)?
            == 0
        {
            break;
        }
        if line.len() as u64 > MAX_APPROVAL {
            return Err(LifecycleError::Usage("approval_too_large".into()));
        }
        emit(
            &mut output,
            &capture(Path::new(source), root, &store, &line)?,
        )?;
    }

    // Reopen the on-disk packs through the daemon's signature and chain verifier.
    let events = load_verified_events(&store, DEV, root, &device)
        .map_err(|_| fault("signed_readback_failed"))?;
    let verified: Vec<_> = events.iter().map(|event| {
        let EventPayload::MemoryNote(note) = &event.payload;
        json!({"event_id":event.event_id,"brain_id":event.brain_id,"event_type":event.event_type,
               "content_hash":event.content_hash,"classification":event.classification,
               "text_sha256":format!("{:x}", Sha256::digest(note.text.as_bytes()))})
    }).collect();
    emit(&mut output, &json!({"verified_notes":verified}))?;
    Ok(())
}
