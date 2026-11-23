//! Synthetic owner-approved evidence for the installed encrypted backup test.
use super::*;
use crate::cortex_evidence::{self as evidence, Approval, BatchDecision, Choice, Receipt, Source};
use cryptex_events::{EventPayload, SignatureHex};
use ed25519_dalek::{Signer, SigningKey};

pub(super) struct Evidence {
    source_name: String,
    source: Vec<u8>,
    decision: Vec<u8>,
}

fn signed(value: &impl serde::Serialize, key: &SigningKey) -> Vec<u8> {
    let canonical = serde_json_canonicalizer::to_vec(value).unwrap();
    let signature = SignatureHex::from_bytes(&key.sign(&canonical).to_bytes()).unwrap();
    serde_json_canonicalizer::to_vec(&Receipt {
        source: value,
        signature,
    })
    .unwrap()
}

impl Evidence {
    pub(super) fn enroll(
        root: &Path,
        device: &DeviceId,
        key: &SigningKey,
        store: &impl crate::SecureStore,
    ) -> Self {
        let events = crate::canonical_writer::load_verified_events(
            store,
            SecureStoreEnvironment::Dev,
            root,
            device,
        )
        .unwrap();
        let [event] = events.as_slice() else {
            panic!("expected one synthetic note")
        };
        let EventPayload::MemoryNote(note) = &event.payload;
        let source = Source {
            schema: evidence::SCHEMA.into(),
            brain_id: event.brain_id.clone(),
            device_id: event.device_id.clone(),
            event_id: event.event_id.clone(),
            content_hash: event.content_hash.as_str().into(),
            approval: Approval {
                capture_id: "35027d23-6788-4d0f-8ef5-80c08e9faaaa".into(),
                source_path: "Notes/imported.md".into(),
                source_sha256: format!("{:x}", Sha256::digest(note.text.as_bytes())),
                classification: event.classification,
                review_sha256: None,
            },
        };
        let report =
            evidence::comparison_report(BRAIN, std::slice::from_ref(&source), &[], &[], &[]);
        let decision = BatchDecision {
            schema: evidence::DECISION_SCHEMA.into(),
            brain_id: BRAIN.into(),
            device_id: device.as_str().into(),
            review_sha256: evidence::review_digest(&report).unwrap(),
            choice: Choice::Accepted,
        };
        let copies = Self {
            source_name: format!("{}/{}.json", evidence::DIRECTORY, event.event_id),
            source: signed(&source, key),
            decision: signed(&decision, key),
        };
        fs::create_dir(root.join(evidence::DIRECTORY)).unwrap();
        fs::write(root.join(&copies.source_name), &copies.source).unwrap();
        fs::write(root.join(evidence::DECISION_FILE), &copies.decision).unwrap();
        copies
    }

    pub(super) fn check(&self, root: &Path) {
        assert_eq!(fs::read(root.join(&self.source_name)).unwrap(), self.source);
        assert_eq!(
            fs::read(root.join(evidence::DECISION_FILE)).unwrap(),
            self.decision
        );
    }

    pub(super) fn check_damage(
        &self,
        root: &Path,
        checkpoint: &crate::backup_inventory::Checkpoint<'_>,
    ) {
        use crate::backup_inventory::{Error, verify_reconstruction};
        self.check(root);
        let decision_path = root.join(evidence::DECISION_FILE);
        let source_path = root.join(&self.source_name);
        let mut invalid = self.decision.clone();
        invalid.push(b' ');
        fs::write(&decision_path, invalid).unwrap();
        assert!(matches!(
            verify_reconstruction(root, checkpoint),
            Err(Error::CortexEvidence)
        ));
        fs::write(&decision_path, &self.decision).unwrap();
        fs::remove_file(&source_path).unwrap();
        assert!(matches!(
            verify_reconstruction(root, checkpoint),
            Err(Error::CortexEvidence)
        ));
        fs::remove_dir(root.join(evidence::DIRECTORY)).unwrap();
        fs::remove_file(decision_path).unwrap();
        assert!(matches!(
            verify_reconstruction(root, checkpoint),
            Err(Error::CortexEvidence)
        ));
    }
}

#[test]
fn ordinary_and_cortex_backup_inventories_roundtrip() {
    use crate::backup_inventory::reconstruct_v2;
    let work = tempfile::tempdir().unwrap();
    let root = work.path().join("synthetic-source");
    let store = crate::secure_store::FakeSecureStore::new();
    let (device, key) = PairProfiles::enroll(&root, &store);
    let prepare = || backup::prepare(&root, &store, SecureStoreEnvironment::Dev, 1).unwrap();
    let unpack = |prepared: &backup::Prepared| {
        let file = prepared.encryption_input().unwrap();
        let directory = tempfile::tempdir_in(work.path()).unwrap();
        let mut zip = zip::ZipArchive::new(file.as_file()).unwrap();
        zip.extract(directory.path()).unwrap();
        let inventory = fs::read(directory.path().join("inventory.json")).unwrap();
        (directory, inventory)
    };
    let plain = prepare();
    let (plain_dir, plain_inventory) = unpack(&plain);
    let format = |bytes: &[u8]| {
        serde_json::from_slice::<serde_json::Value>(bytes).unwrap()["body"]["format"].clone()
    };
    assert_eq!(format(&plain_inventory), "cryptex.backup.inventory.v2");
    let anchor = plain.selection.writer().unwrap();
    let old = plain.selection.checkpoint(&anchor).unwrap();
    assert!(old.cortex_decision_sha256.is_none());
    let old_copy = reconstruct_v2(
        &plain_dir.path().join("content.zip"),
        &plain_inventory,
        &old,
        work.path(),
    )
    .unwrap();
    assert!(!old_copy.path().join(evidence::DECISION_FILE).exists());
    let receipts = Evidence::enroll(&root, &device, &key, &store);
    let decision_path = root.join(evidence::DECISION_FILE);
    let mut decision: Receipt<BatchDecision> = serde_json::from_slice(&receipts.decision).unwrap();
    decision.source.choice = Choice::Cancelled;
    fs::write(&decision_path, signed(&decision.source, &key)).unwrap();
    assert!(matches!(
        backup::prepare(&root, &store, SecureStoreEnvironment::Dev, 1),
        Err(crate::backup_inventory::Error::CortexEvidence)
    ));
    fs::remove_file(&decision_path).unwrap();
    assert!(matches!(
        backup::prepare(&root, &store, SecureStoreEnvironment::Dev, 1),
        Err(crate::backup_inventory::Error::CortexEvidence)
    ));
    fs::write(decision_path, &receipts.decision).unwrap();
    let accepted = prepare();
    let (accepted_dir, accepted_inventory) = unpack(&accepted);
    assert_eq!(format(&accepted_inventory), "cryptex.backup.inventory.v3");
    let current = accepted.selection.checkpoint(&anchor).unwrap();
    assert!(current.cortex_decision_sha256.is_some());
    assert!(
        reconstruct_v2(
            &accepted_dir.path().join("content.zip"),
            &accepted_inventory,
            &old,
            work.path()
        )
        .is_err()
    );
    assert!(
        reconstruct_v2(
            &plain_dir.path().join("content.zip"),
            &plain_inventory,
            &current,
            work.path()
        )
        .is_err()
    );
    let copy = reconstruct_v2(
        &accepted_dir.path().join("content.zip"),
        &accepted_inventory,
        &current,
        work.path(),
    )
    .unwrap();
    receipts.check_damage(copy.path(), &current);
}
