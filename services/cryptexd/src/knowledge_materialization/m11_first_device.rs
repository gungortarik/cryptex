//! Disposable restore ceremony. No IPC route or production record format.
use super::*;
use crate::knowledge_materialization::recovered_search::*;
use cryptex_core::{RecoveryKitPublicMetadata, RecoveryPurpose};
use cryptex_events::LocalTrustAnchor;
use ed25519_dalek::{Signer, VerifyingKey};
use sha2::{Digest, Sha256};
use std::io::Write;

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
    // One slot per restore. Publish a complete receipt without overwriting a
    // concurrent winner. An identical retry must pass signature checks again.
    fn admit(&self, approval: &Approval) -> Result<(), Denial> {
        self.verify(approval)?;
        let parent = self.receipt.parent().ok_or(Denial::Receipt)?;
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|_| Denial::Receipt)?;
        serde_json::to_writer(&mut file, approval).map_err(|_| Denial::Receipt)?;
        file.flush()
            .and_then(|()| file.as_file().sync_all())
            .map_err(|_| Denial::Receipt)?;
        match file.persist_noclobber(&self.receipt) {
            Ok(_) => Ok(()),
            Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
                self.reopen().map(|_| ()).map_err(|e| {
                    if e == Denial::Context {
                        Denial::AlreadyAuthorized
                    } else {
                        e
                    }
                })
            }
            Err(_) => Err(Denial::Receipt),
        }
    }

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

pub(super) fn rehearse(
    work: &Path,
    backup: &Path,
    archived: &LocalTrustAnchor,
    offline: SigningKey,
    pinned: VerifyingKey,
    history_digest: [u8; 32],
) -> ArchiveReader {
    let new_root = work.join("replacement");
    let store =
        crate::secure_store::FileBackedSecureStore::open(work.join("test-credentials")).unwrap();
    let env = SecureStoreEnvironment::Dev;
    crate::identity_runtime::write_brain_identity(
        &new_root,
        &BrainIdentity::new(BRAIN, "2026-01-01T00:00:00Z").unwrap(),
    )
    .unwrap();
    let target = crate::local_device_identity::enroll_local_device(&new_root, &store, env).unwrap();
    let device = DeviceId::new(target.device_id).unwrap();
    let local = LocalDeviceKeyResolver::load(&new_root, BRAIN, &store, env).unwrap();
    let target = local.local_trust_anchor().unwrap();
    assert_ne!(target.device_id(), archived.device_id());
    assert_ne!(target.public_key(), archived.public_key());
    let public = RecoveryKitPublicMetadata::new(
        "test-offline-kit",
        BRAIN,
        vec![
            RecoveryPurpose::TrustBootstrap,
            RecoveryPurpose::BackupDecrypt,
        ],
    )
    .unwrap();
    let request = Request {
        operation: "cryptex.m11.trial.authorize-first-device.v1".into(),
        kit_id: public.kit_id.clone(),
        brain_id: public.brain_id.clone(),
        challenge: random_bytes(),
        backup_sha256: Sha256::digest(fs::read(backup).unwrap()).into(),
        new_device: device.clone(),
        new_key: *target.public_key(),
        archive_device: archived.device_id().clone(),
        archive_key: *archived.public_key(),
        archive_history_sha256: history_digest,
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
    let grant = Approval::issue(&request, &offline);
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
    ceremony.admit(&grant).unwrap();
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

    append_memory_note(
        &store,
        env,
        AppendMemoryNoteArgs {
            runtime_root: &new_root,
            brain_id: BRAIN,
            device_id: &device,
            classification: Classification::Public,
            text: "First recovered device note",
            event_id: None,
            config: &CanonicalWriterConfig::default(),
        },
    )
    .unwrap();
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
    assert_eq!(hits[0].device_id, device.as_str());
    assert_eq!(hits[0].text, "First recovered device note");
    let mut tampered = grant;
    tampered.signature[0] ^= 1;
    fs::write(&ceremony.receipt, serde_json::to_vec(&tampered).unwrap()).unwrap();
    assert!(matches!(ceremony.reopen(), Err(Denial::Signature)));
    fs::write(&ceremony.receipt, original).unwrap();
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
    restarted_search::save_public_context(work, &ceremony, &hits[0].event_id);
    historical_reader
}
