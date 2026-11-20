//! Disposable encrypted backup fixture. This adapter is compiled only for native
//! daemon tests. Its key and registered directory handles are scoped to the trial.
use super::ReviewError;
use crate::import_staging::export_fs::{Directory, flush};
use crate::knowledge_materialization::{
    VerifiedCanonicalSnapshot, scan_verified_canonical_from_local_anchor_until,
};
use age::x25519::Identity;
use cryptex_core::DeviceId;
use cryptex_events::LocalTrustAnchor;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct PackImage {
    pub(crate) pack: Vec<u8>,
    pub(crate) approvals: BTreeMap<String, Vec<u8>>,
}

struct Target {
    directory: Directory,
    device: DeviceId,
    index: u64,
}
struct Trial {
    source: PathBuf,
    key: Identity,
    targets: Vec<Target>,
}
static TRIAL: Mutex<Option<Trial>> = Mutex::new(None);
const PACK: &str = "pack.age";
const ORIGINAL: &str = "segment.tmp";
const MAX_FIXTURE_BYTES: u64 = 8 * 1024 * 1024;

fn read_image(file: &File, key: &Identity) -> Result<PackImage, ReviewError> {
    let mut sealed = Vec::new();
    file.take(MAX_FIXTURE_BYTES + 1)
        .read_to_end(&mut sealed)
        .map_err(|_| ReviewError::PackStorage)?;
    if sealed.len() as u64 > MAX_FIXTURE_BYTES {
        return Err(ReviewError::PackIntegrity);
    }
    let bytes = age::decrypt(key, &sealed).map_err(|_| ReviewError::PackIntegrity)?;
    let image = serde_json::from_slice(&bytes).map_err(|_| ReviewError::PackIntegrity)?;
    if serde_json::to_vec(&image).map_err(|_| ReviewError::PackIntegrity)? != bytes {
        return Err(ReviewError::PackIntegrity);
    }
    Ok(image)
}

fn write_image(file: &mut File, key: &Identity, image: &PackImage) -> Result<(), ReviewError> {
    let bytes = serde_json::to_vec(image).map_err(|_| ReviewError::PackIntegrity)?;
    let sealed = age::encrypt(&key.to_public(), &bytes).map_err(|_| ReviewError::PackStorage)?;
    if sealed.len() as u64 > MAX_FIXTURE_BYTES {
        return Err(ReviewError::PackStorage);
    }
    file.write_all(&sealed)
        .and_then(|()| flush(file))
        .map_err(|_| ReviewError::PackStorage)
}

pub(super) fn clean_registered(
    root: &Path,
    brain: &str,
    anchor: &LocalTrustAnchor,
) -> Result<(), ReviewError> {
    let guard = TRIAL.lock().map_err(|_| ReviewError::CleanupPending)?;
    let Some(trial) = guard.as_ref().filter(|trial| trial.source == root) else {
        return Ok(());
    };
    let snapshot = scan_verified_canonical_from_local_anchor_until(root, brain, anchor, None)
        .map_err(|_| ReviewError::CleanupPending)?;
    let mut result = Ok(());
    for target in &trial.targets {
        // An unavailable target must not prevent cleanup on the reachable ones.
        if let Err(error) = target.scrub(&snapshot, &trial.key) {
            result = result.and(Err(error));
        }
    }
    result
}

impl Target {
    fn scrub(
        &self,
        snapshot: &VerifiedCanonicalSnapshot,
        key: &Identity,
    ) -> Result<(), ReviewError> {
        let dir = &self.directory;
        let _lock = dir
            .try_lock_exclusive("writer.lock")
            .map_err(|_| ReviewError::CleanupPending)?;
        let interrupted = dir.exists(ORIGINAL)?;
        let source = dir.open_file(if interrupted { ORIGINAL } else { PACK }, false)?;
        let previous = read_image(&source, key)?;
        let replacement = snapshot.scrub_retained_pack(&self.device, self.index, &previous)?;
        if !interrupted {
            if previous == replacement {
                return Ok(());
            }
            // Claim this exact open file; do not overwrite a concurrently created pack.
            dir.promote(PACK, &source, dir, ORIGINAL)?;
        }
        let stage = dir.child("rewrite", true)?;
        if stage.exists(ORIGINAL)? {
            stage.discard_scratch(stage.open_file(ORIGINAL, false)?)?;
        }
        if !dir.exists(PACK)? {
            let mut file = stage.open_file(ORIGINAL, true)?;
            write_image(&mut file, key, &replacement)?;
            stage.promote(ORIGINAL, &file, dir, PACK)?;
        }
        // Reopen and decrypt the durable replacement before removing the saved
        // original. A different existing image is a conflict, even on retry.
        if read_image(&dir.open_file(PACK, false)?, key)? != replacement {
            return Err(ReviewError::PackIntegrity);
        }
        dir.sync()?;
        dir.discard_scratch(source)?;
        dir.sync()?;
        Ok(())
    }
}

mod tests;
