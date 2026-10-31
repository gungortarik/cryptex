//! Pairing **record** persistence (ADR-0015 data only -- no pairing UI / M8 flows).

use std::fs;
use std::path::{Path, PathBuf};

use cryptex_core::{CoreError, PairingRecord, PairingState};

use crate::error::LifecycleError;
use crate::identity_runtime::{IdentityRuntimeError, brain_identity_path, load_brain_identity};
use crate::metadata_persistence::write_public_metadata;
use crate::paths::{assert_outside_repository, ensure_runtime_root};

const PAIRING_DIR: &str = "pairing";

pub fn pairing_dir(runtime_root: &Path) -> PathBuf {
    runtime_root.join(PAIRING_DIR)
}

fn record_path(runtime_root: &Path, peer: &cryptex_core::DeviceId) -> PathBuf {
    // Filename stem is lowercase-hex of UTF-8 bytes (ADR-0015 path-safety).
    pairing_dir(runtime_root).join(format!("{}.json", peer.as_filename_stem()))
}

/// ADR-0015 section 2.4: `local_brain_id` must match the local Brain identity.
fn require_local_brain_match(
    runtime_root: &Path,
    local_brain_id: &str,
) -> Result<(), IdentityRuntimeError> {
    if !brain_identity_path(runtime_root).exists() {
        return Err(IdentityRuntimeError::Core(CoreError::InvalidId(
            "local Brain identity metadata is missing; cannot accept pairing record".into(),
        )));
    }
    let brain = load_brain_identity(runtime_root)?;
    if brain.brain_id != local_brain_id {
        return Err(IdentityRuntimeError::Core(CoreError::InvalidId(
            "pairing local_brain_id must match local Brain identity".into(),
        )));
    }
    Ok(())
}

pub fn write_pairing_record(
    runtime_root: &Path,
    record: &PairingRecord,
) -> Result<(), IdentityRuntimeError> {
    record.validate().map_err(IdentityRuntimeError::from)?;
    require_local_brain_match(runtime_root, &record.local_brain_id)?;
    assert_outside_repository(runtime_root)?;
    ensure_runtime_root(runtime_root)?;
    let dir = pairing_dir(runtime_root);
    fs::create_dir_all(&dir)?;
    let path = record_path(runtime_root, &record.peer_device_id);
    let mut json = serde_json::to_string_pretty(record).map_err(|e| {
        IdentityRuntimeError::Lifecycle(LifecycleError::Io(format!("serialize pairing: {e}")))
    })?;
    json = json.replace("\r\n", "\n");
    if !json.ends_with('\n') {
        json.push('\n');
    }
    write_public_metadata(&path, json.as_bytes())
}

pub fn load_pairing_record(
    runtime_root: &Path,
    peer: &cryptex_core::DeviceId,
) -> Result<PairingRecord, IdentityRuntimeError> {
    let path = record_path(runtime_root, peer);
    let text = fs::read_to_string(&path)?;
    let record = PairingRecord::from_json(&text)?;
    require_local_brain_match(runtime_root, &record.local_brain_id)?;
    Ok(record)
}

pub fn list_pairing_states(
    runtime_root: &Path,
) -> Result<Vec<(String, PairingState)>, IdentityRuntimeError> {
    let dir = pairing_dir(runtime_root);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        let record = PairingRecord::from_json(&text)?;
        require_local_brain_match(runtime_root, &record.local_brain_id)?;
        out.push((record.peer_device_id.to_string(), record.state));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity_runtime::write_brain_identity;
    use cryptex_core::{BrainIdentity, DeviceId};

    const BRAIN: &str = "11111111-1111-4111-8111-111111111111";

    fn temp_root() -> PathBuf {
        crate::test_temp::exclusive_temp_dir("cryptexd-pairing")
    }

    fn seed_brain(root: &Path, brain_id: &str) {
        let identity = BrainIdentity::new(brain_id, "2026-10-06T00:00:00Z").expect("brain");
        write_brain_identity(root, &identity).expect("write brain");
    }

    #[test]
    fn write_load_pairing_record_round_trip() {
        let root = temp_root();
        seed_brain(&root, BRAIN);
        let peer = DeviceId::new("peer-device-aabb").expect("id");
        let record = PairingRecord::new(
            peer.clone(),
            PairingState::Trusted,
            BRAIN,
            "2026-10-06T00:00:00Z",
        )
        .expect("record");
        write_pairing_record(&root, &record).expect("write");
        let loaded = load_pairing_record(&root, &peer).expect("load");
        assert_eq!(loaded, record);
        let states = list_pairing_states(&root).expect("list");
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].1, PairingState::Trusted);
        let mut updated = record.clone();
        updated.state = PairingState::Retired;
        write_pairing_record(&root, &updated).expect("replace existing record");
        assert_eq!(load_pairing_record(&root, &peer).unwrap(), updated);
        assert_eq!(
            list_pairing_states(&root).unwrap(),
            vec![(peer.to_string(), PairingState::Retired)]
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn pairing_file_contains_no_secret_marker() {
        let root = temp_root();
        seed_brain(&root, BRAIN);
        let peer = DeviceId::new("peer-device-ccdd").expect("id");
        let record = PairingRecord::new(peer, PairingState::Pending, BRAIN, "2026-10-06T00:00:00Z")
            .expect("record");
        write_pairing_record(&root, &record).expect("write");
        let text = fs::read_to_string(record_path(&root, &record.peer_device_id)).unwrap();
        assert!(!text.contains("SECRET"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn write_fails_closed_when_local_brain_metadata_missing() {
        let root = temp_root();
        let peer = DeviceId::new("peer-device-missing-brain").expect("id");
        let record = PairingRecord::new(peer, PairingState::Trusted, BRAIN, "2026-10-06T00:00:00Z")
            .expect("record");
        let err = write_pairing_record(&root, &record).expect_err("missing brain");
        assert!(matches!(
            err,
            IdentityRuntimeError::Core(CoreError::InvalidId(msg))
                if msg.contains("Brain identity metadata is missing")
        ));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn write_and_load_fail_closed_on_local_brain_id_mismatch() {
        let root = temp_root();
        seed_brain(&root, BRAIN);
        let peer = DeviceId::new("peer-device-mismatch").expect("id");
        let other = "22222222-2222-4222-8222-222222222222";
        let record = PairingRecord::new(
            peer.clone(),
            PairingState::Trusted,
            other,
            "2026-10-06T00:00:00Z",
        )
        .expect("record");
        let err = write_pairing_record(&root, &record).expect_err("mismatch write");
        assert!(matches!(
            err,
            IdentityRuntimeError::Core(CoreError::InvalidId(msg))
                if msg.contains("must match local Brain identity")
        ));

        // Persist a mismatched record out-of-band, then fail closed on load/list.
        fs::create_dir_all(pairing_dir(&root)).unwrap();
        let path = record_path(&root, &peer);
        let mut json = serde_json::to_string_pretty(&record).unwrap();
        json = json.replace("\r\n", "\n");
        if !json.ends_with('\n') {
            json.push('\n');
        }
        fs::write(&path, json).unwrap();
        let load_err = load_pairing_record(&root, &peer).expect_err("mismatch load");
        assert!(matches!(
            load_err,
            IdentityRuntimeError::Core(CoreError::InvalidId(msg))
                if msg.contains("must match local Brain identity")
        ));
        let list_err = list_pairing_states(&root).expect_err("mismatch list");
        assert!(matches!(
            list_err,
            IdentityRuntimeError::Core(CoreError::InvalidId(msg))
                if msg.contains("must match local Brain identity")
        ));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_fails_closed_on_future_schema_and_malformed_fields() {
        let root = temp_root();
        seed_brain(&root, BRAIN);
        let peer = DeviceId::new("peer-device-eeff").expect("id");
        let path = record_path(&root, &peer);
        fs::create_dir_all(pairing_dir(&root)).unwrap();
        fs::write(
            &path,
            r#"{
  "schema_version": 9,
  "peer_device_id": "peer-device-eeff",
  "state": "trusted",
  "local_brain_id": "11111111-1111-4111-8111-111111111111",
  "updated_at": "2026-10-06T00:00:00Z"
}
"#,
        )
        .unwrap();
        let err = load_pairing_record(&root, &peer).unwrap_err();
        assert!(matches!(
            err,
            IdentityRuntimeError::Core(CoreError::UnsupportedSchema {
                found: 9,
                expected: 1,
                ..
            })
        ));

        fs::write(
            &path,
            r#"{
  "schema_version": 1,
  "peer_device_id": "peer-device-eeff",
  "state": "trusted",
  "local_brain_id": "not-a-uuid",
  "updated_at": "2026-10-06T00:00:00Z"
}
"#,
        )
        .unwrap();
        assert!(load_pairing_record(&root, &peer).is_err());
        let _ = fs::remove_dir_all(&root);
    }
}
