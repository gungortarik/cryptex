//! Disposable restore ceremony. No IPC route or production record format.
use super::*;
use crate::capture::{CaptureOutcome, CaptureRequest, capture_memory_note};
use crate::knowledge_materialization::recovered_search::*;
use cryptex_core::{RecoveryKitPublicMetadata, RecoveryPurpose};
use cryptex_events::LocalTrustAnchor;
use ed25519_dalek::{Signer, VerifyingKey};

#[path = "m11_restarted_search.rs"]
pub(super) mod restarted_search;

impl Approval {
    // Models an explicit owner signature produced with the offline kit.
    fn issue(request: &Request, offline: &SigningKey) -> Self {
        let bytes = serde_json_canonicalizer::to_vec(request).unwrap();
        Self {
            request: request.clone(),
            signature: offline.sign(&bytes).to_bytes().to_vec(),
        }
    }
}

impl Ceremony {
    fn open_replacement(
        &self,
        root: &Path,
        store: &dyn SecureStore,
    ) -> Result<(LocalDeviceKeyResolver, ArchiveReader), Denial> {
        let archive = self.reopen()?;
        let local = LocalDeviceKeyResolver::load(
            root,
            &self.expected.brain_id,
            store,
            SecureStoreEnvironment::Dev,
        )
        .map_err(|_| Denial::Context)?;
        let key = local.local_trust_anchor().ok_or(Denial::Context)?;
        if key.device_id() != &self.expected.new_device
            || key.public_key() != &self.expected.new_key
        {
            return Err(Denial::Context);
        }
        Ok((local, archive))
    }
}

fn random_bytes() -> [u8; 32] {
    let mut bytes = [0; 32];
    getrandom::getrandom(&mut bytes).unwrap();
    bytes
}

pub(super) struct OfflineInputs<'a> {
    pub(super) sealed_kit: &'a Path,
    pub(super) unlock: &'a Path,
    pub(super) backup: &'a Path,
}

impl OfflineInputs<'_> {
    fn sign(&self, challenge: &Path, approval: &Path) {
        let script = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/m11-backup-spike/offline_approval_test.py");
        assert!(
            std::process::Command::new("python")
                .arg("-B")
                .arg(script)
                .args([
                    challenge,
                    self.sealed_kit,
                    self.unlock,
                    self.backup,
                    approval
                ])
                .status()
                .unwrap()
                .success()
        );
    }

    fn owner_approval(&self, work: &Path, request: &Request, ceremony: &Ceremony) -> Approval {
        let folder = work.join("public-challenges");
        fs::create_dir(&folder).unwrap();
        let challenge = if let Some(desktop) = std::env::var_os("CRYPTEX_M11_INSTALLED_DESKTOP") {
            super::separate_backups::launched(
                Path::new(&desktop),
                work,
                "first-device-export",
                self.unlock,
                "",
            );
            let files = fs::read_dir(&folder)
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert_eq!(files.len(), 1);
            files[0].path()
        } else {
            folder.join(ceremony.export_challenge(&folder).unwrap().file)
        };
        assert_eq!(
            fs::read(&challenge).unwrap(),
            serde_json_canonicalizer::to_vec(request).unwrap()
        );
        let approval = folder.join("owner-approval.json");
        self.sign(&challenge, &approval);
        serde_json::from_slice(&fs::read(approval).unwrap()).unwrap()
    }
}

pub(super) fn blank_replacement(
    desktop: &Path,
    work: &Path,
    files: OfflineInputs<'_>,
    selected: &Path,
) {
    let trial = |mode| super::separate_backups::launched(desktop, work, mode, files.unlock, "");
    let root = work.join("clean-replacement");
    let identity = root.join("identity");
    let receipt = root.join(RECOVERY).join(RECEIPT);
    // The preceding inspection left the selected ciphertext truncated.
    trial("first-device-clean-reject");
    assert!(!identity.exists() && !root.join(RECOVERY).exists());
    fs::copy(files.backup, selected).unwrap();
    // A normal M8 instance lock is insufficient for the restored v2 history.
    let legacy = work.join("legacy-first-device");
    let _old_layout = crate::instance::try_acquire_instance(&legacy).unwrap();
    let isolated = crate::secure_store::FakeSecureStore::new();
    let codec = std::env::var_os("CRYPTEX_M11_TEST_INSTALLED_CODEC").unwrap();
    let sources = crate::ipc::backup_verify::Selection {
        backup: selected.into(),
        checkpoint: work.join("kit/selected-backup.json"),
        kit: files.sealed_kit.into(),
        unlock: files.unlock.into(),
    };
    assert!(
        crate::knowledge_materialization::backup::first_device::prepare(
            &legacy,
            &isolated,
            SecureStoreEnvironment::Dev,
            Path::new(&codec),
            &sources
        )
        .is_err()
    );
    assert!(!legacy.join(RECOVERY).exists() && !legacy.join("identity").exists());
    let sentinel = root.join("existing-profile");
    fs::write(&sentinel, b"keep").unwrap();
    trial("first-device-clean-reject");
    assert_eq!(fs::read(&sentinel).unwrap(), b"keep");
    assert!(!identity.exists() && !root.join(RECOVERY).exists());
    fs::remove_file(sentinel).unwrap();
    trial("first-device-clean-prepare");
    let local = crate::local_device_identity::local_device_identity_path(&root);
    let staged_identity =
        crate::local_device_identity::local_device_identity_path(&root.join(RECOVERY));
    let before = fs::read(&staged_identity).unwrap();
    assert!(!receipt.exists());
    assert!(!identity.exists());
    let store =
        crate::secure_store::FileBackedSecureStore::open(work.join("clean-credentials")).unwrap();
    let env = SecureStoreEnvironment::Dev;
    let capture = || {
        capture_memory_note(
            &store,
            env,
            &root,
            CaptureRequest {
                capture_id: "3d8412fa-c224-4799-a3db-08d891ee0cb9".into(),
                classification: Classification::Public,
                text: "First recovered device note".into(),
            },
            &CanonicalWriterConfig::default(),
        )
    };
    assert!(!matches!(capture(), CaptureOutcome::Committed(_)));
    assert!(!root.join("canonical").exists());
    trial("first-device-clean-prepare");
    assert_eq!(fs::read(&staged_identity).unwrap(), before);
    let exchange = work.join("clean-challenges");
    fs::create_dir(&exchange).unwrap();
    trial("first-device-clean-export");
    let entries: Vec<_> = fs::read_dir(&exchange)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(entries.len(), 1);
    let challenge = entries[0].path();
    let request: Request = serde_json::from_slice(&fs::read(&challenge).unwrap()).unwrap();
    assert_eq!(
        request.backup_sha256,
        <[u8; 32]>::from(Sha256::digest(fs::read(selected).unwrap()))
    );
    assert_ne!(request.new_device, request.archive_device);
    let signed = exchange.join("owner-approval.json");
    files.sign(&challenge, &signed);
    trial("first-device-clean-import");
    assert_eq!(fs::read(&local).unwrap(), before);
    // Power loss after the receipt was saved but before identity activation.
    fs::rename(&identity, root.join(RECOVERY).join("identity")).unwrap();
    trial("first-device-clean-import");
    assert_eq!(fs::read(&local).unwrap(), before);
    assert_eq!(
        fs::read(receipt).unwrap(),
        serde_json::to_vec(
            &serde_json::from_slice::<Approval>(&fs::read(&signed).unwrap()).unwrap()
        )
        .unwrap()
    );
    assert!(matches!(capture(), CaptureOutcome::Committed(_)));
    let mut resolver = LocalDeviceKeyResolver::load(&root, BRAIN, &store, env).unwrap();
    let (mut db, _) = crate::projection::open_projection_db(&root).unwrap();
    rebuild_knowledge_from_local_approval_history(
        &root,
        BRAIN,
        &resolver.local_trust_anchor().unwrap(),
        &mut db,
    )
    .unwrap();
    drop(db);
    let ceremony =
        crate::knowledge_materialization::backup::first_device::bound(&root, &store, env).unwrap();
    let hits = crate::knowledge_materialization::recovered_search::search(
        &root,
        &ceremony,
        &mut resolver,
        &SearchQuery {
            text: Some("prefix OR recovered".into()),
            ..SearchQuery::default()
        },
        None,
    )
    .unwrap();
    for text in ["synthetic original prefix", "First recovered device note"] {
        assert!(hits.iter().any(|hit| hit.text == text));
    }
    println!(
        "m11_replacement_activation=passed unsigned_capture=held signed_capture=committed guarded_search=both"
    );
    println!(
        "m11_blank_bootstrap=passed damaged=held existing=preserved interrupted=resumed device_key=stable offline_approval=verified"
    );
}

pub(super) fn rehearse(
    work: &Path,
    backup_sha256: [u8; 32],
    archived: &LocalTrustAnchor,
    offline: SigningKey,
    pinned: VerifyingKey,
    history_digest: [u8; 32],
    owner_inputs: OfflineInputs<'_>,
) -> ArchiveReader {
    let new_root = work.join("replacement");
    let store =
        crate::secure_store::FileBackedSecureStore::open(work.join("test-credentials")).unwrap();
    let env = SecureStoreEnvironment::Dev;
    let identity =
        crate::identity_runtime::load_brain_identity(&new_root.join("recovery/archive")).unwrap();
    assert_eq!(identity.brain_id, BRAIN);
    crate::identity_runtime::write_brain_identity(&new_root, &identity).unwrap();
    let target = crate::local_device_identity::enroll_local_device(&new_root, &store, env).unwrap();
    let device = DeviceId::new(target.device_id).unwrap();
    let local = LocalDeviceKeyResolver::load(&new_root, BRAIN, &store, env).unwrap();
    let target = local.local_trust_anchor().unwrap();
    assert_ne!(target.device_id(), archived.device_id());
    assert_ne!(target.public_key(), archived.public_key());
    let public = RecoveryKitPublicMetadata::new(
        format!("{:x}", Sha256::digest(pinned.as_bytes())),
        BRAIN,
        vec![
            RecoveryPurpose::TrustBootstrap,
            RecoveryPurpose::BackupDecrypt,
        ],
    )
    .unwrap();
    let request = Request {
        operation: "cryptex.m11.trial.authorize-first-device.v2".into(),
        kit_id: public.kit_id.clone(),
        brain_id: public.brain_id.clone(),
        challenge: random_bytes(),
        backup_sha256,
        new_device: device.clone(),
        new_key: *target.public_key(),
        archive_device: archived.device_id().clone(),
        archive_key: *archived.public_key(),
        archive_history_sha256: history_digest,
        brain_identity_sha256: Some(
            Sha256::digest(identity.to_json_lf().unwrap().as_bytes()).into(),
        ),
    };
    let mut ceremony = Ceremony {
        expected: request.clone(),
        kit: public,
        trusted_root: pinned,
        receipt: new_root.join(RECOVERY).join(RECEIPT),
    };
    assert!(matches!(
        ceremony.open_replacement(&new_root, &store),
        Err(Denial::Receipt)
    ));
    let binding = Binding {
        request: ceremony.expected.clone(),
        kit: ceremony.kit.clone(),
        pinned: ceremony.trusted_root.to_bytes(),
    };
    let account = crate::secure_store::AccountLabel::recovery_search(device.as_str());
    let service = crate::secure_store::ServiceLabel::device_credentials(env);
    store
        .put(
            service.as_str(),
            account.as_str(),
            &serde_json::to_vec(&binding).unwrap(),
        )
        .unwrap();
    let grant = owner_inputs.owner_approval(work, &request, &ceremony);
    let mut legacy = request.clone();
    legacy.operation = "cryptex.m11.trial.authorize-first-device.v1".into();
    legacy.brain_identity_sha256 = None;
    assert!(
        serde_json::to_value(&legacy)
            .unwrap()
            .get("brain_identity_sha256")
            .is_none()
    );
    ceremony.expected = legacy.clone();
    ceremony
        .verify(&Approval::issue(&legacy, &offline))
        .unwrap();
    ceremony.expected = request.clone();
    assert_eq!(
        ceremony.verify(&Approval::issue(&legacy, &offline)),
        Err(Denial::Context)
    );
    // A normal device can sign, but has no recovery authority.
    let ordinary = load_existing_event_signing_key(&store, env, &new_root, &device).unwrap();
    assert_eq!(
        ceremony.admit(&Approval::issue(&request, &ordinary.signing_key)),
        Err(Denial::Signature)
    );
    drop(ordinary);
    ceremony.kit.purposes = vec![RecoveryPurpose::BackupDecrypt];
    assert_eq!(ceremony.admit(&grant), Err(Denial::Context));
    ceremony.kit.purposes.push(RecoveryPurpose::TrustBootstrap);
    for edit in [
        (|r: &mut Request| r.brain_id = "11111111-1111-4111-8111-111111111111".into())
            as fn(&mut Request),
        |r| r.operation = "backup.decrypt".into(),
        |r| r.kit_id.push('x'),
        |r| r.challenge[0] ^= 1,
        |r| r.backup_sha256[0] ^= 1,
        |r| r.brain_identity_sha256 = None,
        |r| r.new_key[0] ^= 1,
        |r| r.archive_key[0] ^= 1,
        |r| r.new_device = r.archive_device.clone(),
    ] {
        let mut different = request.clone();
        edit(&mut different);
        assert_eq!(
            ceremony.admit(&Approval::issue(&different, &offline)),
            Err(Denial::Context)
        );
    }
    assert!(!ceremony.receipt.exists());
    assert!(!new_root.join("canonical/events").exists());
    let exchange = work.join("public-challenges");
    if let Some(desktop) = std::env::var_os("CRYPTEX_M11_INSTALLED_DESKTOP") {
        let wrong = Approval::issue(&request, &SigningKey::from_bytes(&random_bytes()));
        fs::write(
            exchange.join("wrong-approval.json"),
            serde_json::to_vec(&wrong).unwrap(),
        )
        .unwrap();
        super::separate_backups::launched(
            Path::new(&desktop),
            work,
            "first-device-wrong",
            owner_inputs.unlock,
            "",
        );
        assert!(!ceremony.receipt.exists());
        super::separate_backups::launched(
            Path::new(&desktop),
            work,
            "first-device-import",
            owner_inputs.unlock,
            "",
        );
    } else {
        ceremony
            .accept_owner_file(&new_root, &exchange.join("owner-approval.json"))
            .unwrap();
    }
    let original = fs::read(&ceremony.receipt).unwrap();
    ceremony.admit(&grant).unwrap();
    let mut competing = request.clone();
    competing.challenge = random_bytes();
    ceremony.expected = competing.clone();
    assert_eq!(
        ceremony.admit(&Approval::issue(&competing, &offline)),
        Err(Denial::AlreadyAuthorized)
    );
    ceremony.expected = request;
    drop(offline);
    assert_eq!(fs::read(&ceremony.receipt).unwrap(), original);
    assert!(matches!(
        ceremony.open_replacement(&new_root, &FakeSecureStore::new()),
        Err(Denial::Context)
    ));
    let (mut local, historical_reader) = ceremony.open_replacement(&new_root, &store).unwrap();

    let outcome = capture_memory_note(
        &store,
        env,
        &new_root,
        CaptureRequest {
            capture_id: "3d8412fa-c224-4799-a3db-08d891ee0cb9".into(),
            classification: Classification::Public,
            text: "First recovered device note".into(),
        },
        &CanonicalWriterConfig::default(),
    );
    let CaptureOutcome::Committed(commit) = outcome else {
        panic!("offline local capture did not commit: {outcome:?}");
    };
    let (mut db, _) = crate::projection::open_projection_db(&new_root).unwrap();
    rebuild_knowledge_from_local_approval_history(&new_root, BRAIN, &target, &mut db).unwrap();
    let hits = search_events_with_retrieval_safety(
        &new_root,
        BRAIN,
        &[],
        &mut local,
        &db,
        &SearchQuery::default(),
    )
    .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].event_id, commit.event_id);
    assert_eq!(hits[0].device_id, device.as_str());
    assert_eq!(hits[0].text, "First recovered device note");
    let mut tampered = grant;
    tampered.signature[0] ^= 1;
    fs::write(&ceremony.receipt, serde_json::to_vec(&tampered).unwrap()).unwrap();
    assert!(matches!(ceremony.reopen(), Err(Denial::Signature)));
    fs::write(&ceremony.receipt, original).unwrap();
    restarted_search::save_public_context(work, &ceremony, &commit.event_id);
    historical_reader
}
