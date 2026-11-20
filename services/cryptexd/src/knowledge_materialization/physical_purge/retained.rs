use super::*;
use crate::ipc::purge_review::physical::retained::PackImage;
use std::collections::BTreeMap;

impl VerifiedCanonicalSnapshot {
    /// Reconstruct only the authenticated prefix actually present at backup time.
    pub(crate) fn scrub_retained_pack(
        &self,
        device: &DeviceId,
        index: u64,
        image: &PackImage,
    ) -> Result<PackImage, ReviewError> {
        if !physical::enabled(&self.runtime_root) || !self.locks_runtime(&self.runtime_root) {
            return Err(ReviewError::Disabled);
        }
        let frames: Vec<_> = self
            .security_frames
            .iter()
            .filter(|frame| {
                frame.position.device_id == device.as_str() && frame.position.pack_index == index
            })
            .collect();
        let (count, _) = self.inspect_version(&mut image.pack.as_slice(), &frames)?;
        let mut pack = Vec::new();
        let mut approvals = BTreeMap::new();
        let canonical = Directory::open(&self.runtime_root)?.child("canonical", false)?;
        let bridges: std::collections::BTreeSet<_> = self
            .reader_device_chain(device.as_str())
            .iter()
            .filter(|r| matches!(r, AuthenticatedSecurityRecord::PurgeBridge(..)))
            .map(|r| r.event_id())
            .collect();
        for frame in &frames[..count] {
            append_framed(&mut pack, &frame.bytes)?;
            let id = &frame.position.event_id;
            if bridges.contains(id.as_str()) {
                // The locked scan authenticated the ledger against the original author.
                let ledger = canonical.child(receipt::FOLDER, false)?;
                let mut proof = Vec::new();
                ledger
                    .open_file(&format!("{id}.json"), false)?
                    .take(1025)
                    .read_to_end(&mut proof)
                    .map_err(|_| ReviewError::ReceiptStorage)?;
                if proof.len() > 1024 {
                    return Err(ReviewError::ReceiptIntegrity);
                }
                approvals.insert(id.clone(), proof);
            }
        }
        if image
            .approvals
            .iter()
            .any(|(id, proof)| approvals.get(id) != Some(proof))
        {
            return Err(ReviewError::ReceiptIntegrity);
        }
        Ok(PackImage { pack, approvals })
    }
}
