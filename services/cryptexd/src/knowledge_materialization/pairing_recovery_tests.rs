//! Actual daemon-written bilateral anchors, not fabricated terminal cache rows.
//! All profiles, notes, keys and approvals are isolated synthetic test data.

use super::*;
use crate::bootstrap_journal::BootstrapJournalStore;
use crate::canonical_writer::{AppendMemoryNoteArgs, CanonicalWriterConfig, append_memory_note};
use crate::secure_store::FakeSecureStore;
use cryptex_core::BrainIdentity;
use cryptex_events::{BootstrapApprovalRole, Classification};
use ed25519_dalek::SigningKey;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

use crate::local_device_identity::LocalDeviceKeyResolver;
use std::collections::BTreeSet;

const BRAIN: &str = "88888888-8888-4888-8888-888888888888";

struct PairProfiles {
    root: PathBuf,
    profiles: [PathBuf; 2],
    devices: [DeviceId; 2],
    keys: [SigningKey; 2],
    store: FakeSecureStore,
}

impl PairProfiles {
    fn new() -> Self {
        let root = crate::test_temp::exclusive_temp_dir("m8-pairing-recovery");
        let store = FakeSecureStore::new();
        let profiles = [root.join("a"), root.join("b")];
        let (a, key_a) = Self::enroll(&profiles[0], &store);
        let (b, key_b) = Self::enroll(&profiles[1], &store);
        Self {
            root,
            profiles,
            devices: [a, b],
            keys: [key_a, key_b],
            store,
        }
    }

    fn enroll(root: &Path, store: &FakeSecureStore) -> (DeviceId, SigningKey) {
        crate::identity_runtime::write_brain_identity(
            root,
            &BrainIdentity::new(BRAIN, "2026-01-01T00:00:00Z").unwrap(),
        )
        .unwrap();
        let identity = crate::local_device_identity::enroll_local_device(
            root,
            store,
            SecureStoreEnvironment::Dev,
        )
        .unwrap();
        let device = DeviceId::new(identity.device_id).unwrap();
        append_memory_note(
            store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: root,
                brain_id: BRAIN,
                device_id: &device,
                classification: Classification::Public,
                text: "synthetic original prefix",
                event_id: None,
                config: &CanonicalWriterConfig::default(),
            },
        )
        .unwrap();
        let ready =
            load_existing_event_signing_key(store, SecureStoreEnvironment::Dev, root, &device)
                .unwrap();
        (device, ready.signing_key)
    }

    fn pair(&self, index: usize, anchors: [bool; 2], duplicate_event_id: bool) -> Vec<Vec<u8>> {
        // Deterministic transaction/nonces are test data, never production IDs.
        let transaction_id = format!("{:064x}", index + 1);
        let mut nonce_a = [0x31; 32];
        let mut nonce_b = [0x32; 32];
        nonce_a[..8].copy_from_slice(&u64::try_from(index).unwrap().to_le_bytes());
        nonce_b[..8].copy_from_slice(&u64::try_from(index).unwrap().to_le_bytes());
        let offer = cryptex_events::create_bootstrap_offer(
            BRAIN,
            &transaction_id,
            &self.devices[0],
            &["AAAAAAA"; 8].join("-"),
            &nonce_a,
            &self.keys[0],
        )
        .unwrap();
        let proof = cryptex_events::create_bootstrap_proof(
            &offer,
            &self.devices[1],
            &["BBBBBBB"; 8].join("-"),
            &nonce_b,
            &self.keys[1],
        )
        .unwrap();
        let request_a = cryptex_events::verify_bootstrap_approval_request(
            &[&offer, &proof],
            BootstrapApprovalRole::Initiator,
        )
        .unwrap();
        let approval_a =
            cryptex_events::sign_bootstrap_approval(&request_a, &self.keys[0]).unwrap();
        let request_b = cryptex_events::verify_bootstrap_approval_request(
            &[&offer, &proof, &approval_a],
            BootstrapApprovalRole::Responder,
        )
        .unwrap();
        let approval_b =
            cryptex_events::sign_bootstrap_approval(&request_b, &self.keys[1]).unwrap();
        let messages = vec![offer, proof, approval_a, approval_b];
        let event_a = format!("aaaaaaaa-aaaa-4aaa-8aaa-{index:012x}");
        let event_b = if duplicate_event_id {
            event_a.clone()
        } else {
            format!("bbbbbbbb-bbbb-4bbb-8bbb-{index:012x}")
        };
        for (side, request, prefix, event_id) in [
            (0, request_a, 2, event_a.as_str()),
            (1, request_b, 3, event_b.as_str()),
        ] {
            let journal = BootstrapJournalStore::new(&self.profiles[side]).unwrap();
            journal
                .begin_verified_approval_prefix(&request, &messages[..prefix], event_id)
                .unwrap();
            // Models the domain's already-confirmed owner input. Actual IPC
            // capability/connection authorization is tested separately.
            journal
                .record_local_approval_signature(&request, &messages[prefix])
                .unwrap();
            journal
                .record_verified_complete_transcript(&transaction_id, &messages)
                .unwrap();
            if anchors[side] {
                append_local_peer_approval_anchor(
                    &self.profiles[side],
                    BRAIN,
                    &self.store,
                    SecureStoreEnvironment::Dev,
                    &transaction_id,
                    &messages,
                )
                .unwrap();
            }
        }
        messages
    }

    fn import_peer_fixture(&self, segment: u64) {
        // Seal through the real daemon writer. Fixture transport then creates
        // one NEW destination under inventory/device locks, preserving bytes.
        // This tests import-visible reconciliation, not live Syncthing delivery.
        append_memory_note(
            &self.store,
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: &self.profiles[1],
                brain_id: BRAIN,
                device_id: &self.devices[1],
                classification: Classification::Public,
                text: "synthetic seal trigger",
                event_id: None,
                config: &CanonicalWriterConfig {
                    seal_event_threshold: 1,
                },
            },
        )
        .unwrap();
        let source = sealed_segment_path(&self.profiles[1], &self.devices[1], segment);
        let bytes = fs::read(&source).unwrap();
        let _locks = crate::canonical_writer::acquire_canonical_write_lock_set(
            &self.profiles[0],
            &self.devices,
        )
        .unwrap();
        let destination = sealed_segment_path(&self.profiles[0], &self.devices[1], segment);
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .unwrap();
        file.write_all(&bytes).unwrap();
        file.sync_all().unwrap();
        assert_eq!(fs::read(&source).unwrap(), bytes);
        assert_eq!(fs::read(&destination).unwrap(), bytes);
    }

    fn policy(&self) -> VerifiedTrustPolicySnapshot {
        let key = cryptex_events::signing_key_id_from_verifying_key(&self.keys[0].verifying_key())
            .unwrap();
        let inventory = read_canonical_device_inventory(&self.profiles[0], None).unwrap();
        scan_verified_trust_policy(
            &self.profiles[0],
            BRAIN,
            &self.devices[0],
            &key,
            &self.keys[0].verifying_key().to_bytes(),
            &inventory,
        )
        .unwrap()
    }
}

impl Drop for PairProfiles {
    fn drop(&mut self) {
        let root = fs::canonicalize(&self.root).unwrap();
        let temp = fs::canonicalize(std::env::temp_dir()).unwrap();
        assert!(root.starts_with(&temp) && root != temp);
        assert!(
            root.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("m8-pairing-recovery-")
        );
        fs::remove_dir_all(root).unwrap();
    }
}

fn canonical_bytes(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut result = BTreeMap::new();
    for entry in fs::read_dir(root.join("canonical/events")).unwrap() {
        let entry = entry.unwrap();
        if !entry.file_type().unwrap().is_dir() {
            continue;
        }
        for file in fs::read_dir(entry.path()).unwrap() {
            let file = file.unwrap();
            if matches!(
                file.path().extension().and_then(|s| s.to_str()),
                Some("seg" | "open")
            ) {
                result.insert(file.path(), fs::read(file.path()).unwrap());
            }
        }
    }
    result
}

fn vault_bytes(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, directory: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                visit(root, &entry.path(), files);
            } else {
                assert!(kind.is_file());
                files.insert(
                    entry.path().strip_prefix(root).unwrap().to_owned(),
                    fs::read(entry.path()).unwrap(),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

fn resolver(profiles: &PairProfiles) -> LocalDeviceKeyResolver {
    LocalDeviceKeyResolver::load(
        &profiles.profiles[0],
        BRAIN,
        &profiles.store,
        SecureStoreEnvironment::Dev,
    )
    .unwrap()
}

fn refresh(
    profiles: &PairProfiles,
    db: &mut ProjectionDb,
) -> KnowledgeMaterializationResult<KnowledgeRebuildReport> {
    let anchor = resolver(profiles).local_trust_anchor().unwrap();
    rebuild_knowledge_from_local_approval_history(&profiles.profiles[0], BRAIN, &anchor, db)
}

fn guarded_ids(profiles: &PairProfiles, db: &ProjectionDb) -> BTreeSet<String> {
    search_events_with_retrieval_safety(
        &profiles.profiles[0],
        BRAIN,
        &[],
        &mut resolver(profiles),
        db,
        &SearchQuery::default(),
    )
    .unwrap()
    .into_iter()
    .map(|hit| hit.event_id)
    .collect()
}

fn assert_stale_views_hold(
    profiles: &PairProfiles,
    db: &ProjectionDb,
    vault: &Path,
    excluded: &str,
) {
    let root = &profiles.profiles[0];
    let mut key = resolver(profiles);
    let expected = RetrievalSafetyHold::MaterializationStateMissingOrStale;
    assert!(
        matches!(search_events_with_retrieval_safety(root, BRAIN, &[], &mut key, db, &SearchQuery::default()),
        Err(GuardedSearchError::BrainWideHold(reason)) if reason == expected)
    );
    assert!(
        matches!(timeline_events_with_retrieval_safety_deadline(root, BRAIN, &[], &mut key, db,
        &TimelineQuery { limit: 50, ..TimelineQuery::default() }, None),
        Err(GuardedSearchError::BrainWideHold(reason)) if reason == expected)
    );
    assert!(
        matches!(event_detail_with_retrieval_safety_deadline(root, BRAIN, &[], &mut key, db, excluded, None),
        Err(GuardedSearchError::BrainWideHold(reason)) if reason == expected)
    );
    assert!(
        matches!(rebuild_vault_with_retrieval_safety(root, BRAIN, &[], &mut key, db, vault),
        Err(GuardedVaultError::BrainWideHold(reason)) if reason == expected)
    );
}

fn restriction_projection_recovery(cutoff: u64) {
    use crate::bootstrap_journal::{BootstrapState, RestrictionReason};

    let profiles = PairProfiles::new();
    profiles.pair(0, [true, true], false);
    profiles.import_peer_fixture(0);
    let root = &profiles.profiles[0];
    let (mut db, _) = crate::projection::open_projection_db(root).unwrap();
    refresh(&profiles, &mut db).unwrap();
    let original = search_events_with_retrieval_safety(
        root,
        BRAIN,
        &[],
        &mut resolver(&profiles),
        &db,
        &SearchQuery::default(),
    )
    .unwrap();
    assert_eq!(original.len(), 3);
    let excluded = original
        .iter()
        .find(|hit| hit.device_id == profiles.devices[1].as_str() && hit.device_seq == "3")
        .unwrap()
        .event_id
        .clone();
    let admitted = original
        .iter()
        .filter(|hit| {
            hit.device_id != profiles.devices[1].as_str()
                || hit.device_seq.parse::<u64>().unwrap() <= cutoff
        })
        .map(|hit| hit.event_id.clone())
        .collect::<BTreeSet<_>>();
    let vault = root.join("selected-vault");
    fs::create_dir(&vault).unwrap();
    fs::write(vault.join("owner-note.txt"), b"synthetic owner-owned note").unwrap();
    assert_eq!(
        rebuild_vault_with_retrieval_safety(
            root,
            BRAIN,
            &[],
            &mut resolver(&profiles),
            &db,
            &vault
        )
        .unwrap()
        .note_count,
        3
    );
    let old_vault = vault_bytes(&vault);
    let before = canonical_bytes(root);
    let transaction = "cd".repeat(32);
    let policy = profiles.policy();
    let request = policy
        .device_restriction_approval_snapshot(
            transaction.clone(),
            &profiles.devices[1],
            RestrictionReason::Compromised,
            cutoff,
        )
        .unwrap();
    drop(policy);
    let journal = BootstrapJournalStore::new(root).unwrap();
    let event_id = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
    journal
        .begin_owner_operation(request.clone(), event_id)
        .unwrap();
    journal
        .confirm_owner_operation(&transaction, &request.digest().unwrap())
        .unwrap();
    crate::canonical_writer::fail_after_append_before_tip();
    assert!(
        append_confirmed_owner_security_operation(
            root,
            BRAIN,
            &profiles.store,
            SecureStoreEnvironment::Dev,
            &transaction
        )
        .is_err()
    );
    let pending = journal.load().unwrap();
    let operation = pending
        .transactions
        .iter()
        .find(|t| t.snapshot.transaction_id == transaction)
        .unwrap();
    assert_eq!(operation.state, BootstrapState::AppendUnknown);
    assert_eq!(operation.append_attempts.len(), 1);
    let durable = canonical_bytes(root);
    assert_ne!(durable, before);
    for (path, bytes) in before {
        assert!(
            durable[&path].starts_with(&bytes),
            "canonical prefix must remain immutable"
        );
    }
    assert_stale_views_hold(&profiles, &db, &vault, &excluded);
    assert_eq!(vault_bytes(&vault), old_vault);
    drop(db);

    // Reopen the real on-disk SQLite DB and journal; this is domain restart,
    // not a claim of Desktop process or physical power-loss testing.
    reconcile_peer_approval_anchors_before_startup(
        root,
        &profiles.store,
        SecureStoreEnvironment::Dev,
    )
    .unwrap();
    let (mut db, _) = crate::projection::open_projection_db(root).unwrap();
    let recovered = journal.load().unwrap();
    let operation = recovered
        .transactions
        .iter()
        .find(|t| t.snapshot.transaction_id == transaction)
        .unwrap();
    assert_eq!(operation.state, BootstrapState::Anchored);
    assert_eq!(operation.append_attempts.len(), 1);
    assert_eq!(canonical_bytes(root), durable);
    assert_stale_views_hold(&profiles, &db, &vault, &excluded);

    // Fail the M4 transaction after M3 has committed the cutoff-aware rows.
    // A partially refreshed derived DB must still hold every guarded surface.
    db.connection().execute_batch("CREATE TRIGGER synthetic_m4_rebuild_failure BEFORE DELETE ON knowledge_materialization_state BEGIN SELECT RAISE(ABORT, 'synthetic M4 rebuild interruption'); END;").unwrap();
    let failure = refresh(&profiles, &mut db).unwrap_err();
    assert!(matches!(
        failure,
        KnowledgeMaterializationError::Knowledge(_)
    ));
    assert!(
        failure
            .to_string()
            .contains("synthetic M4 rebuild interruption")
    );
    assert!(
        event_detail(db.connection(), BRAIN, &excluded)
            .unwrap()
            .is_none(),
        "the preceding M3 transaction really committed"
    );
    assert_stale_views_hold(&profiles, &db, &vault, &excluded);
    assert_eq!(vault_bytes(&vault), old_vault);
    assert_eq!(canonical_bytes(root), durable);
    db.connection()
        .execute_batch("DROP TRIGGER synthetic_m4_rebuild_failure;")
        .unwrap();
    drop(db);

    for _ in 0..2 {
        let (mut db, _) = crate::projection::open_projection_db(root).unwrap();
        refresh(&profiles, &mut db).unwrap();
        assert_eq!(guarded_ids(&profiles, &db), admitted);
        let page = timeline_events_with_retrieval_safety_deadline(
            root,
            BRAIN,
            &[],
            &mut resolver(&profiles),
            &db,
            &TimelineQuery {
                limit: 50,
                ..TimelineQuery::default()
            },
            None,
        )
        .unwrap();
        assert_eq!(
            page.hits
                .into_iter()
                .map(|hit| hit.event_id)
                .collect::<BTreeSet<_>>(),
            admitted
        );
        for hit in &original {
            let detail = event_detail_with_retrieval_safety_deadline(
                root,
                BRAIN,
                &[],
                &mut resolver(&profiles),
                &db,
                &hit.event_id,
                None,
            )
            .unwrap();
            assert_eq!(detail.is_some(), admitted.contains(&hit.event_id));
        }
        assert_eq!(
            rebuild_vault_with_retrieval_safety(
                root,
                BRAIN,
                &[],
                &mut resolver(&profiles),
                &db,
                &vault
            )
            .unwrap()
            .note_count,
            admitted.len()
        );
        let notes = fs::read_dir(vault.join("Cryptex/notes"))
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                assert!(entry.file_type().unwrap().is_file());
                entry.file_name().to_string_lossy().into_owned()
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            notes,
            admitted.iter().map(|id| format!("{id}.md")).collect()
        );
        assert_eq!(
            fs::read(vault.join("owner-note.txt")).unwrap(),
            b"synthetic owner-owned note"
        );
        assert_eq!(canonical_bytes(root), durable);
        reconcile_peer_approval_anchors_before_startup(
            root,
            &profiles.store,
            SecureStoreEnvironment::Dev,
        )
        .unwrap();
        assert_eq!(canonical_bytes(root), durable);
        drop(db);
    }
}

#[test]
fn durable_zero_cutoff_holds_stale_views_and_reopens_after_partial_sqlite_rebuild() {
    restriction_projection_recovery(0);
}

#[test]
fn durable_prefix_cutoff_holds_stale_views_and_reopens_after_partial_sqlite_rebuild() {
    restriction_projection_recovery(1);
}

#[test]
fn paired_startup_refuses_torn_journal_and_missing_key_without_rewriting_evidence() {
    use crate::secure_store::{AccountLabel, ServiceLabel};

    let profiles = PairProfiles::new();
    profiles.pair(0, [true, true], false);
    profiles.import_peer_fixture(0);
    let root = &profiles.profiles[0];
    let path = root.join("bootstrap/pending.json");
    let good = fs::read(&path).unwrap();
    let original = canonical_bytes(root);
    let puts = profiles.store.put_count();
    let writes = crate::bootstrap_journal::JOURNAL_WRITES.with(|count| count.get());
    let mut trailing = good.clone();
    trailing.extend_from_slice(b"synthetic torn suffix");
    for torn in [Vec::new(), good[..good.len() / 2].to_vec(), trailing] {
        fs::write(&path, &torn).unwrap();
        assert!(matches!(
            reconcile_peer_approval_anchors_before_startup(
                root,
                &profiles.store,
                SecureStoreEnvironment::Dev
            ),
            Err(KnowledgeMaterializationError::Verification(_))
        ));
        assert_eq!(fs::read(&path).unwrap(), torn);
        assert_eq!(canonical_bytes(root), original);
        assert_eq!(profiles.store.put_count(), puts);
        assert_eq!(
            crate::bootstrap_journal::JOURNAL_WRITES.with(|count| count.get()),
            writes
        );
    }
    fs::write(&path, &good).unwrap();

    // Missing-key simulation at the SecureStore facade, not an OS credential
    // deletion/reinstall test. A new startup cannot mint over signed history.
    let service = ServiceLabel::device_credentials(SecureStoreEnvironment::Dev);
    let account = AccountLabel::device_event_signing_seed(profiles.devices[0].as_str());
    let seed = profiles
        .store
        .get(service.as_str(), account.as_str())
        .unwrap();
    let public_path = cryptex_storage::event_signing_key_path(root, &profiles.devices[0]);
    let public = fs::read(&public_path).unwrap();
    profiles
        .store
        .delete(service.as_str(), account.as_str())
        .unwrap();
    for _ in 0..2 {
        assert!(
            LocalDeviceKeyResolver::load(root, BRAIN, &profiles.store, SecureStoreEnvironment::Dev)
                .is_err()
        );
        assert!(matches!(
            reconcile_peer_approval_anchors_before_startup(
                root,
                &profiles.store,
                SecureStoreEnvironment::Dev
            ),
            Err(KnowledgeMaterializationError::Verification(_))
        ));
        assert_eq!(profiles.store.put_count(), puts);
        assert_eq!(fs::read(&public_path).unwrap(), public);
        assert_eq!(fs::read(&path).unwrap(), good);
        assert_eq!(canonical_bytes(root), original);
        assert_eq!(
            crate::bootstrap_journal::JOURNAL_WRITES.with(|count| count.get()),
            writes
        );
    }
    // Restore only this synthetic fixture's exact original seed. Recovery
    // must prove canonical authority again; journal rows are never sufficient.
    profiles
        .store
        .put(service.as_str(), account.as_str(), &seed)
        .unwrap();
    reconcile_peer_approval_anchors_before_startup(
        root,
        &profiles.store,
        SecureStoreEnvironment::Dev,
    )
    .unwrap();
    assert_eq!(canonical_bytes(root), original);
    assert_eq!(fs::read(&path).unwrap(), good);
}

#[test]
fn imported_bilateral_counterpart_allows_local_retry_and_startup_without_another_append() {
    let profiles = PairProfiles::new();
    let messages = profiles.pair(0, [true, true], false);
    profiles.import_peer_fixture(0);
    let root = &profiles.profiles[0];
    let before = canonical_bytes(root);
    let first = append_local_peer_approval_anchor(
        root,
        BRAIN,
        &profiles.store,
        SecureStoreEnvironment::Dev,
        &format!("{:064x}", 1),
        &messages,
    )
    .unwrap();
    assert_eq!(first.event_id, "aaaaaaaa-aaaa-4aaa-8aaa-000000000000");
    reconcile_peer_approval_anchors_before_startup(
        root,
        &profiles.store,
        SecureStoreEnvironment::Dev,
    )
    .unwrap();
    assert_eq!(canonical_bytes(root), before);
    let policy = profiles.policy();
    assert_eq!(
        policy
            .history
            .records()
            .iter()
            .filter(|record| matches!(record, AuthenticatedSecurityRecord::SecurityV3(_)))
            .count(),
        2
    );
    assert!(
        policy
            .self_retirement_approval_snapshot(format!("{:064x}", 1))
            .is_err(),
        "same-author cross-security-type transaction reuse must still refuse"
    );
}

#[test]
fn imported_counterpart_does_not_relax_global_event_id_uniqueness() {
    let profiles = PairProfiles::new();
    let messages = profiles.pair(0, [true, true], true);
    profiles.import_peer_fixture(0);
    let root = &profiles.profiles[0];
    let before = canonical_bytes(root);
    let journal = fs::read(root.join("bootstrap/pending.json")).unwrap();
    assert!(
        append_local_peer_approval_anchor(
            root,
            BRAIN,
            &profiles.store,
            SecureStoreEnvironment::Dev,
            &format!("{:064x}", 1),
            &messages
        )
        .is_err()
    );
    assert!(
        reconcile_peer_approval_anchors_before_startup(
            root,
            &profiles.store,
            SecureStoreEnvironment::Dev
        )
        .is_err()
    );
    assert_eq!(canonical_bytes(root), before);
    assert_eq!(
        fs::read(root.join("bootstrap/pending.json")).unwrap(),
        journal
    );
}

#[test]
fn an_already_trusted_peers_anchor_may_arrive_before_the_local_completion() {
    let profiles = PairProfiles::new();
    profiles.pair(0, [true, true], false);
    profiles.import_peer_fixture(0);
    let messages = profiles.pair(1, [false, true], false);
    profiles.import_peer_fixture(1);
    let transaction = format!("{:064x}", 2);
    let root = &profiles.profiles[0];
    let policy = profiles.policy();
    // Pure canonical namespace check, not permission to rebind a pending
    // journal ID to another operation. The journal still forbids that reuse.
    assert!(
        policy
            .self_retirement_approval_snapshot(transaction.clone())
            .is_ok()
    );
    drop(policy);
    let outcome = append_local_peer_approval_anchor(
        root,
        BRAIN,
        &profiles.store,
        SecureStoreEnvironment::Dev,
        &transaction,
        &messages,
    )
    .unwrap();
    assert_eq!(outcome.event_id, "aaaaaaaa-aaaa-4aaa-8aaa-000000000001");
    let before = canonical_bytes(root);
    reconcile_peer_approval_anchors_before_startup(
        root,
        &profiles.store,
        SecureStoreEnvironment::Dev,
    )
    .unwrap();
    assert_eq!(canonical_bytes(root), before);
}

#[test]
fn seventeen_real_anchors_reopen_with_one_scan_no_rewrite_and_no_duplicate_append() {
    let profiles = PairProfiles::new();
    profiles.pair(0, [true, true], false);
    profiles.import_peer_fixture(0);
    for index in 1..17 {
        // Same already-trusted peer, fresh transaction/nonces, actual signed
        // approval and daemon append. B retains 16 released pending rows.
        profiles.pair(index, [true, false], false);
    }
    let root = &profiles.profiles[0];
    let path = root.join("bootstrap/pending.json");
    let before = canonical_bytes(root);
    let journal_bytes = fs::read(&path).unwrap();
    let wire: serde_json::Value = serde_json::from_slice(&journal_bytes).unwrap();
    assert!(wire["transactions"].as_array().unwrap().is_empty());
    assert_eq!(wire["terminal_receipts"].as_array().unwrap().len(), 17);
    for _ in 0..2 {
        let scans = POLICY_SCAN_COUNT.with(|count| count.get());
        let writes = crate::bootstrap_journal::JOURNAL_WRITES.with(|count| count.get());
        let started = Instant::now();
        // Existing native M8 startup budget, local_startup.rs::prepare. This
        // measures domain reconciliation, not a complete Desktop launch.
        reconcile_peer_approval_anchors_before_startup(
            root,
            &profiles.store,
            SecureStoreEnvironment::Dev,
        )
        .unwrap();
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
        assert_eq!(POLICY_SCAN_COUNT.with(|count| count.get()) - scans, 1);
        assert_eq!(
            crate::bootstrap_journal::JOURNAL_WRITES.with(|count| count.get()),
            writes
        );
        let reopened = BootstrapJournalStore::new(root).unwrap().load().unwrap();
        assert_eq!(reopened.transactions.len(), 17);
        assert!(
            reopened
                .transactions
                .iter()
                .all(|t| t.state == crate::bootstrap_journal::BootstrapState::Anchored)
        );
        assert_eq!(fs::read(&path).unwrap(), journal_bytes);
        assert_eq!(canonical_bytes(root), before);
    }
    let policy = profiles.policy();
    assert_eq!(policy.history.records().iter().filter(|record| matches!(record,
        AuthenticatedSecurityRecord::SecurityV3(event) if event.device_id() == &profiles.devices[0])).count(), 17);
    drop(policy);

    // An old full anchor needs upgrade, while the final compact witness is
    // forged. Failure must not commit even the earlier valid upgrade.
    let journal = BootstrapJournalStore::new(root).unwrap();
    journal
        .update(|pending| {
            pending.transactions[0].verified_anchor = None;
            Ok(())
        })
        .unwrap();
    let mut altered: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    altered["terminal_receipts"][15]["verified_anchor"]["operation_digest"] =
        serde_json::Value::String("ff".repeat(32));
    let forged = serde_json::to_vec(&altered).unwrap();
    fs::write(&path, &forged).unwrap();
    let writes = crate::bootstrap_journal::JOURNAL_WRITES.with(|count| count.get());
    assert!(
        reconcile_peer_approval_anchors_before_startup(
            root,
            &profiles.store,
            SecureStoreEnvironment::Dev
        )
        .is_err()
    );
    assert_eq!(
        crate::bootstrap_journal::JOURNAL_WRITES.with(|count| count.get()),
        writes
    );
    assert_eq!(fs::read(&path).unwrap(), forged);
    assert_eq!(canonical_bytes(root), before);
}
