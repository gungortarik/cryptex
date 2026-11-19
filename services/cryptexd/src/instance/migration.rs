//! Offline layout-only migration. All canonical, identity and transport data
//! retain their original paths. The old daemon may win the vacant PID name
//! before publication; no-replace then leaves a retryable v1 profile.
use std::fs;
use std::io::{self, Read};
use std::path::Path;

use super::{M9_INSTANCE_FILE, observe_instance};
use crate::import_staging::export_fs::{Directory, Error as FsError};

pub(crate) const STAGED: &str = "m9-lock.prepare";
const LEGACY: &str = "daemon.lock";

#[derive(Debug, PartialEq, Eq)]
pub enum MigrationError {
    InvalidProfile,
    Busy,
    UnexpectedLayout,
    UnsafeFilesystem,
    Io(io::ErrorKind),
}

impl From<io::Error> for MigrationError {
    fn from(error: io::Error) -> Self {
        if super::is_lock_conflict(&error) || cfg!(windows) && error.raw_os_error() == Some(32) {
            Self::Busy
        } else {
            Self::Io(error.kind())
        }
    }
}

impl From<FsError> for MigrationError {
    fn from(error: FsError) -> Self {
        match error {
            FsError::Unsafe => Self::UnsafeFilesystem,
            FsError::Io(_, Some(code)) => io::Error::from_raw_os_error(code).into(),
            FsError::Io(kind, None) => Self::Io(kind),
        }
    }
}

/// Explicit maintenance operation on an already initialized profile.
/// Success is idempotent. Normal startup neither starts nor repairs a migration.
pub fn migrate_profile(root: &Path) -> Result<(), MigrationError> {
    crate::paths::assert_outside_repository(root).map_err(|_| MigrationError::InvalidProfile)?;
    let namespace = Directory::open(root)?;
    let identity = namespace.child("identity", false)?;
    let mut brain = String::new();
    let file = identity.open_existing_brain_identity()?;
    if file.metadata()?.len() > 65536 {
        return Err(MigrationError::InvalidProfile);
    }
    file.take(65537).read_to_string(&mut brain)?;
    if brain.len() > 65536 || cryptex_core::BrainIdentity::from_json(&brain).is_err() {
        return Err(MigrationError::InvalidProfile);
    }
    namespace
        .child("canonical", false)?
        .child("events", false)?;
    let _migration = identity.try_lock_exclusive("m9-migration.lock")?;
    let pending = namespace.exists(STAGED)?;
    let legacy_exists = namespace.exists(LEGACY)?;
    if legacy_exists {
        let observation = observe_instance(root).map_err(|_| MigrationError::UnexpectedLayout)?;
        if observation.lock_held {
            return Err(MigrationError::Busy);
        }
        if fs::symlink_metadata(root.join(LEGACY))?.is_dir() {
            // observe_instance has already verified the protected PID file.
            if pending {
                return Err(MigrationError::UnexpectedLayout);
            }
            return Ok(namespace.sync()?);
        }
    } else if !pending {
        return Err(MigrationError::UnexpectedLayout);
    }

    let stage = if pending {
        namespace.child(STAGED, false)?
    } else {
        namespace.create_child_exclusive(STAGED)?
    };
    // Never publish unexpected contents of a stale or foreign staging folder.
    for entry in fs::read_dir(root.join(STAGED))? {
        if entry?.file_name() != M9_INSTANCE_FILE {
            return Err(MigrationError::UnexpectedLayout);
        }
    }
    let prepared_pid = stage.exists(M9_INSTANCE_FILE)?;
    if !prepared_pid && !legacy_exists {
        // The old name can be retired only after the staged PID was flushed.
        return Err(MigrationError::UnexpectedLayout);
    }
    let pid = stage.open_file(M9_INSTANCE_FILE, !prepared_pid)?;
    if pid.metadata()?.len() != 0 {
        return Err(MigrationError::UnexpectedLayout);
    }
    pid.sync_all()?;
    drop(pid);
    stage.sync()?;
    namespace.sync()?;
    drop(stage);
    #[cfg(test)]
    interruption("prepared");

    if legacy_exists {
        // M8 validates its opened handle again after acquiring its lock.
        // On macOS unlink while locked makes stale descriptors zero-link.
        // On Windows the delete-capable handle excludes existing writers.
        let file = namespace.open_file(LEGACY, false)?;
        fs2::FileExt::try_lock_exclusive(&file)?;
        let mut pid = String::new();
        (&file).take(32).read_to_string(&mut pid)?;
        if file.metadata()?.len() >= 32
            || !(pid.is_empty() || pid.trim_end().parse::<u32>().is_ok_and(|p| p > 0))
        {
            return Err(MigrationError::UnexpectedLayout);
        }
        namespace.retire_m8_pid(file)?;
    }
    #[cfg(test)]
    interruption("retired");
    namespace.publish_m9_lock()?;
    #[cfg(test)]
    interruption("published");
    crate::import_staging::open_existing_profile_lock(&root.join(LEGACY), M9_INSTANCE_FILE)
        .map_err(|_| MigrationError::UnexpectedLayout)?;
    Ok(())
}

#[cfg(test)]
fn interruption(stage: &str) {
    if std::env::var("CRYPTEX_TEST_M9_MIGRATION_EXIT").as_deref() == Ok(stage) {
        std::process::exit(76);
    }
}
