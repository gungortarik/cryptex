//! Read-only union of the recovered snapshot and the first replacement's notes.
//! Confined to disposable M11 rehearsals.
use super::*;
use crate::secure_store::FileBackedSecureStore;

#[derive(Serialize, Deserialize)]
struct PublicContext {
    request: Request,
    kit: RecoveryKitPublicMetadata,
    pinned: [u8; 32],
    replacement_note_id: String,
}

pub(super) fn save_public_context(work: &Path, ceremony: &Ceremony, id: &str) {
    // Models independently provisioned local restore settings across a process exit.
    // Never derive the expected root or challenge from the presented receipt.
    let context = PublicContext {
        request: ceremony.expected.clone(),
        kit: ceremony.kit.clone(),
        pinned: ceremony.trusted_root.to_bytes(),
        replacement_note_id: id.into(),
    };
    fs::write(
        work.join("public-test-context.json"),
        serde_json::to_vec(&context).unwrap(),
    )
    .unwrap();
}

pub(in super::super) fn history_digest(snapshot: &VerifiedCanonicalSnapshot) -> [u8; 32] {
    // security_history is ordered by device and sequence; each content hash is
    // fixed-width. Distinguish a purge bridge from an original record with the
    // same chain hash.
    let mut digest = Sha256::new();
    for event in snapshot.security_history.records() {
        digest.update([u8::from(matches!(
            event,
            AuthenticatedSecurityRecord::PurgeBridge(..)
        ))]);
        digest.update(event.content_hash().as_str().as_bytes());
    }
    digest.finalize().into()
}

#[derive(Debug, PartialEq, Eq)]
enum SearchHold {
    Approval,
    Archive,
    ArchiveCheckpoint,
    Replacement,
    Overlap,
    Projection,
    Deadline,
    InvalidQuery,
}

fn search(
    work: &Path,
    ceremony: &Ceremony,
    query: &SearchQuery,
    deadline: Option<std::time::Instant>,
) -> Result<Vec<SearchHit>, SearchHold> {
    let old = work.join("rehearsal");
    let new = work.join("replacement");
    let store = FileBackedSecureStore::open(work.join("test-credentials"))
        .map_err(|_| SearchHold::Approval)?;
    let (mut local, mut archived) = ceremony
        .open_replacement(&new, &store)
        .map_err(|_| SearchHold::Approval)?;
    let (old_db, _) =
        crate::projection::open_projection_db(&old).map_err(|_| SearchHold::Archive)?;
    let (new_db, _) =
        crate::projection::open_projection_db(&new).map_err(|_| SearchHold::Replacement)?;
    let old_guard = evaluate_retrieval_safety_until(
        &old,
        BRAIN,
        &[],
        &mut archived,
        &old_db,
        deadline,
        NoteLifecycleRead::Apply,
    );
    let old_snapshot = old_guard.snapshot.as_ref().ok_or(SearchHold::Archive)?;
    if history_digest(old_snapshot) != ceremony.expected.archive_history_sha256 {
        return Err(SearchHold::ArchiveCheckpoint);
    }
    let new_guard = evaluate_retrieval_safety_until(
        &new,
        BRAIN,
        &[],
        &mut local,
        &new_db,
        deadline,
        NoteLifecycleRead::Apply,
    );
    let new_snapshot = new_guard.snapshot.as_ref().ok_or(SearchHold::Replacement)?;
    // The first-device trial admits a single fresh device's note stream. New
    // trust or lifecycle controls require reduction across both histories.
    let device = &ceremony.expected.new_device;
    if old_snapshot.locked_devices().contains(device)
        || new_snapshot.locked_devices() != std::slice::from_ref(device)
        || new_snapshot
            .security_history
            .records()
            .iter()
            .any(|r| !matches!(r, AuthenticatedSecurityRecord::V1(_)))
    {
        return Err(SearchHold::Overlap);
    }
    let ids: BTreeSet<_> = old_snapshot
        .security_history
        .records()
        .iter()
        .map(|e| e.event_id())
        .collect();
    if new_snapshot
        .security_history
        .records()
        .iter()
        .any(|e| ids.contains(e.event_id()))
    {
        return Err(SearchHold::Overlap);
    }
    // Both canonical locks stay held through the single FTS query. Project
    // only policy-admitted note bodies into this short-lived read model.
    let union = ProjectionDb::open_in_memory().map_err(|_| SearchHold::Projection)?;
    for snapshot in [old_snapshot, new_snapshot] {
        for record in snapshot.verified_records() {
            if deadline.is_some_and(|until| std::time::Instant::now() >= until) {
                return Err(SearchHold::Deadline);
            }
            if let VersionedRecord::VerifiedV1(note) = record
                && !snapshot.forgotten.contains(&note.event_id)
            {
                cryptex_projection::project_memory_note(union.connection(), note)
                    .map_err(|_| SearchHold::Projection)?;
            }
        }
    }
    let scoped = SearchQuery {
        brain_id: Some(BRAIN.into()),
        ..query.clone()
    };
    cryptex_projection::search::search_visible_events(
        union.connection(),
        &scoped,
        &BTreeSet::new(),
        deadline,
    )
    .map_err(|e| match e {
        ProjectionError::OperationDeadlineExceeded => SearchHold::Deadline,
        ProjectionError::InvalidQuery { .. } => SearchHold::InvalidQuery,
        _ => SearchHold::Projection,
    })
}

pub(in super::super) fn child(work: &Path) {
    let ctx: PublicContext =
        serde_json::from_slice(&fs::read(work.join("public-test-context.json")).unwrap()).unwrap();
    let ceremony = Ceremony {
        expected: ctx.request,
        kit: ctx.kit,
        trusted_root: VerifyingKey::from_bytes(&ctx.pinned).unwrap(),
        receipt: work.join("first-device.json"),
    };
    let query = SearchQuery {
        text: Some("prefix OR recovered".into()),
        ..SearchQuery::default()
    };
    let case = std::env::var("CRYPTEX_M11_REOPEN_CASE").unwrap();
    if matches!(case.as_str(), "desktop" | "installed") {
        let root = work.join("replacement");
        let store = FileBackedSecureStore::open(work.join("test-credentials")).unwrap();
        let work = work.to_owned();
        crate::ipc::recovered_desktop::run(
            &root,
            std::sync::Arc::new(store),
            std::sync::Arc::new(move |query, deadline| {
                search(&work, &ceremony, query, deadline).map_err(|reason| match reason {
                    SearchHold::Deadline => GuardedSearchError::BrainWideHold(
                        RetrievalSafetyHold::OperationDeadlineExceeded,
                    ),
                    SearchHold::InvalidQuery => {
                        GuardedSearchError::Query(ProjectionError::InvalidQuery {
                            detail: "invalid search query".into(),
                        })
                    }
                    _ => GuardedSearchError::BrainWideHold(
                        RetrievalSafetyHold::CanonicalVerificationUnavailable,
                    ),
                })
            }),
        );
        return;
    }
    if matches!(case.as_str(), "checkpoint-held" | "rebuilt-ready") {
        let root = work.join("rehearsal");
        let key = ceremony.reopen().unwrap().local_trust_anchor().unwrap();
        let (mut db, _) = crate::projection::open_projection_db(&root).unwrap();
        rebuild_knowledge_from_local_approval_history(&root, BRAIN, &key, &mut db).unwrap();
    }
    let result = search(work, &ceremony, &query, None);
    match case.as_str() {
        "ready" | "rebuilt-ready" => {
            let hits = result.unwrap();
            let old_id = std::env::var("CRYPTEX_M11_ARCHIVE_NOTE").unwrap();
            let old = hits.iter().find(|h| h.event_id == old_id).unwrap();
            assert_eq!(old.device_id, ceremony.expected.archive_device.as_str());
            assert_eq!(old.text, "synthetic original prefix");
            let new = hits
                .iter()
                .find(|h| h.event_id == ctx.replacement_note_id)
                .unwrap();
            assert_eq!(new.device_id, ceremony.expected.new_device.as_str());
            assert_eq!(new.text, "First recovered device note");
            assert_eq!(
                search(
                    work,
                    &ceremony,
                    &SearchQuery {
                        limit: Some(1),
                        ..query
                    },
                    None
                )
                .unwrap(),
                hits[..1]
            );
        }
        "history-held" => assert_eq!(result, Err(SearchHold::Archive)),
        "checkpoint-held" => assert_eq!(result, Err(SearchHold::ArchiveCheckpoint)),
        "replacement-held" => assert_eq!(result, Err(SearchHold::Replacement)),
        "approval-held" => assert_eq!(result, Err(SearchHold::Approval)),
        _ => panic!("unknown restart case"),
    }
    println!("m11_restarted_search_child=passed");
}

fn launch(work: &Path, old_id: &str, case: &str) {
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["knowledge_materialization::pairing_recovery_tests::m11_backup_read::restored_note_and_attachment_pass_guarded_reads", "--exact", "--ignored", "--nocapture"])
        .env("CRYPTEX_M11_REOPEN_FIXTURE", work)
        .env("CRYPTEX_M11_REOPEN_CASE", case)
        .env("CRYPTEX_M11_ARCHIVE_NOTE", old_id)
        .output().unwrap();
    assert!(
        output.status.success(),
        "restart {case}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .any(|line| line == "m11_restarted_search_child=passed")
    );
}

pub(in super::super) fn check(work: &Path, old_id: &str, case: &str) {
    launch(work, old_id, case);
    if case != "ready" {
        return;
    }
    if std::env::var_os("CRYPTEX_M11_DESKTOP_BROWSER").is_some() {
        let script = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/desktop/scripts/recovered-search.test.mjs");
        assert!(
            Command::new("node")
                .arg(script)
                .arg(std::env::current_exe().unwrap())
                .env("CRYPTEX_M11_REOPEN_FIXTURE", work)
                .env("CRYPTEX_M11_REOPEN_CASE", "desktop")
                .env("CRYPTEX_M11_ARCHIVE_NOTE", old_id)
                .status()
                .unwrap()
                .success()
        );
        println!(
            "m11_desktop_search=passed reopened=true archived=true replacement=true invalid_receipt=held"
        );
    }
    if let Some(executable) = std::env::var_os("CRYPTEX_M11_INSTALLED_DESKTOP") {
        let script = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/m11-backup-spike/installed_desktop.py");
        assert!(
            Command::new("python")
                .arg("-B")
                .arg(script)
                .arg("check")
                .arg(executable)
                .arg(work)
                .status()
                .unwrap()
                .success()
        );
        println!("m11_installed_search=passed launches=2 archive=true replacement=true");
    }
    let receipt = work.join("first-device.json");
    let original = fs::read(&receipt).unwrap();
    let mut bad: Approval = serde_json::from_slice(&original).unwrap();
    bad.signature[0] ^= 1;
    fs::write(&receipt, serde_json::to_vec(&bad).unwrap()).unwrap();
    launch(work, old_id, "approval-held");
    fs::write(receipt, original).unwrap();
    let packs = canonical_bytes(&work.join("replacement"));
    let (path, bytes) = packs.iter().find(|(_, bytes)| !bytes.is_empty()).unwrap();
    let mut bad = bytes.clone();
    let field = br#""signature":""#;
    let i = bad.windows(field.len()).position(|p| p == field).unwrap() + field.len();
    bad[i] = if bad[i] == b'0' { b'1' } else { b'0' };
    fs::write(path, bad).unwrap();
    launch(work, old_id, "replacement-held");
    fs::write(path, bytes).unwrap();
    // A shorter but internally valid signed history can rebuild its index.
    // Only the offline grant distinguishes it from the selected recovery point.
    for (path, bytes) in canonical_bytes(&work.join("rehearsal")) {
        let (frames, consumed) = cryptex_storage::scan_complete_frames(&bytes).unwrap();
        assert_eq!(bytes.len(), consumed);
        let Some(last) = frames.last() else { continue };
        let wire: serde_json::Value =
            serde_json::from_slice(&last[cryptex_events::FRAME_HEADER_BYTES..]).unwrap();
        if wire["event_type"] != cryptex_events::attachment_link::EVENT_TYPE {
            continue;
        }
        fs::write(&path, &bytes[..consumed - last.len()]).unwrap();
        launch(work, old_id, "checkpoint-held");
        fs::write(path, bytes).unwrap();
        launch(work, old_id, "rebuilt-ready");
        return;
    }
    panic!("synthetic archive must end its originating device stream with the signed link");
}
