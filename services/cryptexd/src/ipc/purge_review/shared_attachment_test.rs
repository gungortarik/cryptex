use super::*;
use crate::canonical_writer::{AppendMemoryNoteArgs, append_memory_note};
use crate::ipc::{attachment, tests::prepared_search_fixture};
use crate::knowledge_materialization::{
    evaluate_retrieval_safety, scan_verified_canonical_from_local_anchor_until,
};
use cryptex_events::{AcceptedEvent, AuthenticatedSecurityRecord, Classification};
use sha2::{Digest, Sha256};

#[test]
#[ignore = "launched in a separate process for its disposable native profile"]
fn purge_shared_attachment() {
    let mut fixture = prepared_search_fixture("physical-shared-attachment");
    let root = &fixture.root;
    let env = SecureStoreEnvironment::Dev;
    let store = fixture.store.as_ref();
    drop(crate::instance::try_acquire_instance(root).unwrap());
    crate::instance::migration::migrate_profile(root).unwrap();
    let instance = crate::instance::try_acquire_instance(root).unwrap();
    physical::register(root);
    let ctx = &mut fixture.context;
    ctx.admission = instance.admission();
    ctx.isolated_attachment_writer = true;
    ctx.purge_reviews = Some(Arc::default());
    let mut resolver = LocalDeviceKeyResolver::load(root, &fixture.brain_id, store, env).unwrap();
    let anchor = resolver.local_trust_anchor().unwrap();
    let add_note = |text, classification| {
        append_memory_note(
            store,
            env,
            AppendMemoryNoteArgs {
                runtime_root: root,
                brain_id: &fixture.brain_id,
                device_id: &fixture.device,
                classification,
                text,
                event_id: None,
                config: &CanonicalWriterConfig::default(),
            },
        )
        .unwrap()
        .accepted
    };
    let first = add_note("first shared attachment note", Classification::Public);
    let second = add_note("second shared attachment note", Classification::Restricted);
    let control = add_note("independent attachment note", Classification::Internal);
    let shared = b"\0shared-file-trial-x7b1\xff";
    let separate = b"unrelated-control-file-x7b2";
    let blobs = root.join("canonical/blobs");
    let path = |bytes: &[u8]| blobs.join(format!("{:x}", Sha256::digest(bytes)));
    let attach = |note: &AcceptedEvent, bytes: &[u8]| {
        let frame = serde_json::to_vec(&serde_json::json!({
            "protocol_version": PROTOCOL_VERSION, "request_id": new_bootstrap_event_id().unwrap(),
            "client_role":"desktop", "capabilities":["content.attach"], "operation":"attachment.add",
            "body":{"noteId":note.event_id, "noteHash":note.content_hash, "bytes":bytes}
        })).unwrap();
        let reply =
            serde_json::to_value(attachment::handle(&frame, Some(ctx), true, None)).unwrap();
        assert_eq!(reply["result"], serde_json::json!({"Ok":null}));
        let snap =
            scan_verified_canonical_from_local_anchor_until(root, &fixture.brain_id, &anchor, None)
                .unwrap();
        snap.reader_device_chain(fixture.device.as_str())
            .iter()
            .find_map(|record| match record {
                AuthenticatedSecurityRecord::Attachment(record, fields)
                    if fields.note_event_id() == note.event_id =>
                {
                    Some(record.event_id.clone())
                }
                _ => None,
            })
            .unwrap()
    };
    let first_link = attach(&first, shared);
    let second_link = attach(&second, shared);
    let control_link = attach(&control, separate);
    assert_ne!(first_link, second_link);
    assert_eq!(std::fs::read_dir(&blobs).unwrap().count(), 2);
    synchronize_local_derived_state(ctx).unwrap();

    let inspect = |resolver: &mut LocalDeviceKeyResolver,
                   note: &AcceptedEvent,
                   link: &str,
                   expected: Option<&[u8]>| {
        let db = ProjectionDb::open_or_create(root).unwrap();
        let gate = evaluate_retrieval_safety(
            root,
            &fixture.brain_id,
            std::slice::from_ref(&fixture.device),
            resolver,
            &db,
        );
        let result = gate.open_local_attachment(&note.event_id, link);
        if let Some(bytes) = expected {
            let mut actual = Vec::new();
            result.unwrap().read_to_end(&mut actual).unwrap();
            assert_eq!(actual, bytes);
        } else {
            assert!(matches!(
                result,
                Err(crate::attachment_blobs::LocalLinkError::Reference(
                    cryptex_events::attachment_link::LinkError::MissingLink
                ))
            ));
        }
    };
    let purge = |ctx: &SearchContext, note: &AcceptedEvent| {
        let Outcome::Prepared {
            ticket,
            remove_locally: true,
        } = decide(
            ctx,
            Action::Prepare {
                note_id: note.event_id.clone(),
                note_hash: note.content_hash.as_str().into(),
            },
        )
        .unwrap()
        else {
            panic!("expected deletion review");
        };
        assert!(matches!(
            decide(ctx, Action::Confirm { ticket }).unwrap(),
            Outcome::Purged { .. }
        ));
    };
    inspect(&mut resolver, &first, &first_link, Some(shared));
    inspect(&mut resolver, &second, &second_link, Some(shared));
    purge(ctx, &first);
    assert_eq!(std::fs::read(path(shared)).unwrap(), shared);
    inspect(&mut resolver, &first, &first_link, None);
    inspect(&mut resolver, &second, &second_link, Some(shared));

    drop(instance);
    let reopened = crate::instance::try_acquire_instance(root).unwrap();
    ctx.admission = reopened.admission();
    purge(ctx, &second);
    assert!(!path(shared).try_exists().unwrap());
    assert_eq!(std::fs::read_dir(&blobs).unwrap().count(), 1);
    physical::finish_cleanup(root, &fixture.brain_id, &anchor).unwrap();
    drop(reopened);
    let reopened = crate::instance::try_acquire_instance(root).unwrap();
    inspect(&mut resolver, &second, &second_link, None);
    inspect(&mut resolver, &control, &control_link, Some(separate));
    assert!(!path(shared).try_exists().unwrap());
    drop(reopened);
    fn contains_bytes(dir: &std::path::Path, needle: &[u8]) -> bool {
        std::fs::read_dir(dir).unwrap().any(|entry| {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                contains_bytes(&entry.path(), needle)
            } else {
                assert!(entry.file_type().unwrap().is_file());
                std::fs::read(entry.path())
                    .unwrap()
                    .windows(needle.len())
                    .any(|part| part == needle)
            }
        })
    }
    assert!(!contains_bytes(root, shared));
    assert!(contains_bytes(root, separate));
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    std::fs::remove_dir_all(root).unwrap();
    println!("shared_purge=verified first_preserved=true last_removed=true control_preserved=true");
}

#[test]
#[ignore = "launched in a separate process for its disposable native profile"]
fn review_mixed_history_purge() {
    let mut fixture = prepared_search_fixture("physical-closeout-mixed");
    let root = &fixture.root;
    let env = SecureStoreEnvironment::Dev;
    let store = fixture.store.as_ref();
    drop(crate::instance::try_acquire_instance(root).unwrap());
    crate::instance::migration::migrate_profile(root).unwrap();
    let instance = crate::instance::try_acquire_instance(root).unwrap();
    physical::register(root);
    let ctx = &mut fixture.context;
    ctx.admission = instance.admission();
    ctx.isolated_attachment_writer = true;
    ctx.purge_reviews = Some(Arc::default());
    let mut resolver = LocalDeviceKeyResolver::load(root, &fixture.brain_id, store, env).unwrap();
    let anchor = resolver.local_trust_anchor().unwrap();
    let add_note = |text, classification| {
        append_memory_note(
            store,
            env,
            AppendMemoryNoteArgs {
                runtime_root: root,
                brain_id: &fixture.brain_id,
                device_id: &fixture.device,
                classification,
                text,
                event_id: None,
                config: &CanonicalWriterConfig::default(),
            },
        )
        .unwrap()
        .accepted
    };
    let forgotten = add_note("old review fixture", Classification::Public);
    let target = add_note("unrelated visible review fixture", Classification::Public);
    synchronize_local_derived_state(ctx).unwrap();
    {
        let snapshot = crate::knowledge_materialization::lock_local_lifecycle_history(
            root,
            &fixture.brain_id,
            &anchor,
            Instant::now() + Duration::from_secs(10),
        )
        .unwrap();
        let key = crate::event_signing_key::load_existing_event_signing_key(
            store,
            env,
            root,
            &fixture.device,
        )
        .unwrap();
        crate::canonical_writer::isolated_attachment::append_note_control(root,&fixture.device,&forgotten,&key,"memory.forget",
            serde_json::json!({"note_event_id":forgotten.event_id,"note_content_hash":forgotten.content_hash,"prior_state_hash":forgotten.content_hash})).unwrap();
        drop(snapshot);
    }
    let prepared = decide(
        ctx,
        Action::Prepare {
            note_id: target.event_id.clone(),
            note_hash: target.content_hash.as_str().into(),
        },
    )
    .unwrap();
    let Outcome::Prepared {
        ticket,
        remove_locally: true,
    } = prepared
    else {
        panic!("expected physical review")
    };
    let result = decide(ctx, Action::Confirm { ticket }).unwrap();
    println!("closeout_probe outcome={result:?}");
    assert!(
        matches!(result, Outcome::Purged { .. }),
        "confirmed deletion of unrelated visible note must finish"
    );
}
