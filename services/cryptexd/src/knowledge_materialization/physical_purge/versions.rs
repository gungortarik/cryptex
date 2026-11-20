//! Scrub only managed Syncthing archive packs covered by authenticated purge bridges.
use super::*;
use cryptex_events::framing::{FRAME_HEADER_BYTES, decode_frame_header, encode_frame_header};
use std::{collections::BTreeSet, fs::File};

impl VerifiedCanonicalSnapshot {
    pub(crate) fn clean_archived_versions(&self) -> Result<(), ReviewError> {
        if !physical::enabled(&self.runtime_root) || !self.locks_runtime(&self.runtime_root) {
            return Err(ReviewError::Disabled);
        }
        let mut archive = Directory::open(&self.runtime_root)?;
        for name in ["transport", "incoming", ".stversions"] {
            if !archive.exists(name)? {
                if self.has_pending_versions()? {
                    return Err(ReviewError::CleanupPending);
                }
                return Ok(());
            }
            archive = archive.child(name, false)?;
        }
        let bridged: BTreeSet<_> = self
            .security_history
            .records()
            .iter()
            .filter(|record| matches!(record, AuthenticatedSecurityRecord::PurgeBridge(..)))
            .map(|record| record.event_id())
            .collect();
        for device in &self.devices {
            let indices: BTreeSet<_> = self
                .security_frames
                .iter()
                .filter(|frame| {
                    frame.position.device_id == device.as_str()
                        && bridged.contains(frame.position.event_id.as_str())
                })
                .map(|f| f.position.pack_index)
                .collect();
            let stem = device.as_filename_stem();
            if indices.is_empty() {
                continue;
            }
            let work_root = Directory::open(&self.runtime_root)?
                .child("purge-staging", true)?
                .child("versions", true)?;
            let work = work_root.child(&stem, true)?;
            let mut names: BTreeSet<_> = work.archived_segments()?.into_iter().collect();
            if !archive.exists(&stem)? {
                if !names.is_empty() {
                    return Err(ReviewError::CleanupPending);
                }
                continue;
            }
            let versions = archive.child(&stem, false)?;
            names.extend(versions.archived_segments()?);
            for (index, name) in names {
                if indices.contains(&index) {
                    let frames: Vec<_> = self
                        .security_frames
                        .iter()
                        .filter(|f| {
                            f.position.device_id == device.as_str()
                                && f.position.pack_index == index
                        })
                        .collect();
                    self.scrub_version(&versions, &work, &name, &frames)?;
                }
            }
        }
        Ok(())
    }

    fn has_pending_versions(&self) -> Result<bool, ReviewError> {
        let mut dir = Directory::open(&self.runtime_root)?;
        for name in ["purge-staging", "versions"] {
            if !dir.exists(name)? {
                return Ok(false);
            }
            dir = dir.child(name, false)?;
        }
        for device in &self.devices {
            let stem = device.as_filename_stem();
            if dir.exists(&stem)? && !dir.child(&stem, false)?.archived_segments()?.is_empty() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// A historical pack may end on an earlier frame boundary. Every included
    /// frame must match its anchored position or the signed original of its bridge.
    fn inspect_version(
        &self,
        file: &mut File,
        frames: &[&RawFrame],
    ) -> Result<(usize, bool), ReviewError> {
        let mut count = 0;
        let mut stale = false;
        let mut header = [0; FRAME_HEADER_BYTES];
        while file
            .read(&mut header[..1])
            .map_err(|_| ReviewError::PackStorage)?
            != 0
        {
            let frame = frames.get(count).ok_or(ReviewError::PackIntegrity)?;
            file.read_exact(&mut header[1..])
                .map_err(|_| ReviewError::PackIntegrity)?;
            let (_, len) = decode_frame_header(&header).map_err(|_| ReviewError::PackIntegrity)?;
            let mut body = vec![0; len as usize];
            file.read_exact(&mut body)
                .map_err(|_| ReviewError::PackIntegrity)?;
            if body != frame.bytes {
                self.verify_original_for_bridge(frame, &body)?;
                stale = true;
            }
            count += 1;
        }
        if count == 0 {
            return Err(ReviewError::PackIntegrity);
        }
        Ok((count, stale))
    }

    fn scrub_version(
        &self,
        versions: &Directory,
        work: &Directory,
        name: &str,
        frames: &[&RawFrame],
    ) -> Result<(), ReviewError> {
        if work.exists(name)? {
            self.finish_version(work, versions, name, frames)?;
        }
        let mut source = versions.open_file(name, false)?;
        if self.inspect_version(&mut source, frames)?.1 {
            versions.promote(name, &source, work, name)?;
            drop(source);
            self.finish_version(work, versions, name, frames)?;
        }
        Ok(())
    }

    fn finish_version(
        &self,
        work: &Directory,
        versions: &Directory,
        name: &str,
        frames: &[&RawFrame],
    ) -> Result<(), ReviewError> {
        let mut original = work.open_file(name, false)?;
        let (count, _) = self.inspect_version(&mut original, frames)?;
        if work.exists("segment.tmp")? {
            work.discard_scratch(work.open_file("segment.tmp", false)?)?;
        }
        if versions.exists(name)? {
            // Already published, or recreated by the transport. Never overwrite a
            // different historical prefix. The caller rechecks this copy for staleness.
            let mut current = versions.open_file(name, false)?;
            if self.inspect_version(&mut current, frames)?.0 != count {
                return Err(ReviewError::CleanupPending);
            }
        } else {
            let mut clean = work.open_file("segment.tmp", true)?;
            for frame in &frames[..count] {
                let len =
                    u32::try_from(frame.bytes.len()).map_err(|_| ReviewError::PackIntegrity)?;
                let header = encode_frame_header(len).map_err(|_| ReviewError::PackIntegrity)?;
                clean
                    .write_all(&header)
                    .and_then(|()| clean.write_all(&frame.bytes))
                    .map_err(|_| ReviewError::PackStorage)?;
            }
            work.promote("segment.tmp", &clean, versions, name)?;
        }
        versions.sync()?;
        work.discard_archived_segment(name, original)?;
        work.sync()?;
        Ok(())
    }
}
