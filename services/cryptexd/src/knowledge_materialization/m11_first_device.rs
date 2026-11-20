//! Disposable restore ceremony. No IPC route or production record format.
use super::*;
use cryptex_core::{RecoveryKitPublicMetadata, RecoveryPurpose};
use cryptex_events::LocalTrustAnchor;
use ed25519_dalek::{Signature, Signer, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Write;

#[path = "m11_restarted_search.rs"]
pub(super) mod restarted_search;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    operation: String,
    kit_id: String,
    brain_id: String,
    challenge: [u8; 32],
    backup_sha256: [u8; 32],
    new_device: DeviceId,
    new_key: [u8; 32],
    archive_device: DeviceId,
    archive_key: [u8; 32],
    archive_history_sha256: [u8; 32],
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Approval {
    request: Request,
    signature: Vec<u8>,
}

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

#[derive(Debug, PartialEq, Eq)]
enum Denial {
    Context,
    Signature,
    AlreadyAuthorized,
    Receipt,
}

struct Ceremony {
    expected: Request,
    kit: RecoveryKitPublicMetadata,
    trusted_root: VerifyingKey,
    receipt: PathBuf,
}

impl Ceremony {
    fn verify(&self, approval: &Approval) -> Result<(), Denial> {
        if self.kit.validate().is_err()
            || !self.kit.purposes.contains(&RecoveryPurpose::TrustBootstrap)
            || self.kit.brain_id != self.expected.brain_id
            || self.kit.kit_id != self.expected.kit_id
            || self.expected.operation != "cryptex.m11.trial.authorize-first-device.v1"
            || self.expected != approval.request
        {
            return Err(Denial::Context);
        }
        let bytes =
            serde_json_canonicalizer::to_vec(&approval.request).map_err(|_| Denial::Receipt)?;
        let signature =
            Signature::from_slice(&approval.signature).map_err(|_| Denial::Signature)?;
        self.trusted_root
            .verify_strict(&bytes, &signature)
            .map_err(|_| Denial::Signature)
    }

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

    fn reopen(&self) -> Result<ArchiveReader, Denial> {
        let reader = fs::File::open(&self.receipt).map_err(|_| Denial::Receipt)?;
        let approval: Approval = serde_json::from_reader(reader).map_err(|_| Denial::Receipt)?;
        self.verify(&approval)?;
        let history = &approval.request;
        let key = VerifyingKey::from_bytes(&history.archive_key).map_err(|_| Denial::Receipt)?;
        let id =
            cryptex_events::signing_key_id_from_verifying_key(&key).map_err(|_| Denial::Receipt)?;
        Ok(ArchiveReader(LocalTrustAnchor::new(
            history.archive_device.clone(),
            id,
            key.to_bytes(),
        )))
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

// This resolver is confined to historical reads. The replacement signs with
// its newly enrolled key in its own device stream.
pub(super) struct ArchiveReader(LocalTrustAnchor);
impl TrustedDaemonKeyResolver for ArchiveReader {
    fn local_trust_anchor(&self) -> Option<LocalTrustAnchor> {
        Some(self.0.clone())
    }
    fn trusted_daemon_key(&mut self, device: &str, key: &str) -> Option<VerifyingKey> {
        if self.0.device_id().as_str() != device || self.0.signing_key_id().as_str() != key {
            return None;
        }
        VerifyingKey::from_bytes(self.0.public_key()).ok()
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
        receipt: work.join("first-device.json"),
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
    restarted_search::save_public_context(work, &ceremony, &hits[0].event_id);
    historical_reader
}
