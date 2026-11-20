//! Disposable profile compaction under the canonical writer locks.
use super::*;
use crate::import_staging::export_fs::{self, Directory};
use crate::ipc::purge_review::{ReviewError, physical, receipt};
use std::io::{Read, Write};

mod replica;

pub(super) fn check_receipts(
    root: &Path,
    brain: &str,
    history: &VerifiedSecurityHistory,
    trust: &crate::trust_reducer::VerifiedTrustHistory,
) -> KnowledgeMaterializationResult<()> {
    let mut resolver = trust.key_resolver();
    for record in history.records() {
        if let AuthenticatedSecurityRecord::PurgeBridge(bridge, _, original, _) = record {
            let invalid = || {
                KnowledgeMaterializationError::Verification("invalid purge bridge approval".into())
            };
            let key = resolver
                .trusted_daemon_key(&bridge.device_id, bridge.signing_key_id.as_str())
                .ok_or_else(invalid)?;
            receipt::verify_reopened(
                root,
                brain,
                &bridge.device_id,
                &bridge.event_id,
                original.as_str(),
                key.as_bytes(),
            )
            .map_err(|_| invalid())?;
        }
    }
    Ok(())
}

/// A replica can retain an old signed frame. Its author's durable decision
/// takes precedence on every guarded read, including after an index rebuild.
pub(super) fn suppress_approved_notes(
    root: &Path,
    brain: &str,
    history: &VerifiedSecurityHistory,
    trust: &crate::trust_reducer::VerifiedTrustHistory,
    hidden: &mut std::collections::BTreeSet<String>,
) -> KnowledgeMaterializationResult<()> {
    let invalid = || KnowledgeMaterializationError::Verification("invalid purge ledger".into());
    let canonical = Directory::open(root)
        .and_then(|p| p.child("canonical", false))
        .map_err(|_| invalid())?;
    if !canonical.exists(receipt::FOLDER).map_err(|_| invalid())? {
        return Ok(());
    }
    let ledger = canonical
        .child(receipt::FOLDER, false)
        .map_err(|_| invalid())?;
    let mut resolver = trust.key_resolver();
    for record in history.records() {
        let AuthenticatedSecurityRecord::V1(note) = record else {
            continue;
        };
        if !ledger
            .exists(&format!("{}.json", note.event_id))
            .map_err(|_| invalid())?
        {
            continue;
        }
        let key = resolver
            .trusted_daemon_key(&note.device_id, note.signing_key_id.as_str())
            .ok_or_else(invalid)?;
        receipt::verify_reopened(
            root,
            brain,
            &note.device_id,
            &note.event_id,
            note.content_hash.as_str(),
            key.as_bytes(),
        )
        .map_err(|_| invalid())?;
        hidden.insert(note.event_id.clone());
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
    pub(crate) fn reader_has_link_target(&self, id: &str) -> bool {
        self.security_history.records().iter().any(|r|
            matches!(r, AuthenticatedSecurityRecord::Attachment(_, f) if f.note_event_id().eq_ignore_ascii_case(id)))
    }

    pub(crate) fn clean_purged_blobs(&self) -> Result<(), ReviewError> {
        if !physical::enabled(&self.runtime_root) || !self.locks_runtime(&self.runtime_root) {
            return Err(ReviewError::Disabled);
        }
        if self.orphaned_by_purge.is_empty() {
            return Ok(());
        }
        let root = Directory::open(&self.runtime_root)?;
        let staging = root.child("blob-staging", true)?;
        let _lock = staging
            .try_lock_exclusive("writer.lock")
            .map_err(|_| ReviewError::CleanupPending)?;
        let canonical = root.child("canonical", false)?;
        let mut locations = Vec::new();
        if canonical.exists("blobs")? {
            locations.push(canonical.child("blobs", false)?);
        }
        let mut exchange = Some(root);
        for component in [
            "transport",
            "incoming",
            crate::import_staging::ATTACHMENT_FOLDER,
        ] {
            exchange = match exchange {
                Some(dir) if dir.exists(component)? => Some(dir.child(component, false)?),
                _ => None,
            };
        }
        if let Some(exchange) = exchange {
            let (history, _) = self
                .verified_trust_context()
                .ok_or(ReviewError::Unauthorized)?;
            let slots: std::collections::BTreeSet<_> = history
                .approved_peer_bindings
                .values()
                .flat_map(|b| [&b.local_transport_device_id, &b.transport_device_id])
                .collect();
            for slot in slots {
                if exchange.exists(slot)? {
                    locations.push(exchange.child(slot, false)?);
                }
            }
        }
        for digest in &self.orphaned_by_purge {
            for loc in &locations {
                crate::attachment_blobs::remove_verified_copy(loc, digest)?;
            }
        }
        Ok(())
    }

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
        if !physical::enabled(root)
            || !self.locks_runtime(root)
            || (!physical::paired(root) && self.devices != [device.clone()])
        {
            return Err(ReviewError::Disabled);
        }
        let Some(AuthenticatedSecurityRecord::V1(original)) = self
            .reader_device_chain(device.as_str())
            .iter()
            .find(|r| r.event_id() == note && r.content_hash().as_str() == hash)
        else {
            return Err(ReviewError::TargetUnavailable);
        };
        let bridge = crate::test_temp::sign_v2_fixture(
            receipt::bridge_body(original, self.reader_has_link_target(note)),
            key,
        );
        self.replace_approved_frame(anchor, device, note, &bridge, expires)
    }

    /// Install the author's exact signed bridge, keeping all other frames byte-identical.
    pub(crate) fn replace_approved_frame(
        &self,
        anchor: &LocalTrustAnchor,
        device: &DeviceId,
        note: &str,
        bridge: &[u8],
        expires: Instant,
    ) -> Result<(), ReviewError> {
        let root = &self.runtime_root;
        if !physical::enabled(root)
            || !self.locks_runtime(root)
            || (device != anchor.device_id() && !physical::paired(root))
        {
            return Err(ReviewError::Disabled);
        }
        // Other cross-record dependencies still require a dedicated purge protocol.
        if !self.security_history.records().iter().all(|r| {
            matches!(
                r,
                AuthenticatedSecurityRecord::V1(_)
                    | AuthenticatedSecurityRecord::Attachment(..)
                    | AuthenticatedSecurityRecord::PurgeBridge(..)
                    | AuthenticatedSecurityRecord::SecurityV3(_)
            )
        }) {
            return Err(ReviewError::TargetUnavailable);
        }
        let target = self
            .security_frames
            .iter()
            .find(|f| f.position.device_id == device.as_str() && f.position.event_id == note)
            .ok_or(ReviewError::TargetUnavailable)?;
        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut candidate = Vec::new();
        let index = target.position.pack_index;
        for frame in &self.security_frames {
            let next = if frame.position == target.position {
                bridge
            } else {
                &frame.bytes
            };
            candidate.push(next);
            if frame.position.device_id == device.as_str() && frame.position.pack_index == index {
                append_framed(&mut before, &frame.bytes)?;
                append_framed(&mut after, next)?;
            }
        }
        crate::trust_reducer::verify_anchored_history_for_reads(
            &candidate,
            self.security_history.expected_brain_id(),
            anchor.device_id().clone(),
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
