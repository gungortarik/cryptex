use super::*;
use crate::backup_inventory::{self, Error};
use crate::canonical_writer::{AppendMemoryNoteArgs, append_memory_note};
use crate::ipc::tests::prepared_search_fixture;
use crate::knowledge_materialization::backup;
use cryptex_events::Classification;
use sha2::Digest;
use std::{fs, io::Read};

#[test]
#[ignore = "isolated native process with a disposable purge profile"]
fn verified_backup_preserves_purge_and_unaffected_notes() {
    let browser_server =
        std::env::var("CRYPTEX_FILE_FIXTURE_MODE").as_deref() == Ok("backup-removal");
    if !browser_server && std::env::var("CRYPTEX_M11_DESKTOP_BROWSER").as_deref() == Ok("1") {
        assert!(
            std::process::Command::new("node")
                .arg(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("../../apps/desktop/scripts/backup-removal.test.mjs")
                )
                .arg(std::env::current_exe().unwrap())
                .status()
                .unwrap()
                .success()
        );
        println!(
            "m11_backup_removal_ui=passed cancelled=kept confirmed=removed replay=denied replacement=restored"
        );
        return;
    }
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
    let survivor = add(remaining);
    let attachment = b"surviving attachment after the purge";
    crate::canonical_writer::isolated_attachment::append(
        root,
        store,
        env,
        &crate::ipc::attachment::SelectedFile {
            note_id: survivor.event_id,
            note_hash: survivor.content_hash.as_str().into(),
            bytes: attachment.to_vec(),
        },
    )
    .unwrap();
    synchronize_local_derived_state(ctx).unwrap();
    let writer = LocalDeviceKeyResolver::load(root, &fixture.brain_id, store, env)
        .unwrap()
        .local_trust_anchor()
        .unwrap();
    let work = tempfile::tempdir().unwrap();
    let kit = work.path().join("kit");
    fs::create_dir(&kit).unwrap();
    let public = kit.join("recovery.pub");
    let tools =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/m11-backup-spike");
    assert!(
        std::process::Command::new("node")
            .arg(tools.join("envelope.mjs"))
            .arg("keygen")
            .arg(&public)
            .arg(kit.join("recovery.sec"))
            .status()
            .unwrap()
            .success()
    );
    let codec = |op: &str, args: &[&std::path::Path]| -> std::io::Result<()> {
        let result = std::process::Command::new("python")
            .arg("-B")
            .arg(tools.join("daemon_roundtrip.py"))
            .arg(op)
            .args(args)
            .output()?;
        if result.status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other("fixture codec unavailable"))
        }
    };
    let role = backup::role::DeviceRole {
        store,
        env,
        local: &writer,
    };
    let (volume, _) = role.create(root, work.path()).unwrap();
    let first = volume.join("backup-00000000000000000001.pgp");
    let second = volume.join("backup-00000000000000000002.pgp");
    let old = backup::prepare(root, store, env, 1).unwrap();
    codec(
        "seal-inventory",
        &[&public, old.encryption_input().unwrap().path(), &first],
    )
    .unwrap();
    let encrypted_old = fs::read(&first).unwrap();
    let jobs = crate::backup_scheduler::journal::DeviceJobs::new(store, env, &writer);
    jobs.enqueue(&first, sha2::Sha256::digest(&encrypted_old).into(), 2)
        .unwrap();
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
    let prepared = backup::prepare(root, store, env, 2).unwrap();
    let checkpoint = prepared.selection.checkpoint(&writer).unwrap();
    let outer = prepared.encryption_input().unwrap();
    let mut envelope = zip::ZipArchive::new(outer.reopen().unwrap()).unwrap();
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
    let blobs = restored.path().join("canonical/blobs");
    fs::create_dir_all(&blobs).unwrap();
    let orphan = blobs.join(format!("{:x}", sha2::Sha256::digest(secret.as_bytes())));
    fs::write(&orphan, secret).unwrap();
    assert_eq!(
        backup_inventory::verify_reconstruction(restored.path(), &checkpoint),
        Err(Error::Attachment)
    );
    fs::remove_file(orphan).unwrap();
    use crate::backup_scheduler::retention::{VerifiedBackups, removal::Decision};
    let (_keep, stop) = std::sync::mpsc::channel();
    jobs.resume(
        &writer,
        &stop,
        || panic!("target offline"),
        |file| codec("seal-inventory", &[&public, outer.path(), file]),
    )
    .unwrap();
    let catalog = VerifiedBackups {
        store,
        environment: env,
        designated: &writer,
        target: &volume,
        writer_profile: root,
    };
    let restore = |number: u64, file: &std::path::Path| -> std::io::Result<tempfile::TempDir> {
        let private = tempfile::tempdir_in(work.path())?;
        let decoded = private.path().join("decrypted");
        codec("open-payload", &[&kit, file, &decoded])?;
        let selection = if number == 1 {
            &old.selection
        } else {
            &prepared.selection
        };
        backup_inventory::reconstruct_v2(
            &decoded.join("content.zip"),
            &fs::read(decoded.join("inventory.json"))?,
            &selection.checkpoint(&writer).unwrap(),
            work.path(),
        )
        .map_err(|_| std::io::Error::other("invalid backup"))
    };
    let [prior, latest] = jobs.published_pair().unwrap();
    for (number, publication) in [(1, &prior), (2, &latest)] {
        catalog
            .verify(&writer, publication, |file| {
                let copy = restore(number, file)?;
                let at = copy.path().to_owned();
                Ok((copy, at))
            })
            .unwrap();
    }
    catalog
        .set_retention(&writer, std::num::NonZeroUsize::new(1))
        .unwrap();
    assert_eq!(catalog.prune(&writer).unwrap(), 0);
    let review = || catalog.review_removal(&writer, 1, 2, restore);
    fs::rename(&kit, work.path().join("offline-kit")).unwrap();
    assert!(review().is_err());
    assert_eq!(fs::read(&first).unwrap(), encrypted_old);
    fs::rename(work.path().join("offline-kit"), &kit).unwrap();
    assert!(
        !catalog
            .decide_removal(&writer, review().unwrap(), Decision::Cancel)
            .unwrap()
    );
    assert_eq!(fs::read(&first).unwrap(), encrypted_old);
    let wrong_selection = review().unwrap();
    assert_eq!(wrong_selection.selected_file(), first);
    assert!(
        catalog
            .decide_removal(&writer, wrong_selection, Decision::Confirm(second.clone()))
            .is_err()
    );
    assert_eq!(fs::read(&first).unwrap(), encrypted_old);
    let stale = review().unwrap();
    let encrypted_new = fs::read(&second).unwrap();
    fs::write(&second, b"changed after review").unwrap();
    assert!(
        catalog
            .decide_removal(&writer, stale, Decision::Confirm(first.clone()))
            .is_err()
    );
    assert_eq!(fs::read(&first).unwrap(), encrypted_old);
    fs::write(&second, &encrypted_new).unwrap();
    if let Some(installed_codec) = std::env::var_os("CRYPTEX_M11_TEST_INSTALLED_CODEC") {
        use ed25519_dalek::pkcs8::EncodePrivateKey;
        let sealed = work.path().join("sealed-kit.pgp");
        let unlock = work.path().join("unlock");
        let trusted = work.path().join("checkpoints");
        fs::create_dir(&trusted).unwrap();
        let older_checkpoint = trusted.join("older.json");
        let newer_checkpoint = trusted.join("newer.json");
        for (file, selection) in [
            (&older_checkpoint, &old.selection),
            (&newer_checkpoint, &prepared.selection),
        ] {
            fs::write(file, serde_json::to_vec(selection).unwrap()).unwrap();
        }
        fs::write(
            kit.join("brain-identity.sha256"),
            checkpoint.brain_identity_sha256,
        )
        .unwrap();
        fs::copy(&public, kit.join("writer.pub")).unwrap();
        let signing = ed25519_dalek::SigningKey::from_bytes(&[43; 32]);
        fs::write(
            kit.join("bootstrap.pk8"),
            signing.to_pkcs8_der().unwrap().as_bytes(),
        )
        .unwrap();
        fs::write(&unlock, "ac".repeat(32)).unwrap();
        codec("seal-kit", &[&kit, &unlock, &sealed]).unwrap();
        ctx.backup_export = Some(Arc::new(backup_export::Config {
            codec: installed_codec.into(),
            busy: Mutex::default(),
        }));
        let prepare = |check: &Path| {
            let frame = serde_json::json!({
                "protocol_version": PROTOCOL_VERSION, "request_id": new_bootstrap_event_id().unwrap(),
                "client_role": "desktop", "capabilities": ["backup.writer.owner"],
                "operation": backup_writer::OPERATION, "action": {
                    "kind":"prepare_removal", "older":first, "older_checkpoint":check,
                    "files":{"backup":second, "checkpoint":newer_checkpoint, "kit":sealed, "unlock":unlock}
                }
            });
            backup_writer::handle(&serde_json::to_vec(&frame).unwrap(), Some(ctx), true).result
        };
        assert_eq!(
            prepare(&newer_checkpoint),
            Err(backup_writer::Error::Verification)
        );
        assert!(ctx.backup_removal.lock().unwrap().is_none());
        assert_eq!(
            prepare(&older_checkpoint),
            Ok(backup_writer::Outcome::RemovalPrepared)
        );
        assert_eq!(fs::read(&first).unwrap(), encrypted_old);
        assert_eq!(fs::read(&second).unwrap(), encrypted_new);
    } else {
        *ctx.backup_removal.lock().unwrap() = Some(
            backup_removal::Pending::from_review(ctx, writer.clone(), review().unwrap()).unwrap(),
        );
    }
    if browser_server {
        use std::io::{BufRead, Write};
        let mut after_cancel = Some(
            backup_removal::Pending::from_review(ctx, writer.clone(), review().unwrap()).unwrap(),
        );
        let token = std::env::var("CRYPTEX_FILE_FIXTURE_SOCKET").unwrap();
        let listener = ListenerOptions::new()
            .name(rehearsal_socket(&token).unwrap())
            .create_sync()
            .unwrap();
        println!("fixture-ready");
        io::stdout().flush().unwrap();
        for command in io::stdin().lock().lines() {
            let command = command.unwrap();
            if command == "finish" {
                break;
            }
            assert_eq!(command, "serve");
            let mut stream = listener.accept().unwrap();
            handle_connection(&mut stream, Some(ctx), None, None, None).unwrap();
            let mut slot = ctx.backup_removal.lock().unwrap();
            if slot.is_none() && after_cancel.is_some() {
                assert_eq!(fs::read(&first).unwrap(), encrypted_old);
                *slot = after_cancel.take();
            }
        }
        assert!(after_cancel.is_none());
    } else {
        use backup_removal::{Action, Outcome};
        let Outcome::Review { ticket, .. } = backup_removal::decide(ctx, Action::Inspect).unwrap()
        else {
            panic!("expected review")
        };
        assert!(matches!(
            backup_removal::decide(ctx, Action::Remove { ticket }),
            Ok(Outcome::Removed)
        ));
    }
    assert!(!first.exists());
    assert_eq!(fs::read(&second).unwrap(), encrypted_new);
    let last_copy = restore(2, &second).unwrap();
    assert_eq!(
        fs::read(
            last_copy
                .path()
                .join("canonical/blobs")
                .join(format!("{:x}", sha2::Sha256::digest(attachment)))
        )
        .unwrap(),
        attachment
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
