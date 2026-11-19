//! Schedule discovery hints. The publisher authenticates the complete chain
//! under its own locks; filesystem presence here grants no authority.
use super::{active_segment_path, parse_transport_segment_path, sealed_segment_path};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

// A clean rejection publishes none of the batch. Keep its members together
// until one changes, so an unchanged successor can accompany a corrected pack.
#[derive(Default)]
pub(super) struct RejectedBatches(Vec<Vec<PathBuf>>);

impl RejectedBatches {
    pub(super) fn remember(&mut self, paths: Vec<PathBuf>) {
        if paths.len() > 1 {
            self.0.push(paths);
        }
    }

    pub(super) fn select_unseen<T: PartialEq>(
        &mut self,
        seen: &mut BTreeMap<PathBuf, T>,
        inventory: &mut BTreeMap<PathBuf, T>,
    ) {
        self.0.retain(|batch| {
            if batch
                .iter()
                .all(|path| inventory.get(path) == seen.get(path))
            {
                return true;
            }
            for path in batch {
                seen.remove(path);
            }
            false
        });
        inventory.retain(|path, fingerprint| seen.get(path) != Some(fingerprint));
    }
}

fn present(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub(super) fn ready_batch<T>(
    root: &Path,
    candidates: BTreeMap<PathBuf, T>,
) -> io::Result<Vec<(PathBuf, T)>> {
    let mut ready = Vec::new();
    for (path, fingerprint) in candidates {
        // A malformed name still goes through the existing rejection path.
        if let Ok((device, index)) = parse_transport_segment_path(&path)
            && let Some(previous) = index.checked_sub(1)
        {
            let predecessor = path.with_file_name(format!("{previous:020}.seg"));
            if !ready.iter().any(|(p, _)| p == &predecessor)
                && !present(&sealed_segment_path(root, &device, previous))?
                && !present(&active_segment_path(root, &device, previous))?
            {
                // Leave the fingerprint unseen. A later scan may find the missing
                // predecessor even if this file's bytes and timestamps never change.
                continue;
            }
        }
        ready.push((path, fingerprint));
        if ready.len() == crate::import_limits::MAX_IMPORT_FILES_PER_BATCH {
            break;
        }
    }
    Ok(ready)
}
