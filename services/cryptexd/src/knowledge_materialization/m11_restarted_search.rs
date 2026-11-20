//! Read-only union of the recovered snapshot and the first replacement's notes.
//! Confined to disposable M11 rehearsals.
use super::*;
use crate::secure_store::FileBackedSecureStore;
use serde::{Deserialize, Serialize};

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

pub(in super::super) fn child(work: &Path) {
    let ctx: PublicContext =
        serde_json::from_slice(&fs::read(work.join("public-test-context.json")).unwrap()).unwrap();
    let ceremony = Ceremony {
        expected: ctx.request,
        kit: ctx.kit,
        trusted_root: VerifyingKey::from_bytes(&ctx.pinned).unwrap(),
        receipt: work.join("replacement/recovery/first-device.json"),
    };
    let query = SearchQuery {
        text: Some("prefix OR recovered".into()),
        ..SearchQuery::default()
    };
    let case = std::env::var("CRYPTEX_M11_REOPEN_CASE").unwrap();
    let root = work.join("replacement");
    let store = FileBackedSecureStore::open(work.join("test-credentials")).unwrap();
    if case == "desktop" {
        crate::ipc::recovered_desktop::run_fixture(&root, std::sync::Arc::new(store));
        return;
    }
    let mut resolver =
        LocalDeviceKeyResolver::load(&root, BRAIN, &store, SecureStoreEnvironment::Dev).unwrap();
    let bound = for_profile(
        &root,
        BRAIN,
        &ceremony.expected.new_device,
        &store,
        SecureStoreEnvironment::Dev,
    );
    if case == "missing-binding" {
        assert!(matches!(bound, Err(SearchHold::Approval)));
        println!("m11_restarted_search_child=passed");
        return;
    }
    let bound = bound.unwrap().unwrap();
    if matches!(case.as_str(), "checkpoint-held" | "rebuilt-ready") {
        let root = work.join("replacement/recovery/archive");
        let key = ceremony.reopen().unwrap().local_trust_anchor().unwrap();
        let (mut db, _) = crate::projection::open_projection_db(&root).unwrap();
        rebuild_knowledge_from_local_approval_history(&root, BRAIN, &key, &mut db).unwrap();
    }
    let result = search(&root, &bound, &mut resolver, &query, None);
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
                    &root,
                    &bound,
                    &mut resolver,
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
        let archive = work.join("replacement/recovery/archive");
        let root = work.join("replacement");
        let old_packs = canonical_bytes(&archive);
        let new_packs = canonical_bytes(&root);
        assert!(!old_packs.is_empty() && !new_packs.is_empty());
        // The libtest oracle belongs only to the fixture generator.
        let oracle = work.join("public-test-context.json");
        let saved_oracle = fs::read(&oracle).unwrap();
        fs::remove_file(&oracle).unwrap();
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
        assert_eq!(canonical_bytes(&archive), old_packs);
        assert_eq!(canonical_bytes(&root), new_packs);
        fs::write(oracle, saved_oracle).unwrap();
        println!(
            "m11_installed_search=passed launches=2 archive=true replacement=true reader=cryptexd canonical=unchanged"
        );
    }
    let root = work.join("replacement");
    let identity = crate::local_device_identity::load_local_device_identity(&root, BRAIN).unwrap();
    let store = FileBackedSecureStore::open(work.join("test-credentials")).unwrap();
    let service =
        crate::secure_store::ServiceLabel::device_credentials(SecureStoreEnvironment::Dev);
    let account = crate::secure_store::AccountLabel::recovery_search(&identity.device_id);
    let binding = store.get(service.as_str(), account.as_str()).unwrap();
    store.delete(service.as_str(), account.as_str()).unwrap();
    launch(work, old_id, "missing-binding");
    store
        .put(service.as_str(), account.as_str(), &binding)
        .unwrap();
    let receipt = work.join("replacement/recovery/first-device.json");
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
    for (path, bytes) in canonical_bytes(&work.join("replacement/recovery/archive")) {
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
