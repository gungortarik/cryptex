use super::*;
use crate::knowledge_materialization::backup;

pub(super) struct Trial {
    desktop: std::path::PathBuf,
    new: std::path::PathBuf,
    old: std::path::PathBuf,
    checkpoint: std::path::PathBuf,
}

pub(super) fn launched(desktop: &Path, work: &Path, mode: &str, password: &Path, digest: &str) {
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/m11-backup-spike/installed_desktop.py");
    assert!(
        Command::new("python")
            .arg("-B")
            .arg(script)
            .arg("export")
            .arg(desktop)
            .arg(work)
            .env("CRYPTEX_M11_INSTALLED_EXPORT", mode)
            .env("CRYPTEX_M11_EXPECT_PUBLIC_DIGEST", digest)
            .env("CRYPTEX_M11_VERIFY_UNLOCK", password)
            .status()
            .unwrap()
            .success(),
        "installed backup pair: {mode}"
    );
}

pub(super) fn export(
    work: &Path,
    desktop: &Path,
    source: &Path,
    kit: &Path,
    password: &Path,
) -> Trial {
    let volume = work.join("pair-volume");
    let selections = work.join("pair-checkpoints");
    let input = work.join("pair-input");
    for path in [&volume, &selections, &input] {
        fs::create_dir(path).unwrap();
    }
    fs::copy(source, input.join("original.pgp")).unwrap();
    fs::copy(
        work.join("kit/selected-backup.json"),
        selections.join("original.json"),
    )
    .unwrap();
    let public = work.join("kit-public");
    run_archive("kit-public-recipient", &[kit, password, &public]);
    let recipient = fs::read(public.join("recovery.pub")).unwrap();
    let digest = format!("{:x}", Sha256::digest(&recipient));
    let wrong = input.join("unrelated.pgp");
    fs::write(&wrong, b"not the original ciphertext").unwrap();
    let store =
        crate::secure_store::FileBackedSecureStore::open(work.join("test-credentials")).unwrap();
    let codec = std::env::var_os("CRYPTEX_M11_TEST_INSTALLED_CODEC").unwrap();
    assert!(matches!(
        backup::export_recovered(
            &work.join("replacement"),
            &store,
            SecureStoreEnvironment::Dev,
            backup::PriorFiles {
                backup: &wrong,
                checkpoint: &selections.join("original.json"),
            },
            backup::Destination {
                codec: Path::new(&codec),
                recipient: &recipient,
                target: &volume,
                checkpoints: &selections,
            },
        ),
        Err(crate::backup_inventory::Error::RecoveryHistory)
    ));
    assert_eq!(fs::read_dir(&volume).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&selections).unwrap().count(), 1);
    launched(desktop, work, "pair-write", password, &digest);
    let files = fs::read_dir(&volume)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(files.len(), 2);
    let old = files
        .iter()
        .find(|f| f.to_string_lossy().ends_with(".prior.pgp"))
        .unwrap()
        .clone();
    let new = files.into_iter().find(|f| f != &old).unwrap();
    assert_eq!(fs::read(source).unwrap(), fs::read(&old).unwrap());
    assert_eq!(new.with_extension("prior.pgp"), old);
    let checkpoint = selections
        .join(new.file_name().unwrap())
        .with_extension("json");
    assert!(checkpoint.is_file());
    Trial {
        desktop: desktop.to_owned(),
        new,
        old,
        checkpoint,
    }
}

impl Trial {
    pub(super) fn restore_after_source_loss(
        self,
        work: &Path,
        kit: &Path,
        password: &Path,
        old_note: &str,
    ) {
        fs::remove_dir_all(work.join("replacement")).unwrap();
        fs::remove_dir_all(work.join("test-credentials")).unwrap();
        fs::remove_dir_all(work.join("pair-input")).unwrap();
        let new = self.new.with_file_name("selected.pgp");
        let old = new.with_extension("prior.pgp");
        let checkpoint = self.checkpoint.with_file_name("selected.json");
        fs::rename(self.new, &new).unwrap();
        fs::rename(self.checkpoint, &checkpoint).unwrap();
        launched(&self.desktop, work, "verify-pair-missing", password, "");
        fs::rename(self.old, &old).unwrap();
        launched(&self.desktop, work, "verify-pair", password, "");
        let codec = std::env::var_os("CRYPTEX_M11_TEST_INSTALLED_CODEC").unwrap();
        let scratch = tempfile::tempdir().unwrap();
        let open = |selected: &Path| {
            backup::reconstruct_files(
                Path::new(&codec),
                selected,
                &checkpoint,
                kit,
                password,
                scratch.path(),
            )
        };
        // The installed picker check chose the newer file. Choosing the prior
        // file with the same pair checkpoint must recover both histories too.
        let profiles = open(&old).unwrap();
        assert_eq!(profiles.len(), 2);
        let members: serde_json::Value =
            serde_json::from_slice(&fs::read(&checkpoint).unwrap()).unwrap();
        let mut hits = Vec::new();
        for (profile, role) in profiles.iter().zip(["archived", "replacement"]) {
            let selection: crate::backup_inventory::RecoverySelection =
                serde_json::from_value(members[role].clone()).unwrap();
            let key = selection.writer().unwrap();
            let snapshot = backup::snapshot(profile.path(), BRAIN, &key).unwrap();
            hits.push(
                snapshot
                    .verified_records()
                    .iter()
                    .filter_map(|record| match record {
                        VersionedRecord::VerifiedV1(note) => {
                            let cryptex_events::EventPayload::MemoryNote(n) = &note.payload;
                            Some((note.event_id.clone(), n.text.clone()))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
            );
        }
        assert!(
            hits[0]
                .iter()
                .any(|(id, text)| id == old_note && text == "synthetic original prefix")
        );
        assert!(
            hits[1]
                .iter()
                .any(|(_, text)| text == "First recovered device note")
        );
        assert!(hits[1].iter().all(|(id, _)| id != old_note));
        drop(profiles);
        // A sound newer file cannot make a mixed or incomplete pair verify.
        fs::copy(&new, &old).unwrap();
        assert!(open(&new).is_err());
        assert_eq!(fs::read_dir(scratch.path()).unwrap().count(), 0);
        println!(
            "m11_separate_files=passed archived_bytes=identical replacement=independent both_notes=restored missing=held swapped=held source=lost"
        );
    }
}
