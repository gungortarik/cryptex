//! Installed Desktop export, followed by a restore after losing the disposable source.
use super::*;
use crate::secure_store::FileBackedSecureStore;

pub(super) fn check(work: &Path, desktop: &Path, kit: &Path, password: &Path) {
    let root = work.join("export-source");
    let volume = work.join("export-volume");
    let selections = work.join("export-checkpoints");
    for path in [&root, &volume, &selections] {
        fs::create_dir(path).unwrap();
    }
    let store = FileBackedSecureStore::open(work.join("export-credentials")).unwrap();
    let (device, _) = PairProfiles::enroll(&root, &store);
    let local = resolver_for_export(&root, &store);
    drop(store);
    let public = work.join("kit-public");
    assert_eq!(fs::read_dir(&public).unwrap().count(), 2);
    let digest = format!(
        "{:x}",
        Sha256::digest(fs::read(public.join("recovery.pub")).unwrap())
    );
    let identity = public.join("brain-identity.sha256");
    let brain = fs::read(&identity).unwrap();
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/m11-backup-spike/installed_desktop.py");
    let targets = work.join("writer-targets");
    fs::create_dir(&targets).unwrap();
    for mode in [
        "writer-decline",
        "writer-create",
        "writer-inspect",
        "writer-copy",
    ] {
        if mode == "writer-copy" {
            let owned = fs::read_dir(&targets)
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            let other = work.join("other-target");
            fs::create_dir(&other).unwrap();
            fs::copy(
                owned.join("backup-target.id"),
                other.join("backup-target.id"),
            )
            .unwrap();
        }
        let status = Command::new("python")
            .arg("-B")
            .arg(&script)
            .arg("export")
            .arg(desktop)
            .arg(work)
            .env("CRYPTEX_M11_INSTALLED_EXPORT", mode)
            .status()
            .unwrap();
        assert!(status.success(), "installed backup writer: {mode}");
        assert_eq!(
            fs::read_dir(&targets).unwrap().count(),
            usize::from(mode != "writer-decline")
        );
    }
    println!("m11_installed_writer=passed consent=required assignment=reopened copied_target=held");
    for mode in [
        "wrong-brain",
        "decline",
        "cancel",
        "overlap",
        "write",
        "damaged",
    ] {
        if mode == "wrong-brain" {
            fs::write(&identity, [0; 32]).unwrap();
        }
        if mode == "decline" {
            fs::write(&identity, &brain).unwrap();
        }
        if mode == "overlap" {
            fs::remove_dir_all(&public).unwrap();
        }
        if mode == "damaged" {
            fs::write(
                root.join("backup-certificates")
                    .join(format!("{digest}.asc")),
                b"changed",
            )
            .unwrap();
        }
        assert!(
            Command::new("python")
                .arg("-B")
                .arg(&script)
                .arg("export")
                .arg(desktop)
                .arg(work)
                .env("CRYPTEX_M11_INSTALLED_EXPORT", mode)
                .env("CRYPTEX_M11_EXPECT_PUBLIC_DIGEST", &digest)
                .status()
                .unwrap()
                .success(),
            "installed export: {mode}"
        );
        let expected = usize::from(matches!(mode, "write" | "damaged"));
        if matches!(mode, "wrong-brain" | "decline") {
            assert!(!root.join("backup-certificates").exists());
        }
        assert_eq!(fs::read_dir(&volume).unwrap().count(), expected);
        assert_eq!(fs::read_dir(&selections).unwrap().count(), expected);
    }
    let only_file = |dir: &Path| fs::read_dir(dir).unwrap().next().unwrap().unwrap().path();
    let backup = only_file(&volume);
    let selection_file = only_file(&selections);
    assert_eq!(backup.file_stem(), selection_file.file_stem());
    assert_eq!(backup.extension().unwrap(), "pgp");
    assert_eq!(selection_file.extension().unwrap(), "json");
    fs::remove_dir_all(&root).unwrap();
    fs::remove_dir_all(work.join("export-credentials")).unwrap();
    // The installed verifier opens on a new disposable session after source loss.
    let session = work.join("verify-session");
    fs::create_dir(&session).unwrap();
    let selected = selections.join("restore-selection.json");
    let cipher = volume.join("restore-input.pgp");
    let original = fs::read(&backup).unwrap();
    let checkpoint_bytes = fs::read(&selection_file).unwrap();
    for mode in [
        "verify-cancel",
        "verify-wrong-key",
        "verify-wrong-checkpoint",
        "verify-damaged",
        "verify",
    ] {
        let mut bytes = original.clone();
        if mode == "verify-damaged" {
            *bytes.last_mut().unwrap() ^= 1;
        }
        fs::write(&cipher, bytes).unwrap();
        if mode == "verify-wrong-checkpoint" {
            let mut other: serde_json::Value = serde_json::from_slice(&checkpoint_bytes).unwrap();
            other["generation"] = 2.into();
            fs::write(&selected, serde_json::to_vec(&other).unwrap()).unwrap();
        } else {
            fs::write(&selected, &checkpoint_bytes).unwrap();
        }
        let mut launch = Command::new("python");
        launch
            .arg("-B")
            .arg(&script)
            .arg("export")
            .arg(desktop)
            .arg(work)
            .env("CRYPTEX_M11_INSTALLED_EXPORT", mode)
            .env_remove("CRYPTEX_M11_VERIFY_UNLOCK");
        if mode == "verify-wrong-key" {
            launch.env(
                "CRYPTEX_M11_VERIFY_UNLOCK",
                password.with_file_name("wrong"),
            );
        } else if mode != "verify-cancel" {
            launch.env("CRYPTEX_M11_VERIFY_UNLOCK", password);
        }
        assert!(
            launch.status().unwrap().success(),
            "installed verification: {mode}"
        );
        assert!(!root.exists());
        assert!(!session.join("canonical").exists());
    }
    println!(
        "m11_installed_verify=passed source_absent=true wrong_unlock=held wrong_checkpoint=held damaged=held cancelled=true"
    );
    let opened = work.join("opened-manual-export");
    run_archive("open-payload", &[kit, &backup, &opened]);
    let selection: crate::backup_inventory::RecoverySelection =
        serde_json::from_slice(&fs::read(selection_file).unwrap()).unwrap();
    let checkpoint = selection.checkpoint(&local).unwrap();
    let archive = crate::backup_inventory::reconstruct_v2(
        &opened.join("content.zip"),
        &fs::read(opened.join("inventory.json")).unwrap(),
        &checkpoint,
        work,
    )
    .unwrap();
    let snapshot =
        scan_verified_canonical_from_local_anchor_until(archive.path(), BRAIN, &local, None)
            .unwrap();
    assert!(snapshot.verified_records().iter().any(|record| matches!(record, VersionedRecord::VerifiedV1(note)
        if note.device_id == device.as_str() && matches!(&note.payload, cryptex_events::EventPayload::MemoryNote(p) if p.text == "synthetic original prefix"))));
    println!(
        "m11_recipient_enrollment=passed wrong_brain=held decline=kept offline_restart=restored modified_certificate=held"
    );
    println!("m11_installed_export=passed cancelled=true overlap=held written=true restored=true");
}

fn resolver_for_export(
    root: &Path,
    store: &FileBackedSecureStore,
) -> cryptex_events::LocalTrustAnchor {
    LocalDeviceKeyResolver::load(root, BRAIN, store, SecureStoreEnvironment::Dev)
        .unwrap()
        .local_trust_anchor()
        .unwrap()
}
