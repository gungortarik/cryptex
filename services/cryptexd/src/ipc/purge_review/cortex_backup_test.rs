use super::*;
use crate::backup_inventory::{self, Error};
use crate::knowledge_materialization::backup;
use crate::secure_store::FileBackedSecureStore;
use cryptex_events::{AuthenticatedSecurityRecord, EventPayload};
use std::{fs, process::Command};

#[test]
#[ignore = "requires the disposable M15 importer test binary and OpenPGP fixture"]
fn encrypted_restore_after_cortex_purge() {
    let root = tempfile::Builder::new()
        .prefix("cryptex-ipc-search-physical-m16-")
        .tempdir()
        .unwrap();
    let path = root.path();
    let fixture = Command::new(std::env::var_os("CRYPTEX_M16_IMPORT_FIXTURE").unwrap())
        .args([
            "--exact",
            "cortex_import_trial::purge_backup_fixture::accept_synthetic_export",
            "--ignored",
        ])
        .env("CRYPTEX_M16_TRIAL", path)
        .output()
        .unwrap();
    assert!(
        fixture.status.success() && String::from_utf8_lossy(&fixture.stdout).contains("1 passed"),
        "import fixture failed: {}",
        String::from_utf8_lossy(&fixture.stdout)
    );
    let env = SecureStoreEnvironment::Dev;
    let brain = crate::identity_runtime::load_brain_identity(path)
        .unwrap()
        .brain_id;
    let store = Arc::new(FileBackedSecureStore::open(path.join("test-credentials")).unwrap());
    let writer = LocalDeviceKeyResolver::load(path, &brain, store.as_ref(), env)
        .unwrap()
        .local_trust_anchor()
        .unwrap();
    let originals = crate::canonical_writer::load_verified_events(
        store.as_ref(),
        env,
        path,
        writer.device_id(),
    )
    .unwrap();
    let [target, public, private] = originals.as_slice() else {
        panic!("expected three imported notes")
    };
    let work = tempfile::tempdir().unwrap();
    let kit = work.path().join("offline-kit");
    fs::create_dir(&kit).unwrap();
    let recipient = kit.join("recovery.pub");
    let tools = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/m11-backup-spike");
    assert!(
        Command::new("node")
            .arg(tools.join("envelope.mjs"))
            .arg("keygen")
            .args([&recipient, &kit.join("recovery.sec")])
            .status()
            .unwrap()
            .success()
    );
    let encrypted = work.path().join("after-purge.pgp");
    let selection = {
        crate::instance::migration::migrate_profile(path).unwrap();
        let instance = crate::instance::try_acquire_instance(path).unwrap();
        physical::register(path);
        let mut ctx = crate::ipc::tests::fixture_search_context(path, store.clone());
        ctx.admission = instance.admission();
        ctx.purge_reviews = Some(Arc::default());
        synchronize_local_derived_state(&ctx).unwrap();
        let Outcome::Prepared { ticket, .. } = decide(
            &ctx,
            Action::Prepare {
                note_id: target.event_id.clone(),
                note_hash: target.content_hash.as_str().into(),
            },
        )
        .unwrap() else {
            panic!("owner review unavailable")
        };
        assert!(matches!(
            decide(&ctx, Action::Confirm { ticket }),
            Ok(Outcome::Purged { .. })
        ));
        let prepared = backup::prepare(path, store.as_ref(), env, 1).unwrap();
        let archive = prepared.encryption_input().unwrap();
        assert!(
            Command::new("python")
                .arg("-B")
                .arg(tools.join("daemon_roundtrip.py"))
                .arg("seal-inventory")
                .args([recipient.as_path(), archive.path(), &encrypted])
                .status()
                .unwrap()
                .success()
        );
        prepared.selection
    };
    drop(store);
    root.close().unwrap();
    let decoded = work.path().join("decrypted");
    assert!(
        Command::new("python")
            .arg("-B")
            .arg(tools.join("daemon_roundtrip.py"))
            .arg("open-payload")
            .args([&kit, &encrypted, &decoded])
            .status()
            .unwrap()
            .success()
    );
    let checkpoint = selection.checkpoint(&writer).unwrap();
    assert!(checkpoint.cortex_decision_sha256.is_some());
    let payload = decoded.join("content.zip");
    let restored = backup_inventory::reconstruct_v2(
        &payload,
        &fs::read(decoded.join("inventory.json")).unwrap(),
        &checkpoint,
        work.path(),
    )
    .unwrap();
    let EventPayload::MemoryNote(deleted) = &target.payload;
    let bytes = fs::read(&payload).unwrap(); // All content.zip members use ZIP_STORED.
    assert!(
        !bytes
            .windows(deleted.text.len())
            .any(|w| w == deleted.text.as_bytes())
    );
    {
        let snapshot = backup::snapshot(restored.path(), &brain, &writer).unwrap();
        let chain = snapshot.reader_device_chain(writer.device_id().as_str());
        assert_eq!(chain.len(), 3);
        assert!(
            matches!(&chain[0], AuthenticatedSecurityRecord::PurgeBridge(bridge, _, hash, _)
            if bridge.event_id == target.event_id && hash == &target.content_hash)
        );
        for (record, expected) in chain[1..].iter().zip([public, private]) {
            assert!(matches!(record, AuthenticatedSecurityRecord::V1(event) if event == expected));
        }
    }
    let proof = restored
        .path()
        .join("canonical/purge-approvals")
        .join(format!("{}.json", target.event_id));
    let original = fs::read(&proof).unwrap();
    fs::remove_file(&proof).unwrap();
    assert_eq!(
        backup_inventory::verify_reconstruction(restored.path(), &checkpoint),
        Err(Error::PurgeHistory)
    );
    fs::write(proof, original).unwrap();
    let source = restored
        .path()
        .join(crate::cortex_evidence::DIRECTORY)
        .join(format!("{}.json", target.event_id));
    // A valid source receipt from the same accepted batch cannot stand in for
    // the receipt bound to the deleted event.
    fs::copy(
        source.with_file_name(format!("{}.json", public.event_id)),
        &source,
    )
    .unwrap();
    assert_eq!(
        backup_inventory::verify_reconstruction(restored.path(), &checkpoint),
        Err(Error::CortexEvidence)
    );
    fs::remove_file(source).unwrap();
    assert_eq!(
        backup_inventory::verify_reconstruction(restored.path(), &checkpoint),
        Err(Error::CortexEvidence)
    );
}
