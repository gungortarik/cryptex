//! Pairing **record** persistence (ADR-0015 data only — no pairing UI / M8 flows).

use std::fs;
use std::path::{Path, PathBuf};

use cryptex_core::{PairingRecord, PairingState};

use crate::error::LifecycleError;
use crate::identity_runtime::IdentityRuntimeError;
use crate::paths::{assert_outside_repository, ensure_runtime_root};

const PAIRING_DIR: &str = "pairing";

pub fn pairing_dir(runtime_root: &Path) -> PathBuf {
    runtime_root.join(PAIRING_DIR)
}

fn record_path(runtime_root: &Path, peer: &cryptex_core::DeviceId) -> PathBuf {
    // Filename stem is lowercase-hex of UTF-8 bytes (ADR-0015 path-safety).
    pairing_dir(runtime_root).join(format!("{}.json", peer.as_filename_stem()))
}

pub fn write_pairing_record(
    runtime_root: &Path,
    record: &PairingRecord,
) -> Result<(), IdentityRuntimeError> {
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
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json.as_bytes())?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

pub fn load_pairing_record(
    runtime_root: &Path,
    peer: &cryptex_core::DeviceId,
) -> Result<PairingRecord, IdentityRuntimeError> {
    let path = record_path(runtime_root, peer);
    let text = fs::read_to_string(&path)?;
    let record: PairingRecord = serde_json::from_str(&text).map_err(|e| {
        IdentityRuntimeError::Lifecycle(LifecycleError::Io(format!("parse pairing: {e}")))
    })?;
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
        let record: PairingRecord = serde_json::from_str(&text).map_err(|e| {
            IdentityRuntimeError::Lifecycle(LifecycleError::Io(format!("parse pairing: {e}")))
        })?;
        out.push((record.peer_device_id.to_string(), record.state));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cryptex_core::DeviceId;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("cryptexd-pairing-{}", nanos));
        let _ = fs::remove_dir_all(&root);
        root
    }

    #[test]
    fn write_load_pairing_record_round_trip() {
        let root = temp_root();
        // 32 hex chars = 16 bytes for DeviceId
        let peer = DeviceId::new("peer-device-aabb").expect("id");
        let record = PairingRecord::new(
            peer.clone(),
            PairingState::Trusted,
            "brain-1",
            "2026-10-06T00:00:00Z",
        );
        write_pairing_record(&root, &record).expect("write");
        let loaded = load_pairing_record(&root, &peer).expect("load");
        assert_eq!(loaded, record);
        let states = list_pairing_states(&root).expect("list");
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].1, PairingState::Trusted);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn pairing_file_contains_no_secret_marker() {
        let root = temp_root();
        let peer = DeviceId::new("peer-device-ccdd").expect("id");
        let record = PairingRecord::new(
            peer,
            PairingState::Pending,
            "brain-1",
            "2026-10-06T00:00:00Z",
        );
        write_pairing_record(&root, &record).expect("write");
        let text = fs::read_to_string(record_path(&root, &record.peer_device_id)).unwrap();
        assert!(!text.contains("SECRET"));
        let _ = fs::remove_dir_all(&root);
    }
}
