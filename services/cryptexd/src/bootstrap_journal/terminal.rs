//! Closed persisted journal v3. Existing callers receive a unified view so
//! terminal IDs cannot disappear from any lookup, retry or collision check.
//! No canonical locks or I/O may be acquired by this codec.

use super::*;
use serde::{Deserializer, Serializer};

// Every valid receipt contains >256 bytes of mandatory IDs/field names alone.
// This pre-hydration resource bound cannot reduce valid one-MiB wire capacity.
const MAX_RECORDS: usize = MAX_JOURNAL_BYTES / 256;
const MAX_UNIFIED_BYTES: usize = 2 * MAX_JOURNAL_BYTES;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedAnchor {
    pub attempt: AppendAttempt,
    pub operation_digest: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireJournal {
    schema_version: u32,
    transactions: Vec<PendingBootstrapTransaction>,
    exchanges: Vec<PendingBootstrapExchange>,
    terminal_receipts: Vec<TerminalReceipt>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum TerminalReceipt {
    Cancelled {
        snapshot: ApprovalSnapshot,
        approval_request_digest: String,
        stable_event_id: String,
        local_approval_signature_generated: bool,
    },
    Anchored {
        snapshot: ApprovalSnapshot,
        approval_request_digest: String,
        stable_event_id: String,
        transcript_messages: Vec<String>,
        transcript_hash: Option<String>,
        local_approval_signature_generated: bool,
        local_approval_signature_released: bool,
        verified_anchor: VerifiedAnchor,
    },
}

#[derive(Serialize)]
struct WireJournalRef<'a> {
    schema_version: u32,
    transactions: Vec<&'a PendingBootstrapTransaction>,
    exchanges: &'a [PendingBootstrapExchange],
    terminal_receipts: Vec<TerminalReceiptRef<'a>>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum TerminalReceiptRef<'a> {
    Cancelled {
        snapshot: &'a ApprovalSnapshot,
        approval_request_digest: &'a str,
        stable_event_id: &'a str,
        local_approval_signature_generated: bool,
    },
    Anchored {
        snapshot: &'a ApprovalSnapshot,
        approval_request_digest: &'a str,
        stable_event_id: &'a str,
        transcript_messages: &'a [String],
        transcript_hash: &'a Option<String>,
        local_approval_signature_generated: bool,
        local_approval_signature_released: bool,
        verified_anchor: &'a VerifiedAnchor,
    },
}

pub(super) fn compactable(transaction: &PendingBootstrapTransaction) -> bool {
    match transaction.state {
        BootstrapState::Cancelled => {
            !transaction.local_approval_signature_released
                && transaction.append_attempts.is_empty()
                && transaction.verified_anchor.is_none()
        }
        BootstrapState::Anchored => transaction.verified_anchor.is_some(),
        _ => false,
    }
}

impl<'a> TerminalReceiptRef<'a> {
    fn from_transaction(transaction: &'a PendingBootstrapTransaction) -> Result<Self, &'static str> {
        if !compactable(transaction) {
            return Err("nonterminal or uncertain transaction cannot become a terminal receipt");
        }
        Ok(match transaction.state {
            BootstrapState::Cancelled => Self::Cancelled {
                snapshot: &transaction.snapshot,
                approval_request_digest: &transaction.approval_request_digest,
                stable_event_id: &transaction.stable_event_id,
                local_approval_signature_generated: transaction.local_approval_signature_generated,
            },
            BootstrapState::Anchored => Self::Anchored {
                snapshot: &transaction.snapshot,
                approval_request_digest: &transaction.approval_request_digest,
                stable_event_id: &transaction.stable_event_id,
                transcript_messages: &transaction.transcript_messages,
                transcript_hash: &transaction.transcript_hash,
                local_approval_signature_generated: transaction.local_approval_signature_generated,
                local_approval_signature_released: transaction.local_approval_signature_released,
                verified_anchor: transaction.verified_anchor.as_ref().expect("compactable anchor checked"),
            },
            _ => unreachable!("compactable state checked"),
        })
    }
}

impl TerminalReceipt {
    fn into_transaction(self) -> PendingBootstrapTransaction {
        match self {
            Self::Cancelled {
                snapshot,
                approval_request_digest,
                stable_event_id,
                local_approval_signature_generated,
            } => PendingBootstrapTransaction {
                snapshot,
                approval_request_digest,
                stable_event_id,
                state: BootstrapState::Cancelled,
                transcript_messages: Vec::new(),
                transcript_hash: None,
                local_approval_signature_generated,
                local_approval_signature_released: false,
                append_attempts: Vec::new(),
                verified_anchor: None,
            },
            Self::Anchored {
                snapshot,
                approval_request_digest,
                stable_event_id,
                transcript_messages,
                transcript_hash,
                local_approval_signature_generated,
                local_approval_signature_released,
                verified_anchor,
            } => PendingBootstrapTransaction {
                snapshot,
                approval_request_digest,
                stable_event_id,
                state: BootstrapState::Anchored,
                transcript_messages,
                transcript_hash,
                local_approval_signature_generated,
                local_approval_signature_released,
                append_attempts: vec![verified_anchor.attempt.clone()],
                verified_anchor: Some(verified_anchor),
            },
        }
    }
}

impl Serialize for PendingBootstrapJournal {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut active = Vec::new();
        let mut receipts = Vec::new();
        for transaction in &self.transactions {
            if compactable(transaction) {
                receipts.push(TerminalReceiptRef::from_transaction(transaction)
                    .map_err(serde::ser::Error::custom)?);
            } else {
                active.push(transaction);
            }
        }
        WireJournalRef {
            schema_version: self.schema_version,
            transactions: active,
            exchanges: &self.exchanges,
            terminal_receipts: receipts,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PendingBootstrapJournal {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = WireJournal::deserialize(deserializer)?;
        if wire.schema_version != JOURNAL_SCHEMA_VERSION
            || wire.transactions.len() + wire.exchanges.len() > MAX_PENDING_TRANSACTIONS
            || wire.terminal_receipts.len() + wire.transactions.len() + wire.exchanges.len() > MAX_RECORDS
            || wire.transactions.iter().any(compactable)
        {
            return Err(serde::de::Error::custom("invalid journal v3 collection bounds or placement"));
        }
        let mut transactions = wire.transactions;
        transactions.extend(wire.terminal_receipts.into_iter().map(TerminalReceipt::into_transaction));
        transactions.sort_by(|left, right| {
            left.snapshot.transaction_id.as_bytes().cmp(right.snapshot.transaction_id.as_bytes())
        });
        let journal = Self { schema_version: JOURNAL_SCHEMA_VERSION, transactions, exchanges: wire.exchanges };
        validate_view_bounds(&journal).map_err(serde::de::Error::custom)?;
        // Caller validates closed snapshots, receipt eligibility and all ID
        // collisions before this view can be used. No deduplication occurs.
        Ok(journal)
    }
}

pub(super) fn validate_view_bounds(journal: &PendingBootstrapJournal) -> Result<(), BootstrapJournalError> {
    if journal.transactions.len() + journal.exchanges.len() > MAX_RECORDS {
        return Err(BootstrapJournalError::Limit("journal record resource bound"));
    }
    // Bound the expanded unified representation before transcript signature
    // verification. It is not the persisted payload or a second lifetime cap.
    let expanded = serde_json::to_vec(&(&journal.transactions, &journal.exchanges))
        .map_err(|_| BootstrapJournalError::Corrupt("journal view serialization failed"))?;
    if expanded.len() > MAX_UNIFIED_BYTES {
        return Err(BootstrapJournalError::Limit("expanded journal resource bound"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cryptex_events::{ContentHashHex, DeviceSeq, SigningKeyId};
    use ed25519_dalek::SigningKey;
    use std::sync::OnceLock;

    fn pending(index: usize) -> PendingBootstrapTransaction {
        static SNAPSHOT: OnceLock<ApprovalSnapshot> = OnceLock::new();
        let mut snapshot = SNAPSHOT.get_or_init(|| {
            let data: Value = serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../crates/events/tests/fixtures/m8-protocol-v1.json"
            ))).unwrap();
            serde_json::from_value(data["approval_snapshots"][3]["snapshot"].clone()).unwrap()
        }).clone();
        snapshot.transaction_id = format!("{:064x}", index + 1);
        PendingBootstrapTransaction::new(snapshot,
            format!("77777777-7777-4777-8777-{index:012x}")).unwrap()
    }

    fn uncertain(index: usize) -> (PendingBootstrapTransaction, cryptex_events::SecurityEventV3) {
        let mut transaction = pending(index);
        let snapshot = &transaction.snapshot;
        let key = SigningKey::from_bytes(&[0x11; 32]);
        let previous = ContentHashHex::parse(snapshot.last_authorized_content_hash.as_deref().unwrap()).unwrap();
        let raw = cryptex_events::security_v3::sign_device_self_retired_event(
            &transaction.stable_event_id, &snapshot.brain_id,
            &DeviceId::new(snapshot.local_identity.device_id.clone()).unwrap(),
            DeviceSeq::from_u64(3).unwrap(), "2026-01-01T00:00:00Z",
            &SigningKeyId::parse(&snapshot.local_identity.signing_key_id).unwrap(),
            &previous, &snapshot.transaction_id, 2, &previous, &key,
        ).unwrap();
        let event = cryptex_events::read_security_v3_event(&raw, &key.verifying_key()).unwrap();
        transaction.transition(BootstrapState::OwnerApproved).unwrap();
        transaction.record_append_attempt(AppendAttempt {
            reserved_sequence: 3,
            predecessor_hash: previous.as_str().to_owned(),
            candidate_content_hash: Some(event.content_hash().as_str().to_owned()),
        }).unwrap();
        (transaction, event)
    }

    fn cancelled(index: usize) -> PendingBootstrapTransaction {
        let mut transaction = pending(index);
        transaction.transition(BootstrapState::Cancelled).unwrap();
        transaction
    }

    fn store() -> (PathBuf, BootstrapJournalStore) {
        let root = crate::test_temp::exclusive_temp_dir("m8-terminal-journal");
        let store = BootstrapJournalStore::new(&root).unwrap();
        (root, store)
    }

    fn wire(root: &Path) -> Value {
        serde_json::from_slice(&fs::read(root.join(JOURNAL_DIR).join(JOURNAL_FILE)).unwrap()).unwrap()
    }

    fn cleanup(root: &Path) {
        let resolved = fs::canonicalize(root).unwrap();
        let temp = fs::canonicalize(std::env::temp_dir()).unwrap();
        assert!(resolved.starts_with(&temp) && resolved != temp);
        assert!(resolved.file_name().unwrap().to_string_lossy().starts_with("m8-terminal-journal-"));
        fs::remove_dir_all(resolved).unwrap();
    }

    #[test]
    fn sixteen_cancelled_receipts_allow_new_operation_and_permanently_refuse_reuse() {
        let (root, store) = store();
        store.update(|journal| {
            for index in 0..16 { journal.insert(cancelled(index))?; }
            journal.insert(pending(16))
        }).unwrap();
        let on_disk = wire(&root);
        assert_eq!(on_disk["schema_version"], 3);
        assert_eq!(on_disk["terminal_receipts"].as_array().unwrap().len(), 16);
        assert_eq!(on_disk["transactions"].as_array().unwrap().len(), 1);
        let loaded = store.load().unwrap();
        assert_eq!(loaded.transactions.len(), 17);
        assert_eq!(loaded.active_count(), 1);
        let before = fs::read(root.join(JOURNAL_DIR).join(JOURNAL_FILE)).unwrap();
        assert!(matches!(store.update(|journal| journal.insert(pending(0))), Err(BootstrapJournalError::Conflict(_))));
        assert_eq!(fs::read(root.join(JOURNAL_DIR).join(JOURNAL_FILE)).unwrap(), before);
        store.update(|_| Ok(())).unwrap();
        assert_eq!(store.load().unwrap(), loaded);
        cleanup(&root);
    }

    #[test]
    fn sixteen_verified_anchors_compact_but_unknown_and_legacy_evidence_do_not() {
        let (root, store) = store();
        store.update(|journal| {
            for index in 0..16 {
                let (mut transaction, event) = uncertain(index);
                // This unit fixture tests the codec; production callers require
                // a canonical-locked reread, exercised by existing integration tests.
                transaction.record_verified_anchor_event(&event)?;
                journal.insert(transaction)?;
            }
            journal.insert(uncertain(16).0)?;
            let (mut legacy, event) = uncertain(17);
            legacy.record_verified_anchor(3, event.content_hash().as_str())?;
            journal.insert(legacy)
        }).unwrap();
        let on_disk = wire(&root);
        assert_eq!(on_disk["terminal_receipts"].as_array().unwrap().len(), 16);
        assert_eq!(on_disk["transactions"].as_array().unwrap().len(), 2);
        let loaded = store.load().unwrap();
        assert_eq!(loaded.active_count(), 2);
        assert_eq!(loaded.transactions[16].state, BootstrapState::AppendUnknown);
        assert!(loaded.transactions[17].verified_anchor.is_none());
        assert_eq!(loaded.transactions[16].append_attempts.len(), 1);
        cleanup(&root);
    }

    #[test]
    fn forged_anchor_witness_cannot_replace_current_canonical_metadata() {
        let (root, store) = store();
        let (mut transaction, event) = uncertain(0);
        transaction.record_verified_anchor_event(&event).unwrap();
        let id = transaction.snapshot.transaction_id.clone();
        store.update(|journal| journal.insert(transaction)).unwrap();
        let mut corrupt = wire(&root);
        corrupt["terminal_receipts"][0]["verified_anchor"]["operation_digest"] = Value::String("ff".repeat(32));
        let path = root.join(JOURNAL_DIR).join(JOURNAL_FILE);
        let bytes = serde_json::to_vec(&corrupt).unwrap();
        fs::write(&path, &bytes).unwrap();
        assert!(store.load().is_ok(), "a structurally valid receipt is data, not canonical proof");
        assert!(matches!(store.record_verified_anchor_event(&id, &event), Err(BootstrapJournalError::Conflict(_))));
        assert_eq!(fs::read(&path).unwrap(), bytes);
        cleanup(&root);
    }

    #[test]
    fn wire_and_unified_views_reject_cross_collection_identity_collisions() {
        let mut journal = PendingBootstrapJournal::default();
        journal.insert(cancelled(0)).unwrap();
        let mut encoded = serde_json::to_value(&journal).unwrap();
        encoded["transactions"] = serde_json::json!([pending(0)]);
        let decoded: PendingBootstrapJournal = serde_json::from_value(encoded).unwrap();
        assert!(matches!(decoded.validate(), Err(BootstrapJournalError::Conflict("duplicate transaction ID"))));
        let mut encoded = serde_json::to_value(&journal).unwrap();
        let mut other = pending(1);
        other.stable_event_id = journal.transactions[0].stable_event_id.clone();
        encoded["transactions"] = serde_json::json!([other]);
        let decoded: PendingBootstrapJournal = serde_json::from_value(encoded).unwrap();
        assert!(matches!(decoded.validate(), Err(BootstrapJournalError::Conflict("duplicate stable event ID"))));
    }

    #[test]
    fn terminal_bytes_are_bounded_and_capacity_failure_preserves_committed_file() {
        let (root, store) = store();
        store.update(|journal| journal.insert(cancelled(0))).unwrap();
        let before = fs::read(root.join(JOURNAL_DIR).join(JOURNAL_FILE)).unwrap();
        let result = store.update(|journal| {
            for index in 1..2000 { journal.transactions.push(cancelled(index)); }
            Ok(())
        });
        assert_eq!(result, Err(BootstrapJournalError::TerminalHistoryCapacity));
        assert_eq!(fs::read(root.join(JOURNAL_DIR).join(JOURNAL_FILE)).unwrap(), before);
        assert_eq!(store.load().unwrap().transactions.len(), 1);
        cleanup(&root);
    }

    #[test]
    fn legacy_v2_is_validated_then_migrates_only_on_successful_atomic_update() {
        let (root, store) = store();
        store.load().unwrap();
        let (mut old, event) = uncertain(0);
        old.record_verified_anchor(3, event.content_hash().as_str()).unwrap();
        let legacy = PendingBootstrapJournalV2 {
            schema_version: EXCHANGE_JOURNAL_SCHEMA_VERSION,
            transactions: vec![old.clone()], exchanges: Vec::new(),
        };
        let path = root.join(JOURNAL_DIR).join(JOURNAL_FILE);
        let original = serde_json::to_vec(&legacy).unwrap();
        fs::write(&path, &original).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.transactions, vec![old]);
        assert_eq!(loaded.active_count(), 1, "legacy anchor has no fresh canonical witness");
        assert_eq!(fs::read(&path).unwrap(), original, "load does not claim durable migration");
        store.update(|_| Ok(())).unwrap();
        assert_eq!(wire(&root)["schema_version"], 3);
        assert!(wire(&root)["terminal_receipts"].as_array().unwrap().is_empty());
        store.record_verified_anchor_event(&loaded.transactions[0].snapshot.transaction_id, &event).unwrap();
        assert_eq!(wire(&root)["terminal_receipts"].as_array().unwrap().len(), 1);
        assert_eq!(store.load().unwrap().active_count(), 0);
        // Do not relax legacy limits while migrating terminal rows.
        let invalid = PendingBootstrapJournalV2 {
            schema_version: EXCHANGE_JOURNAL_SCHEMA_VERSION,
            transactions: (0..17).map(cancelled).collect(), exchanges: Vec::new(),
        };
        let bytes = serde_json::to_vec(&invalid).unwrap();
        fs::write(&path, &bytes).unwrap();
        assert!(matches!(store.load(), Err(BootstrapJournalError::Limit(_))));
        assert_eq!(fs::read(&path).unwrap(), bytes);
        cleanup(&root);
    }

    #[test]
    fn released_cancellation_stays_full_but_unreleased_prefix_becomes_a_tombstone() {
        let data: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../crates/events/tests/fixtures/m8-protocol-v1.json"
        ))).unwrap();
        let raw: Vec<Vec<u8>> = data["bootstrap"]["messages"].as_array().unwrap().iter()
            .map(|message| message["wire"].as_str().unwrap().as_bytes().to_vec()).collect();
        let request = cryptex_events::verify_bootstrap_approval_request(
            &[raw[0].as_slice(), raw[1].as_slice()], cryptex_events::BootstrapApprovalRole::Initiator).unwrap();
        let snapshot = ApprovalSnapshot::from_verified_bootstrap_request(&request).unwrap();
        let mut transaction = PendingBootstrapTransaction::new(snapshot,
            "77777777-7777-4777-8777-999999999999".into()).unwrap();
        transaction.record_verified_approval_prefix(&request, &raw[..2]).unwrap();
        let mut before_release = transaction.clone();
        before_release.transition(BootstrapState::Cancelled).unwrap();
        // Preserve generated-but-unreleased evidence without inventing release.
        before_release.local_approval_signature_generated = true;
        let mut journal = PendingBootstrapJournal::default();
        journal.insert(before_release).unwrap();
        let encoded = serde_json::to_value(&journal).unwrap();
        assert_eq!(encoded["terminal_receipts"][0]["local_approval_signature_generated"], true);
        let restored: PendingBootstrapJournal = serde_json::from_value(encoded).unwrap();
        restored.validate().unwrap();
        assert!(restored.transactions[0].local_approval_signature_generated);
        assert!(!restored.transactions[0].local_approval_signature_released);
        assert!(restored.transactions[0].transcript_messages.is_empty());
        assert!(restored.transactions[0].transcript_hash.is_none());

        transaction.record_local_approval_signature(&request, &raw[2]).unwrap();
        transaction.transition(BootstrapState::CancelledAfterRelease).unwrap();
        let mut journal = PendingBootstrapJournal::default();
        journal.insert(transaction.clone()).unwrap();
        let encoded = serde_json::to_value(&journal).unwrap();
        assert!(encoded["terminal_receipts"].as_array().unwrap().is_empty());
        let restored: PendingBootstrapJournal = serde_json::from_value(encoded).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored.transactions, vec![transaction]);
        assert_eq!(restored.active_count(), 1);
    }

    #[test]
    fn concurrent_stores_preserve_all_terminal_ids_and_the_active_limit() {
        let (root, store) = store();
        let mut threads = Vec::new();
        for worker in 0..4 {
            let root = root.clone();
            threads.push(std::thread::spawn(move || {
                let store = BootstrapJournalStore::new(&root).unwrap();
                store.update(|journal| {
                    for index in worker * 8..(worker + 1) * 8 { journal.insert(cancelled(index))?; }
                    Ok(())
                }).unwrap();
            }));
        }
        for thread in threads { thread.join().unwrap(); }
        assert_eq!(store.load().unwrap().transactions.len(), 32);
        store.update(|journal| {
            for index in 32..48 { journal.insert(pending(index))?; }
            Ok(())
        }).unwrap();
        let path = root.join(JOURNAL_DIR).join(JOURNAL_FILE);
        let before = fs::read(&path).unwrap();
        assert!(matches!(store.update(|journal| journal.insert(pending(48))), Err(BootstrapJournalError::Limit(_))));
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(wire(&root)["terminal_receipts"].as_array().unwrap().len(), 32);
        assert_eq!(store.load().unwrap().active_count(), 16);
        cleanup(&root);
    }

    #[test]
    fn pre_replace_faults_preserve_previous_receipts_and_clean_only_staging() {
        for stage in [JournalWriteFault::Write, JournalWriteFault::Flush, JournalWriteFault::Replace] {
            let (root, store) = store();
            store.update(|journal| journal.insert(cancelled(0))).unwrap();
            let path = root.join(JOURNAL_DIR).join(JOURNAL_FILE);
            let before = fs::read(&path).unwrap();
            JOURNAL_WRITE_FAULT.with(|fault| fault.set(Some(stage)));
            assert!(matches!(store.update(|journal| journal.insert(pending(1))), Err(BootstrapJournalError::Io(_))));
            assert_eq!(fs::read(&path).unwrap(), before);
            assert_eq!(fs::read_dir(root.join(JOURNAL_DIR)).unwrap().count(), 2);
            assert_eq!(store.load().unwrap().transactions.len(), 1);
            assert!(!root.join("canonical").exists());
            cleanup(&root);
        }
    }

    #[test]
    fn post_replace_faults_report_durability_uncertainty_without_false_rollback() {
        let stages = [JournalWriteFault::Reopen, JournalWriteFault::ReopenSync,
            #[cfg(unix)] JournalWriteFault::ParentSync];
        for stage in stages {
            let (root, store) = store();
            store.update(|journal| journal.insert(cancelled(0))).unwrap();
            JOURNAL_WRITE_FAULT.with(|fault| fault.set(Some(stage)));
            assert!(matches!(store.update(|journal| journal.insert(pending(1))), Err(BootstrapJournalError::Durability(_))));
            let visible = store.load().unwrap();
            assert_eq!(visible.transactions.len(), 2);
            assert_eq!(visible.transactions[0].state, BootstrapState::Cancelled);
            assert_eq!(visible.transactions[1].state, BootstrapState::OwnerPending);
            assert_eq!(fs::read_dir(root.join(JOURNAL_DIR)).unwrap().count(), 2);
            store.update(|_| Ok(())).unwrap();
            assert_eq!(store.load().unwrap(), visible);
            assert!(!root.join("canonical").exists());
            cleanup(&root);
        }
    }

    struct Workers(Vec<std::process::Child>);

    impl Drop for Workers {
        fn drop(&mut self) {
            for child in &mut self.0 {
                if child.try_wait().ok().flatten().is_none() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }
    }

    #[test]
    fn cross_process_worker_entry() {
        let Some(root) = std::env::var_os("CRYPTEX_TEST_TERMINAL_ROOT") else { return; };
        let root = fs::canonicalize(PathBuf::from(root)).unwrap();
        let temp = fs::canonicalize(std::env::temp_dir()).unwrap();
        assert!(root.starts_with(temp));
        assert!(root.file_name().unwrap().to_string_lossy().starts_with("m8-terminal-journal-"));
        let worker: usize = std::env::var("CRYPTEX_TEST_TERMINAL_WORKER").unwrap().parse().unwrap();
        assert!(worker < 4);
        let store = BootstrapJournalStore::new(&root).unwrap();
        store.update(|journal| {
            for index in worker * 8..(worker + 1) * 8 { journal.insert(cancelled(index))?; }
            Ok(())
        }).unwrap();
    }

    #[test]
    fn separate_processes_serialize_receipt_updates_without_lost_cancellations() {
        let (root, store) = store();
        let mut workers = Workers(Vec::new());
        for index in 0..4 {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command.args(["--exact", "bootstrap_journal::terminal::tests::cross_process_worker_entry"])
                .env("CRYPTEX_TEST_TERMINAL_ROOT", &root)
                .env("CRYPTEX_TEST_TERMINAL_WORKER", index.to_string())
                .current_dir(&root)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            #[cfg(windows)] {
                use std::os::windows::process::CommandExt;
                command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
            }
            workers.0.push(command.spawn().unwrap());
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        for child in &mut workers.0 {
            loop {
                if let Some(status) = child.try_wait().unwrap() {
                    assert!(status.success(), "journal worker failed");
                    break;
                }
                assert!(std::time::Instant::now() < deadline, "journal worker exceeded shared deadline");
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        let loaded = store.load().unwrap();
        assert_eq!(loaded.transactions.len(), 32);
        assert_eq!(loaded.active_count(), 0);
        assert_eq!(wire(&root)["terminal_receipts"].as_array().unwrap().len(), 32);
        cleanup(&root);
    }
}
