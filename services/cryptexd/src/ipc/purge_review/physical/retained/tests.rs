use super::*;
use crate::canonical_writer::{AppendMemoryNoteArgs, CanonicalWriterConfig, append_memory_note};
use crate::ipc::{
    purge_review::{Action, Outcome, decide, physical, receipt},
    synchronize_local_derived_state,
    tests::prepared_search_fixture,
};
use crate::local_device_identity::LocalDeviceKeyResolver;
use crate::secure_store::SecureStoreEnvironment;
use cryptex_events::TrustedDaemonKeyResolver;
use cryptex_events::{Classification, FRAME_HEADER_BYTES, VersionedRecord, read_versioned_record};
use std::{fs, sync::Arc};

#[test]
#[ignore = "launched in a separate process with disposable profile and backup keys"]
fn purge_independent_encrypted_backups() {
    let mut fixture = prepared_search_fixture("physical-retained-backups");
    let root = &fixture.root;
    let device = &fixture.device;
    let env = SecureStoreEnvironment::Dev;
    let store = fixture.store.as_ref();
    drop(crate::instance::try_acquire_instance(root).unwrap());
    crate::instance::migration::migrate_profile(root).unwrap();
    let instance = crate::instance::try_acquire_instance(root).unwrap();
    physical::register(root);
    let ctx = &mut fixture.context;
    ctx.admission = instance.admission();
    ctx.purge_reviews = Some(Arc::default());
    let add = |text, threshold| {
        append_memory_note(
            store,
            env,
            AppendMemoryNoteArgs {
                runtime_root: root,
                brain_id: &fixture.brain_id,
                device_id: device,
                classification: Classification::Internal,
                text,
                event_id: None,
                config: &CanonicalWriterConfig {
                    seal_event_threshold: threshold,
                },
            },
        )
        .unwrap()
    };
    let first = add("unchanged note in both generations", 100);
    let index = first.segment_index;
    let open = cryptex_storage::active_segment_path(root, device, index);
    let before = fs::read(&open).unwrap();
    let secret = "unique retained backup target bfa6913";
    let target = add(secret, 100).accepted;
    let early = fs::read(open).unwrap();
    let last = add("note added only in the later generation", 1);
    assert!(last.sealed && last.segment_index == index);
    let sealed = cryptex_storage::sealed_segment_path(root, device, index);
    let full = fs::read(&sealed).unwrap();
    synchronize_local_derived_state(ctx).unwrap();

    let key = Identity::generate();
    let base = crate::test_temp::exclusive_temp_dir("cryptex-detached-backups");
    assert_ne!(&base, root);
    let parent = Directory::open(&base).unwrap();
    let mut targets = Vec::new();
    for (name, pack) in [("later", &full), ("earlier", &early), ("before", &before)] {
        let directory = parent.create_child_exclusive(name).unwrap();
        let mut file = directory.open_file(PACK, true).unwrap();
        write_image(
            &mut file,
            &key,
            &PackImage {
                pack: pack.clone(),
                approvals: BTreeMap::new(),
            },
        )
        .unwrap();
        directory.sync().unwrap();
        targets.push(Target {
            directory,
            device: device.clone(),
            index,
        });
    }
    drop(parent);
    *TRIAL.lock().unwrap() = Some(Trial {
        source: root.clone(),
        key: key.clone(),
        targets,
    });
    let path = |name: &str| base.join(name).join(PACK);
    let early_cipher = fs::read(path("earlier")).unwrap();
    let late_cipher = fs::read(path("later")).unwrap();
    let untouched = fs::read(path("before")).unwrap();
    let load = |name| read_image(&File::open(path(name)).unwrap(), &key).unwrap();
    assert_eq!(load("earlier").pack, early);
    assert_eq!(load("later").pack, full);
    let prepare = || {
        let Outcome::Prepared {
            ticket,
            remove_locally: true,
        } = decide(
            ctx,
            Action::Prepare {
                note_id: target.event_id.clone(),
                note_hash: target.content_hash.as_str().into(),
            },
        )
        .unwrap()
        else {
            panic!("expected deletion review")
        };
        ticket
    };
    assert!(matches!(
        decide(ctx, Action::Cancel { ticket: prepare() }).unwrap(),
        Outcome::Cancelled
    ));
    assert_eq!(fs::read(path("earlier")).unwrap(), early_cipher);
    assert_eq!(fs::read(path("later")).unwrap(), late_cipher);
    let busy = Directory::open(&base.join("later"))
        .unwrap()
        .lock_exclusive("writer.lock")
        .unwrap();
    assert!(matches!(
        decide(ctx, Action::Confirm { ticket: prepare() }),
        Ok(Outcome::CleanupIncomplete { note_id, note_hash, reason: ReviewError::CleanupPending })
            if note_id == target.event_id && note_hash == target.content_hash.as_str()
    ));
    assert_ne!(fs::read(path("earlier")).unwrap(), early_cipher);
    assert_eq!(fs::read(path("later")).unwrap(), late_cipher);
    drop(busy);
    let resolver = LocalDeviceKeyResolver::load(root, &fixture.brain_id, store, env).unwrap();
    let anchor = resolver.local_trust_anchor().unwrap();
    let retry = || physical::finish_cleanup(root, &fixture.brain_id, &anchor);
    let mut damaged = late_cipher.clone();
    damaged.pop();
    fs::write(path("later"), &damaged).unwrap();
    assert!(matches!(retry(), Err(ReviewError::PackIntegrity)));
    assert_eq!(fs::read(path("later")).unwrap(), damaged);
    // Resume at the boundary after claiming the original but before publication.
    fs::write(path("later"), late_cipher).unwrap();
    fs::rename(path("later"), base.join("later").join(ORIGINAL)).unwrap();
    retry().unwrap();
    let later_clean = fs::read(path("later")).unwrap();
    retry().unwrap();
    assert_eq!(fs::read(path("later")).unwrap(), later_clean);
    assert_eq!(fs::read(path("before")).unwrap(), untouched);
    let clean = fs::read(&sealed).unwrap();
    let tail = &full[early.len()..];
    assert!(!tail.is_empty() && clean.starts_with(&before) && clean.ends_with(tail));
    let public = ed25519_dalek::VerifyingKey::from_bytes(anchor.public_key()).unwrap();
    let expected_approval = fs::read(
        root.join("canonical")
            .join(receipt::FOLDER)
            .join(format!("{}.json", target.event_id)),
    )
    .unwrap();
    for (name, expected, original) in [
        ("earlier", &clean[..clean.len() - tail.len()], &early),
        ("later", clean.as_slice(), &full),
    ] {
        let restored = load(name);
        assert_eq!(restored.pack, expected);
        assert!(
            !restored
                .pack
                .windows(secret.len())
                .any(|w| w == secret.as_bytes())
        );
        assert_eq!(
            restored.approvals,
            BTreeMap::from([(target.event_id.clone(), expected_approval.clone())])
        );
        let (old, _) = cryptex_storage::scan_complete_frames(original).unwrap();
        let (new, _) = cryptex_storage::scan_complete_frames(&restored.pack).unwrap();
        assert_eq!(old.len(), new.len());
        for (old, new) in old.iter().zip(new) {
            let VersionedRecord::VerifiedV1(note) =
                read_versioned_record(&old[FRAME_HEADER_BYTES..], &public).unwrap()
            else {
                panic!("expected original note")
            };
            if note.event_id != target.event_id {
                assert_eq!(*old, new);
                assert!(matches!(
                    read_versioned_record(&new[FRAME_HEADER_BYTES..], &public),
                    Ok(VersionedRecord::VerifiedV1(_))
                ));
            }
        }
        assert!(!base.join(name).join(ORIGINAL).try_exists().unwrap());
        assert!(
            !base
                .join(name)
                .join("rewrite")
                .join(ORIGINAL)
                .try_exists()
                .unwrap()
        );
    }
    drop(TRIAL.lock().unwrap().take());
    drop(instance);
    for path in [&base, root] {
        assert_eq!(path.parent(), Some(std::env::temp_dir().as_path()));
        fs::remove_dir_all(path).unwrap();
    }
    println!("retained_encrypted_purge=verified cleaned=2 untouched=1 authenticated_restore=true");
}
