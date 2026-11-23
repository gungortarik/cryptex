//! Populate the other test process's disposable Brain through the M15 importer.
use super::*;
use cryptexd::secure_store::FileBackedSecureStore;

#[test]
#[ignore = "called by the M16 post-purge encrypted recovery rehearsal"]
fn accept_synthetic_export() {
    let root = std::path::PathBuf::from(std::env::var_os("CRYPTEX_M16_TRIAL").unwrap());
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("cryptex-ipc-search-physical-m16-")
    );
    assert_eq!(root.read_dir().unwrap().count(), 0);
    let brain = random_uuid_v4().unwrap();
    let store = FileBackedSecureStore::open(root.join("test-credentials")).unwrap();
    let device = initialize(&root, &store, brain.clone()).unwrap();
    let _instance = cryptexd::instance::try_acquire_instance(&root).unwrap();
    let export = tempfile::tempdir().unwrap();
    fs::create_dir(export.path().join("Notes")).unwrap();
    let mut batch = provenance::Batch::create(&root, &device, &store).unwrap();
    let mut selected = vec![];
    for (name, text, class) in [
        ("a", "M16 discarded Cortex body", "internal"),
        ("b", "M16 surviving public body", "public"),
        ("c", "M16 surviving private body", "private"),
    ] {
        let path = format!("Notes/{name}.md");
        fs::write(export.path().join(&path), text).unwrap();
        let approval = json!({"capture_id": random_uuid_v4().unwrap(), "source_path": path,
            "source_sha256": format!("{:x}", Sha256::digest(text)), "classification": class});
        let reply = capture(
            export.path(),
            &root,
            &store,
            &serde_json::to_vec(&approval).unwrap(),
            TrialMode::Batch,
            Some(&mut batch),
        )
        .unwrap();
        assert_eq!(reply["status"], "committed");
        selected.push(approval);
    }
    export.close().unwrap();
    let key =
        cryptexd::event_signing_key::load_existing_event_signing_key(&store, DEV, &root, &device)
            .unwrap()
            .signing_key;
    let sources =
        provenance::verified_sources(&root, &device, &store, &key.verifying_key()).unwrap();
    let selection =
        json!({"schema":"cryptex.cortex.selection.v1", "brain_id":brain, "selected":selected});
    let comparison =
        selection::Selection::read(serde_json::to_vec(&selection).unwrap().as_slice(), &brain)
            .unwrap()
            .compare(sources)
            .unwrap();
    let digest = comparison.review_sha256.clone().unwrap();
    let report = decision::review(
        &root,
        &key,
        &device,
        &brain,
        comparison,
        Some((decision::Choice::Accepted, &digest)),
    )
    .unwrap();
    assert_eq!(report["batch_decision"]["choice"], "accepted");
}
