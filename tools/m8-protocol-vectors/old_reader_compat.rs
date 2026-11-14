//! Copied as an integration test into the exact historical product checkout.
//! Its library/dependencies remain unchanged. All data is public synthetic.
use cryptex_core::DeviceId;
use cryptex_events::{TrustedDaemonKeyResolver, VersionedRecord, read_versioned_record, validate_accepted_record_bytes};
use cryptex_projection::{ProjectionDb, SearchQuery, TimelineQuery, project_memory_note};
use cryptexd::knowledge_materialization::*;
use ed25519_dalek::{SigningKey, VerifyingKey};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const SOURCE: &str = "ae61603b517f68b63d559ccb5ee77d744176e6a6";

struct Resolver { device: String, key_id: String, key: VerifyingKey }
impl TrustedDaemonKeyResolver for Resolver {
    fn trusted_daemon_key(&mut self, device: &str, key_id: &str) -> Option<VerifyingKey> {
        (device == self.device && key_id == self.key_id).then_some(self.key)
    }
}

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let root = fs::canonicalize(&self.0).unwrap();
        let temp = fs::canonicalize(std::env::temp_dir()).unwrap();
        assert!(root.starts_with(&temp) && root != temp);
        assert!(root.file_name().unwrap().to_string_lossy().starts_with("cryptex-old-reader-"));
        fs::remove_dir_all(root).unwrap();
    }
}

fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(base: &Path, directory: &Path, result: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            assert!(!kind.is_symlink());
            if kind.is_dir() { visit(base, &entry.path(), result); }
            else { result.insert(entry.path().strip_prefix(base).unwrap().to_owned(), fs::read(entry.path()).unwrap()); }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}

fn database_state(db: &ProjectionDb) -> BTreeMap<String, Vec<Vec<String>>> {
    let names = db.connection().prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name").unwrap()
        .query_map([], |row| row.get::<_, String>(0)).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
    let mut result = BTreeMap::new();
    for name in names {
        let mut statement = db.connection().prepare(&format!("SELECT * FROM \"{}\"", name.replace('"', "\"\""))).unwrap();
        let count = statement.column_count();
        let mut rows = statement.query_map([], |row| {
            Ok((0..count).map(|index| format!("{:?}", row.get_ref(index).unwrap())).collect::<Vec<_>>())
        }).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
        rows.sort();
        result.insert(name, rows);
    }
    result
}

#[test]
fn old_product_holds_signed_v3_without_mutation() {
    assert_eq!(std::env::var("CRYPTEX_OLD_READER_SOURCE").unwrap(), SOURCE);
    let data: Value = serde_json::from_str(include_str!("m8_old_reader_fixture.json")).unwrap();
    let root = std::env::temp_dir().join(format!("cryptex-old-reader-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir(&root).unwrap();
    let _fixture = Fixture(root.clone());
    let runtime = root.join("runtime");
    let vault = root.join("vault");
    let brain = data["brain_id"].as_str().unwrap();
    let device = DeviceId::new(data["device_id"].as_str().unwrap()).unwrap();
    let devices = [device.clone()];
    let key = SigningKey::from_bytes(&[0x11; 32]).verifying_key();
    let mut resolver = Resolver { device: device.as_str().into(),
        key_id: cryptex_events::signing_key_id_from_verifying_key(&key).unwrap().as_str().into(), key };
    let baseline = data["baseline_wire"].as_str().unwrap().as_bytes();
    let security = data["v3_wire"].as_str().unwrap().as_bytes();
    // This is a daemon-owned isolated fixture writer, not an application client.
    cryptex_storage::append_frame(&runtime, &device, 0, baseline).unwrap();
    let accepted = validate_accepted_record_bytes(baseline, &key).unwrap();
    let mut db = ProjectionDb::open_in_memory().unwrap();
    project_memory_note(db.connection(), &accepted).unwrap();
    rebuild_knowledge_from_canonical(&runtime, brain, &devices, &mut resolver, &mut db).unwrap();
    db.connection().execute_batch("CREATE TABLE compatibility_sentinel(value TEXT NOT NULL); INSERT INTO compatibility_sentinel VALUES('keep');").unwrap();
    assert!(evaluate_retrieval_safety(&runtime, brain, &devices, &mut resolver, &db).allows_content());
    let query = SearchQuery { text: Some("Synthetic".into()), limit: Some(10), ..SearchQuery::default() };
    let timeline = TimelineQuery { limit: 10, ..TimelineQuery::default() };
    let hits = search_events_with_retrieval_safety(&runtime, brain, &devices, &mut resolver, &db, &query).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].event_id, data["event_id"].as_str().unwrap());
    assert_eq!(timeline_events_with_retrieval_safety_deadline(&runtime, brain, &devices, &mut resolver, &db, &timeline, None).unwrap().hits.len(), 1);
    assert!(event_detail_with_retrieval_safety_deadline(&runtime, brain, &devices, &mut resolver, &db, data["event_id"].as_str().unwrap(), None).unwrap().is_some());
    rebuild_vault_with_retrieval_safety(&runtime, brain, &devices, &mut resolver, &db, &vault).unwrap();
    fs::write(vault.join("owner-sentinel.txt"), b"keep\n").unwrap();
    let pack = cryptex_storage::layout::active_segment_path(&runtime, &device, 0);
    let old_prefix = fs::read(&pack).unwrap();
    cryptex_storage::append_frame(&runtime, &device, 0, security).unwrap();
    let after_append = fs::read(&pack).unwrap();
    assert!(after_append.starts_with(&old_prefix));
    let VersionedRecord::Unverifiable(envelope) = read_versioned_record(security, &key).unwrap() else { panic!("old dispatcher unexpectedly accepts v3"); };
    assert_eq!(envelope.schema_version, Some(3));
    assert_eq!(envelope.raw_record, security);
    let db_before = database_state(&db);
    let vault_before = files(&vault);
    for _ in 0..2 {
        let safety = evaluate_retrieval_safety(&runtime, brain, &devices, &mut resolver, &db);
        assert!(!safety.allows_content());
        assert_eq!(safety.hold_reason(), Some(RetrievalSafetyHold::CanonicalVerificationUnavailable));
        drop(safety); // Holds canonical locks; never retain across the next read.
        assert!(matches!(search_events_with_retrieval_safety(&runtime, brain, &devices, &mut resolver, &db, &query),
            Err(GuardedSearchError::BrainWideHold(RetrievalSafetyHold::CanonicalVerificationUnavailable))));
        assert!(matches!(timeline_events_with_retrieval_safety_deadline(&runtime, brain, &devices, &mut resolver, &db, &timeline, None),
            Err(GuardedSearchError::BrainWideHold(RetrievalSafetyHold::CanonicalVerificationUnavailable))));
        assert!(matches!(event_detail_with_retrieval_safety_deadline(&runtime, brain, &devices, &mut resolver, &db, data["event_id"].as_str().unwrap(), None),
            Err(GuardedSearchError::BrainWideHold(RetrievalSafetyHold::CanonicalVerificationUnavailable))));
        assert!(matches!(rebuild_vault_with_retrieval_safety(&runtime, brain, &devices, &mut resolver, &db, &vault),
            Err(GuardedVaultError::BrainWideHold(RetrievalSafetyHold::CanonicalVerificationUnavailable))));
        match rebuild_knowledge_from_canonical(&runtime, brain, &devices, &mut resolver, &mut db) {
            Err(KnowledgeMaterializationError::Verification(detail)) => assert!(detail.contains("unverifiable envelope blocks complete history verification")),
            other => panic!("wrong rebuild refusal: {other:?}"),
        }
        assert_eq!(database_state(&db), db_before);
        assert_eq!(files(&vault), vault_before);
        assert_eq!(fs::read(&pack).unwrap(), after_append);
    }
    println!("historical_product_guard_v3_hold=passed");
}
