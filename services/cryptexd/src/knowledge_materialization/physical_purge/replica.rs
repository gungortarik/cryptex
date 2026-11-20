//! Reclaim transport copies only when the canonical pack accounts for every frame.
use super::*;
use cryptex_events::framing::{FRAME_HEADER_BYTES, decode_frame_header};
use cryptex_events::{VersionedRecord, read_versioned_record};
use std::collections::BTreeSet;

impl VerifiedCanonicalSnapshot {
    /// A stale pack can arrive again after compaction. Always check the disposable
    /// exchange, including on restart, without touching unrelated incoming data.
    pub(crate) fn clean_purged_exchange(
        &self,
        local: &DeviceId,
    ) -> Result<Vec<String>, ReviewError> {
        let root = &self.runtime_root;
        if !physical::paired(root) || !self.locks_runtime(root) {
            return Err(ReviewError::Disabled);
        }
        let mut packs = BTreeSet::new();
        let mut remote_bridges = Vec::new();
        for record in self.security_history.records() {
            if let AuthenticatedSecurityRecord::PurgeBridge(bridge, ..) = record {
                let position = &self
                    .security_frames
                    .iter()
                    .find(|f| f.position.event_id == bridge.event_id)
                    .ok_or(ReviewError::PackIntegrity)?
                    .position;
                let device = DeviceId::new(bridge.device_id.clone())
                    .map_err(|_| ReviewError::PackIntegrity)?;
                if &device != local {
                    remote_bridges.push(bridge.event_id.clone());
                }
                packs.insert((device, position.pack_index));
            }
        }
        let incoming = Directory::open(root)?
            .child("transport", false)?
            .child("incoming", false)?;
        for (device, index) in packs {
            let stem = device.as_filename_stem();
            if !incoming.exists(&stem)? {
                continue;
            }
            let dir = incoming.child(&stem, false)?;
            let name = format!("{index:020}.seg");
            if !dir.exists(&name)? {
                continue;
            }
            let mut file = dir.open_file(&name, false)?;
            let mut keys = self
                .trust_history
                .as_ref()
                .ok_or(ReviewError::Unauthorized)?
                .key_resolver();
            for frame in self.security_frames.iter().filter(|f| {
                f.position.device_id == device.as_str() && f.position.pack_index == index
            }) {
                let mut header = [0; FRAME_HEADER_BYTES];
                file.read_exact(&mut header)
                    .map_err(|_| ReviewError::PackIntegrity)?;
                let (_, len) =
                    decode_frame_header(&header).map_err(|_| ReviewError::PackIntegrity)?;
                let mut bytes = vec![0; len as usize];
                file.read_exact(&mut bytes)
                    .map_err(|_| ReviewError::PackIntegrity)?;
                if bytes == frame.bytes {
                    continue;
                }
                let Some(AuthenticatedSecurityRecord::PurgeBridge(bridge, _, hash)) = self
                    .security_history
                    .records()
                    .iter()
                    .find(|r| r.event_id() == frame.position.event_id)
                else {
                    return Err(ReviewError::PackIntegrity);
                };
                let key = keys
                    .trusted_daemon_key(device.as_str(), bridge.signing_key_id.as_str())
                    .ok_or(ReviewError::Unauthorized)?;
                if !matches!(read_versioned_record(&bytes, &key), Ok(VersionedRecord::VerifiedV1(note))
                    if note.event_id == bridge.event_id && note.content_hash == *hash)
                {
                    return Err(ReviewError::PackIntegrity);
                }
            }
            if file.read(&mut [0]).map_err(|_| ReviewError::PackStorage)? != 0 {
                return Err(ReviewError::PackIntegrity);
            }
            dir.remove_purged_exchange(index, file)?;
            dir.sync()?;
        }
        Ok(remote_bridges)
    }
}
