//! Single-device disposable profile compaction under the canonical writer locks.
use super::*;
use crate::import_staging::export_fs::{self, Directory};
use crate::ipc::purge_review::{ReviewError, physical, receipt};
use std::io::{Read, Write};

const BRIDGE: &str = "memory.purge.bridge";

pub(super) fn check_receipts(
    root: &Path,
    brain: &str,
    anchor: &LocalTrustAnchor,
    frames: &[RawFrame],
) -> KnowledgeMaterializationResult<()> {
    for frame in frames {
        let invalid = || {
            KnowledgeMaterializationError::Verification(
                "invalid local purge bridge or approval".into(),
            )
        };
        let record: serde_json::Value =
            serde_json::from_slice(&frame.bytes).map_err(|_| invalid())?;
        if record["event_type"] == BRIDGE {
            if frame.position.device_id != anchor.device_id().as_str() {
                return Err(invalid());
            }
            let original = record["payload"]["original_hash"]
                .as_str()
                .ok_or_else(invalid)?;
            receipt::verify_reopened(
                root,
                brain,
                anchor.device_id().as_str(),
                &frame.position.event_id,
                original,
                anchor.public_key(),
            )
            .map_err(|_| invalid())?;
        }
    }
    Ok(())
}

fn append_framed(buffer: &mut Vec<u8>, body: &[u8]) -> Result<(), ReviewError> {
    let len = u32::try_from(body.len()).map_err(|_| ReviewError::PackIntegrity)?;
    buffer.extend(
        cryptex_events::framing::encode_frame_header(len)
            .map_err(|_| ReviewError::PackIntegrity)?,
    );
    buffer.extend_from_slice(body);
    Ok(())
}

impl VerifiedCanonicalSnapshot {
    pub(crate) fn compact_approved_note(
        &self,
        anchor: &LocalTrustAnchor,
        note: &str,
        hash: &str,
        key: &ed25519_dalek::SigningKey,
        expires: Instant,
    ) -> Result<(), ReviewError> {
        let root = &self.runtime_root;
        let device = anchor.device_id();
        if !physical::enabled(root) || !self.locks_runtime(root) || self.devices != [device.clone()]
        {
            return Err(ReviewError::Disabled);
        }
        // Cross-record references and replicas need their own purge protocol.
        if !self.security_history.records().iter().all(|r| {
            matches!(
                r,
                AuthenticatedSecurityRecord::V1(_) | AuthenticatedSecurityRecord::PurgeBridge(..)
            )
        }) {
            return Err(ReviewError::TargetUnavailable);
        }
        let target = self
            .security_frames
            .iter()
            .find(|f| f.position.event_id == note && f.position.content_hash == hash)
            .ok_or(ReviewError::TargetUnavailable)?;
        let original: serde_json::Value =
            serde_json::from_slice(&target.bytes).map_err(|_| ReviewError::PackIntegrity)?;
        let bridge = crate::test_temp::sign_v2_fixture(
            serde_json::json!({
                "schema_version": 2, "event_id": note, "brain_id": original["brain_id"], "device_id": device.as_str(),
                "device_seq": original["device_seq"], "prev_content_hash": original["prev_content_hash"],
                "created_at": original["created_at"], "signing_key_id": anchor.signing_key_id(), "classification": "restricted",
                "event_type": BRIDGE, "payload_schema_version": 1, "payload": {"original_hash": hash}
            }),
            key,
        );
        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut candidate = Vec::new();
        let index = target.position.pack_index;
        for frame in &self.security_frames {
            let next = if frame.position.event_id == note {
                bridge.as_slice()
            } else {
                &frame.bytes
            };
            candidate.push(next);
            if frame.position.pack_index == index {
                append_framed(&mut before, &frame.bytes)?;
                append_framed(&mut after, next)?;
            }
        }
        crate::trust_reducer::verify_anchored_history_for_reads(
            &candidate,
            original["brain_id"]
                .as_str()
                .ok_or(ReviewError::PackIntegrity)?,
            device.clone(),
            anchor.signing_key_id().clone(),
            *anchor.public_key(),
            NoteLifecycleRead::LocalPurgeTrial,
            || Instant::now() >= expires,
        )
        .map_err(|_| ReviewError::PackIntegrity)?;
        let profile = Directory::open(root)?;
        let packs = profile
            .child("canonical", false)?
            .child("events", false)?
            .child(device.as_filename_stem().as_ref(), false)?;
        let active = cryptex_storage::active_segment_path(root, device, index);
        let sealed = cryptex_storage::sealed_segment_path(root, device, index);
        let leaf = |p: &Path| p.file_name().unwrap().to_str().unwrap().to_owned();
        let path = match (packs.exists(&leaf(&active))?, packs.exists(&leaf(&sealed))?) {
            (true, false) => active,
            (false, true) => sealed,
            _ => return Err(ReviewError::PackIntegrity),
        };
        let mut stored = Vec::new();
        packs
            .open_file(&leaf(&path), false)?
            .take(before.len() as u64 + 1)
            .read_to_end(&mut stored)
            .map_err(|_| ReviewError::PackStorage)?;
        if stored != before {
            return Err(ReviewError::PackIntegrity);
        }
        let stage = profile.child("purge-staging", true)?;
        let name = "segment.tmp";
        if stage.exists(name)? {
            stage.discard_scratch(stage.open_file(name, false)?)?;
        }
        let mut scratch = stage.open_file(name, true)?;
        scratch
            .write_all(&after)
            .and_then(|()| export_fs::flush(&scratch))
            .map_err(|_| ReviewError::PackStorage)?;
        drop(scratch);
        packs.validate()?;
        stage.validate()?;
        if Instant::now() >= expires {
            return Err(ReviewError::ReviewExpired);
        }
        crate::metadata_persistence::replace_atomic(&root.join("purge-staging").join(name), &path)
            .map_err(|_| ReviewError::PackStorage)?;
        packs.sync()?;
        stage.sync()?;
        Ok(())
    }
}
