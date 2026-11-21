use super::*;
use crate::backup_inventory::{self, Error};
use crate::canonical_writer::{AppendMemoryNoteArgs, append_memory_note};
use crate::ipc::tests::prepared_search_fixture;
use crate::knowledge_materialization::backup;
use cryptex_events::Classification;
use std::{fs, io::Read};

#[test]
#[ignore = "isolated native process with a disposable purge profile"]
fn verified_backup_preserves_purge_and_unaffected_notes() {
    let mut fixture = prepared_search_fixture("physical-m11-backup");
    let root = &fixture.root;
    let env = SecureStoreEnvironment::Dev;
    let store = fixture.store.as_ref();
    drop(crate::instance::try_acquire_instance(root).unwrap());
    crate::instance::migration::migrate_profile(root).unwrap();
    let instance = crate::instance::try_acquire_instance(root).unwrap();
    physical::register(root);
    let ctx = &mut fixture.context;
    ctx.admission = instance.admission();
    ctx.purge_reviews = Some(Arc::default());
    let add = |text| {
        append_memory_note(
            store,
            env,
            AppendMemoryNoteArgs {
                runtime_root: root,
                brain_id: &fixture.brain_id,
                device_id: &fixture.device,
                classification: Classification::Internal,
                text,
                event_id: None,
                config: &CanonicalWriterConfig::default(),
            },
        )
        .unwrap()
        .accepted
    };
    let secret = "discarded backup test body 64d9221";
    let target = add(secret);
    let remaining = "surviving backup test body 118bf11";
    add(remaining);
    synchronize_local_derived_state(ctx).unwrap();
    let Outcome::Prepared { ticket, .. } = decide(
        ctx,
        Action::Prepare {
            note_id: target.event_id.clone(),
            note_hash: target.content_hash.as_str().into(),
        },
    )
    .unwrap() else {
        panic!("missing owner review")
    };
    assert!(matches!(
        decide(ctx, Action::Confirm { ticket }),
        Ok(Outcome::Purged { .. })
    ));
    let writer = LocalDeviceKeyResolver::load(root, &fixture.brain_id, store, env)
        .unwrap()
        .local_trust_anchor()
        .unwrap();
    let prepared = backup::prepare(root, store, env, 1).unwrap();
    let checkpoint = prepared.selection.checkpoint(&writer).unwrap();
    let outer = prepared.encryption_input().unwrap();
    let mut envelope = zip::ZipArchive::new(outer.reopen().unwrap()).unwrap();
    let work = tempfile::tempdir().unwrap();
    let payload = work.path().join("content.zip");
    std::io::copy(
        &mut envelope.by_name("content.zip").unwrap(),
        &mut fs::File::create(&payload).unwrap(),
    )
    .unwrap();
    let mut signature = vec![];
    envelope
        .by_name("inventory.json")
        .unwrap()
        .read_to_end(&mut signature)
        .unwrap();
    let restored =
        backup_inventory::reconstruct_v2(&payload, &signature, &checkpoint, work.path()).unwrap();
    let pack = fs::read(&payload).unwrap(); // ZIP_STORED
    assert!(!pack.windows(secret.len()).any(|w| w == secret.as_bytes()));
    assert!(
        pack.windows(remaining.len())
            .any(|w| w == remaining.as_bytes())
    );
    let relative = std::path::PathBuf::from("canonical/purge-approvals")
        .join(format!("{}.json", target.event_id));
    let file = restored.path().join(&relative);
    let proof = fs::read(root.join(&relative)).unwrap();
    assert_eq!(fs::read(&file).unwrap(), proof);
    let mut forged: serde_json::Value = serde_json::from_slice(&proof).unwrap();
    forged["signature"] = "00".repeat(64).into();
    fs::write(&file, serde_json_canonicalizer::to_vec(&forged).unwrap()).unwrap();
    assert_eq!(
        backup_inventory::verify_reconstruction(restored.path(), &checkpoint),
        Err(Error::PurgeHistory)
    );
    fs::remove_file(&file).unwrap();
    assert_eq!(
        backup_inventory::verify_reconstruction(restored.path(), &checkpoint),
        Err(Error::PurgeHistory)
    );
    fs::write(file, proof).unwrap();
    backup_inventory::verify_reconstruction(restored.path(), &checkpoint).unwrap();
}
