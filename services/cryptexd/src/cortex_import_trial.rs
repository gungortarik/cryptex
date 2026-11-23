//! Feature-gated import rehearsals. The daemon allocates the writable trial profiles.
use super::{LifecycleError, random_uuid_v4};
#[cfg(all(
    test,
    feature = "m11-installed-search-check",
    any(windows, target_os = "macos")
))]
mod purge_backup_fixture;
use cryptexd::secure_store::SecureStore;
#[cfg(any(windows, target_os = "macos"))]
mod cutover;
#[cfg(any(windows, target_os = "macos"))]
mod decision;
mod provenance;
#[cfg(any(windows, target_os = "macos"))]
mod restart;
#[cfg(any(windows, target_os = "macos"))]
mod selection;
use cryptex_core::{BrainIdentity, DeviceId, validate::parse_uuid_str};
use cryptex_cortex_inventory::{review_notes, reviewed_payload};
use cryptex_events::{ContentHashHex, EventPayload};
use cryptexd::cortex_evidence::Approval;
use cryptexd::{
    FakeSecureStore, SecureStoreEnvironment,
    canonical_writer::{CanonicalWriterConfig, load_verified_events},
    capture::{CaptureOutcome, CaptureRequest, capture_memory_note},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, BufRead, Read, Write},
    path::Path,
};

const DEV: SecureStoreEnvironment = SecureStoreEnvironment::Dev;
const MAX_APPROVAL: u64 = 8192;

#[derive(Clone, Copy, PartialEq, Eq)]
enum TrialMode {
    Capture,
    ExitBeforeReply,
    Replay,
    Batch,
    BatchExitBeforeSourceReceipt,
}

fn fault(code: &str) -> LifecycleError {
    LifecycleError::Attention(code.into())
}

// Store the owner decision before capture. Its capture ID also identifies the
// daemon's idempotent capture receipt, including if its first reply gets lost.
fn check_saved(path: &Path, approval: &Approval) -> io::Result<Option<bool>> {
    match fs::File::open(path) {
        Ok(file) => Ok(Some(
            &serde_json::from_reader::<_, Approval>(file)? == approval,
        )),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

fn bind(root: &Path, path: &Path, approval: &Approval) -> io::Result<bool> {
    let mut pending = tempfile::NamedTempFile::new_in(root)?;
    serde_json::to_writer(pending.as_file_mut(), approval)?;
    pending.as_file().sync_all()?;
    match pending.persist_noclobber(path) {
        Ok(_) => Ok(true),
        Err(e) if e.error.kind() == io::ErrorKind::AlreadyExists => {
            Ok(check_saved(path, approval)? == Some(true))
        }
        Err(e) => Err(e.error),
    }
}

fn capture(
    source: &Path,
    root: &Path,
    store: &impl SecureStore,
    bytes: &[u8],
    mode: TrialMode,
    mut batch: Option<&mut provenance::Batch>,
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
    let path = root.join("cortex-note-approval.json");
    let saved = match &batch {
        Some(batch) => batch.check(&approval),
        None => check_saved(&path, &approval).map_err(|_| fault("approval_storage_failed"))?,
    };
    if saved == Some(false) {
        return Ok(json!({"status":"rejected","issue":"approval_conflict"}));
    }
    if mode == TrialMode::Replay && saved.is_none() {
        return Err(fault("approval_storage_failed"));
    }
    let previously_committed = saved == Some(true)
        && matches!(
            cryptexd::capture::resolve_capture(store, DEV, root, &approval.capture_id),
            CaptureOutcome::Committed(_)
        );
    if (mode == TrialMode::Replay || (batch.is_some() && saved == Some(true)))
        && !previously_committed
    {
        return Ok(json!({"status":"uncertain","issue":"prior_commit_unverified"}));
    }
    if !previously_committed {
        let review = review_notes(source);
        if review.review_sha256.is_none()
            || review.notes.get(&approval.source_path) != Some(&approval.source_sha256)
            || ((review.requires_choice(&approval.source_path) || approval.review_sha256.is_some())
                && approval.review_sha256 != review.review_sha256)
        {
            return Ok(json!({"status":"needs_review","review":review}));
        }
        if saved.is_none() {
            if let Some(batch) = batch.as_mut() {
                batch.reserve(root, &approval)?;
            } else if !bind(root, &path, &approval).map_err(|_| fault("approval_storage_failed"))? {
                return Ok(json!({"status":"rejected","issue":"approval_conflict"}));
            }
        }
    }
    let request = CaptureRequest {
        capture_id: approval.capture_id.clone(),
        text: payload.text,
        classification: approval.classification,
    };
    Ok(
        match capture_memory_note(store, DEV, root, request, &CanonicalWriterConfig::default()) {
            // Terminate the disposable daemon after durable commit, before
            // serializing the caller's receipt or running session cleanup.
            CaptureOutcome::Committed(_) if mode == TrialMode::ExitBeforeReply => {
                std::process::exit(86)
            }
            CaptureOutcome::Committed(commit) => {
                if mode == TrialMode::BatchExitBeforeSourceReceipt {
                    std::process::exit(87);
                }
                if let Some(batch) = batch {
                    batch.record(root, &approval, &commit)?;
                    batch.finish(root, &approval.capture_id)?;
                }
                json!({
                "status":"committed", "approval":approval, "event_id":commit.event_id,
                "brain_id":commit.brain_id, "content_hash":commit.content_hash
                })
            }
            CaptureOutcome::Rejected { code, .. } => {
                if saved.is_none()
                    && let Some(batch) = batch
                {
                    batch.finish(root, &approval.capture_id)?;
                }
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
    #[cfg(any(windows, target_os = "macos"))]
    if let [flag, id, digest] = args
        && let Some(choice) = decision::Choice::from_flag(flag)
    {
        ContentHashHex::parse(digest).map_err(|_| fault("invalid_review_digest"))?;
        return restart::run(None, "--compare-selection", id, Some((choice, digest)));
    }
    #[cfg(any(windows, target_os = "macos"))]
    if let [flag, id] = args
        && matches!(
            flag.as_str(),
            "--provenance" | "--compare-selection" | "--open-accepted" | "--remove-trial-key"
        )
    {
        return restart::run(None, flag, id, None);
    }
    #[cfg(any(windows, target_os = "macos"))]
    if let [source, mode, trial_id] = args {
        return restart::run(Some(Path::new(source)), mode, trial_id, None);
    }
    let [source] = args else {
        return Err(LifecycleError::Usage(
            "cortex-import-disposable SOURCE_DIRECTORY".into(),
        ));
    };
    let temp = trial_temp(Path::new(source))?;
    let profile = tempfile::Builder::new()
        .prefix("cryptex-m15-")
        .tempdir_in(temp)?;
    let root = profile.path();
    let store = FakeSecureStore::new();
    let _instance = cryptexd::instance::try_acquire_instance(root)?;
    let device = initialize(root, &store, random_uuid_v4()?)?;
    session(Path::new(source), root, &device, &store, TrialMode::Capture)
}

fn trial_temp(source: &Path) -> Result<std::path::PathBuf, LifecycleError> {
    let temp = temp_root()?;
    if temp.starts_with(fs::canonicalize(source)?) {
        return Err(fault("temporary_profile_would_modify_source"));
    }
    Ok(temp)
}

fn temp_root() -> Result<std::path::PathBuf, LifecycleError> {
    let temp = fs::canonicalize(std::env::temp_dir())?;
    cryptexd::paths::assert_outside_repository(&temp)?;
    Ok(temp)
}

fn initialize(
    root: &Path,
    store: &impl SecureStore,
    brain_id: String,
) -> Result<DeviceId, LifecycleError> {
    cryptexd::install_runtime::prepare_m1_runtime_foundation(root)
        .map_err(|_| fault("trial_install_failed"))?;
    let brain = BrainIdentity::new(brain_id, "2026-11-22T00:00:00Z")
        .map_err(|_| fault("trial_brain_failed"))?;
    cryptexd::identity_runtime::write_brain_identity(root, &brain)
        .map_err(|_| fault("trial_brain_failed"))?;
    let local = cryptexd::local_device_identity::enroll_local_device(root, store, DEV)
        .map_err(|_| fault("trial_enrollment_failed"))?;
    DeviceId::new(local.device_id).map_err(|_| fault("trial_device_failed"))
}

fn session(
    source: &Path,
    root: &Path,
    device: &DeviceId,
    store: &impl SecureStore,
    mode: TrialMode,
) -> Result<(), LifecycleError> {
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    let mut batch = if matches!(
        mode,
        TrialMode::Batch | TrialMode::BatchExitBeforeSourceReceipt
    ) {
        Some(provenance::Batch::create(root, device, store)?)
    } else {
        None
    };
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
            &capture(source, root, store, &line, mode, batch.as_mut())?,
        )?;
    }

    // Reopen the on-disk packs through the daemon's signature and chain verifier.
    let events = load_verified_events(store, DEV, root, device)
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
