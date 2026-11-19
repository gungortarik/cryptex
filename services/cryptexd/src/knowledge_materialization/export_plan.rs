//! Complete physical packs borrowed from the locked, authenticated union.
use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub(crate) struct ExportPack<'a> {
    snapshot: &'a VerifiedCanonicalSnapshot,
    pub(crate) device: DeviceId,
    pub(crate) index: u64,
    pub(crate) length: u64,
    pub(crate) digest: [u8; 32],
    pub(crate) eligible: bool,
    pub(crate) contains_attachment: bool,
    pub(crate) retirement_only: bool,
}
impl ExportPack<'_> {
    pub(crate) fn validate(&self, root: &Path) -> bool {
        self.snapshot.locks_runtime(root)
            && self
                .snapshot
                .locks_devices(std::slice::from_ref(&self.device))
    }
    pub(crate) fn local_lock(&self) -> Option<&DeviceCanonicalLock> {
        let (history, _) = self.snapshot.verified_trust_context()?;
        if self.device != history.local_device || !(self.eligible || self.retirement_only) {
            return None;
        }
        self.snapshot
            ._locks
            .iter()
            .find(|lock| lock.device_id() == &self.device)
    }
}

impl VerifiedCanonicalSnapshot {
    pub(crate) fn export_packs(
        &self,
        live: impl Fn() -> bool,
    ) -> KnowledgeMaterializationResult<Vec<ExportPack<'_>>> {
        let check = || {
            if live() {
                Ok(())
            } else {
                Err(KnowledgeMaterializationError::DeadlineExceeded)
            }
        };
        let invalid = || {
            KnowledgeMaterializationError::Verification("export pack authority unavailable".into())
        };
        let (history, trust) = self.verified_trust_context().ok_or_else(invalid)?;
        let mut records = BTreeMap::new();
        for record in self.security_history.records() {
            check()?;
            records.insert((record.device_id(), record.event_id()), record);
        }
        let mut groups = BTreeMap::<(String, u64), Vec<&RawFrame>>::new();
        for frame in &self.security_frames {
            check()?;
            groups
                .entry((frame.position.device_id.clone(), frame.position.pack_index))
                .or_default()
                .push(frame);
        }
        let mut packs = Vec::with_capacity(groups.len());
        for ((device, index), mut frames) in groups {
            check()?;
            let device = DeviceId::new(device).map_err(|_| invalid())?;
            let cutoff = trust.effective_cutoffs.get(&device).ok_or_else(invalid)?;
            frames.sort_by_key(|frame| frame.position.frame_offset);
            let mut length = 0u64;
            let mut hash = Sha256::new();
            let mut eligible = true;
            let single = frames.len() == 1;
            let mut retirement_only = false;
            let mut contains_attachment = false;
            for frame in frames {
                check()?;
                if frame.position.frame_offset != length {
                    return Err(invalid());
                }
                let record = records
                    .get(&(device.as_str(), frame.position.event_id.as_str()))
                    .ok_or_else(invalid)?;
                eligible &= record.device_seq() <= *cutoff;
                // The transport contract currently admits only note and
                // security records. M9 read support does not activate transfer.
                contains_attachment |=
                    matches!(record, AuthenticatedSecurityRecord::Attachment(..));
                retirement_only = single
                    && history.self_retirements.iter().any(|terminal| {
                        terminal.device == device && terminal.event_seq == record.device_seq()
                    });
                let header = cryptex_events::framing::encode_frame_header(
                    u32::try_from(frame.bytes.len()).map_err(|_| invalid())?,
                )
                .map_err(|_| invalid())?;
                hash.update(header);
                hash.update(&frame.bytes);
                length = length
                    .checked_add((header.len() + frame.bytes.len()) as u64)
                    .ok_or_else(invalid)?;
            }
            if length == 0 {
                return Err(invalid());
            }
            packs.push(ExportPack {
                snapshot: self,
                device,
                index,
                length,
                digest: hash.finalize().into(),
                eligible: eligible && !contains_attachment,
                contains_attachment,
                retirement_only,
            });
        }
        Ok(packs)
    }
}
