//! Switch a stopped enrollment between its two verified generations. Both
//! directions detach the executable before moving a database, then publish
//! the matching executable last. A retry identifies directories by their
//! original OS identities even after the candidate has written its database.
use crate::import_staging::export_fs::Directory;
use crate::syncthing_package::{PinnedSyncthingPackage, verify_previous_owner_package};
use serde::{Deserialize, Serialize};
use std::path::Path;

type Id = (u64, u64, u64);
type Result<T> = std::result::Result<T, TransitionError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionError {
    Busy,
    UnsafeDirectory,
    UnexpectedGeneration,
    Package,
}
impl From<crate::import_staging::export_fs::Error> for TransitionError {
    fn from(_: crate::import_staging::export_fs::Error) -> Self {
        Self::UnsafeDirectory
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Versions {
    previous: Id,
    candidate: Id,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransportTransition {
    runtime: Id,
    enrollment: Id,
    native: Versions,
    package: Versions,
}

struct Slots<'a> {
    live: &'a Directory,
    runtime: &'a Directory,
    active: &'static str,
    pending: &'static str,
    retained: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Position {
    Previous,
    Detached,
    Candidate,
}

impl Slots<'_> {
    fn capture(&self) -> Result<Versions> {
        if self.runtime.exists(self.retained)? {
            return Err(TransitionError::UnexpectedGeneration);
        }
        Ok(Versions {
            previous: self.live.child(self.active, false)?.identity()?,
            candidate: self.runtime.child(self.pending, false)?.identity()?,
        })
    }

    fn position(&self, versions: &Versions) -> Result<Position> {
        let read = |parent: &Directory, name: &str| -> Result<Option<Id>> {
            if parent.exists(name)? {
                Ok(Some(parent.child(name, false)?.identity()?))
            } else {
                Ok(None)
            }
        };
        let observed = (
            read(self.live, self.active)?,
            read(self.runtime, self.pending)?,
            read(self.runtime, self.retained)?,
        );
        let old = Some(versions.previous);
        let new = Some(versions.candidate);
        if old != new {
            if observed == (old, new, None) {
                return Ok(Position::Previous);
            }
            if observed == (None, new, old) {
                return Ok(Position::Detached);
            }
            if observed == (new, None, old) {
                return Ok(Position::Candidate);
            }
        }
        Err(TransitionError::UnexpectedGeneration)
    }

    fn detach(&self, from: Position, versions: &Versions) -> Result<()> {
        let destination = match from {
            Position::Detached => return Ok(()),
            Position::Previous => self.retained,
            Position::Candidate => self.pending,
        };
        self.live
            .publish_child(self.active, self.runtime, destination)?;
        if self.position(versions)? != Position::Detached {
            return Err(TransitionError::UnexpectedGeneration);
        }
        #[cfg(test)]
        interruption(self.active, "detach");
        Ok(())
    }

    fn attach(&self, target: Position, versions: &Versions) -> Result<()> {
        let source = match target {
            Position::Previous => self.retained,
            Position::Candidate => self.pending,
            Position::Detached => return Err(TransitionError::UnexpectedGeneration),
        };
        self.runtime.publish_child(source, self.live, self.active)?;
        if self.position(versions)? != target {
            return Err(TransitionError::UnexpectedGeneration);
        }
        #[cfg(test)]
        interruption(self.active, "attach");
        Ok(())
    }
}

#[cfg(test)]
fn interruption(slot: &str, operation: &str) {
    if std::env::var("CRYPTEX_TEST_STOP_TRANSPORT_MOVE").as_deref()
        == Ok(&format!("{operation}-{slot}"))
    {
        std::process::exit(76);
    }
}

struct Enrollment {
    runtime: Directory,
    helper: Directory,
    profile: Directory,
}
impl Enrollment {
    fn open(root: &Path) -> Result<Self> {
        crate::paths::assert_outside_repository(root)
            .map_err(|_| TransitionError::UnsafeDirectory)?;
        let runtime = Directory::open(root)?;
        let helper = runtime.child("managed-transport", false)?;
        let profile = helper.child("00000000000000000000000000000001", false)?;
        Ok(Self {
            runtime,
            helper,
            profile,
        })
    }
    fn native(&self) -> Slots<'_> {
        Slots {
            live: &self.profile,
            runtime: &self.runtime,
            active: "native",
            pending: "managed-transport-update-native",
            retained: "managed-transport-previous-native",
        }
    }
    fn package(&self) -> Slots<'_> {
        Slots {
            live: &self.helper,
            runtime: &self.runtime,
            active: "package",
            pending: "managed-transport-update-package",
            retained: "managed-transport-previous-package",
        }
    }
}

impl TransportTransition {
    pub(super) fn capture(root: &Path) -> Result<Self> {
        let dirs = Enrollment::open(root)?;
        Ok(Self {
            runtime: dirs.runtime.identity()?,
            enrollment: dirs.profile.identity()?,
            native: dirs.native().capture()?,
            package: dirs.package().capture()?,
        })
    }
    pub fn activate(&self, root: &Path) -> Result<()> {
        self.select(root, Position::Candidate)
    }
    pub fn restore(&self, root: &Path) -> Result<()> {
        self.select(root, Position::Previous)
    }
    fn select(&self, root: &Path, target: Position) -> Result<()> {
        let dirs = Enrollment::open(root)?;
        let _instance =
            crate::instance::try_acquire_instance(root).map_err(|_| TransitionError::Busy)?;
        let _lease = super::ManagedProfile::open(
            &root
                .join("managed-transport")
                .join("00000000000000000000000000000001"),
            crate::secure_store::SecureStoreEnvironment::Dev,
        )
        .map_err(|_| TransitionError::Busy)?;
        if self.runtime != dirs.runtime.identity()?
            || self.enrollment != dirs.profile.identity()?
            || dirs.helper.exists("transport.active")?
        {
            return Err(TransitionError::UnexpectedGeneration);
        }
        let package = dirs.package();
        let native = dirs.native();
        let engine = package.position(&self.package)?;
        let db = native.position(&self.native)?;
        // The switch can expose either a matched pair or no active engine.
        if engine != Position::Detached && engine != db {
            return Err(TransitionError::UnexpectedGeneration);
        }
        let active = root.join("managed-transport").join("package");
        let previous = if engine == Position::Previous {
            active.clone()
        } else {
            root.join(package.retained)
        };
        let candidate = if engine == Position::Candidate {
            active
        } else {
            root.join(package.pending)
        };
        verify_previous_owner_package(&previous).map_err(|_| TransitionError::Package)?;
        // Recovery must still work after damage to a rejected candidate.
        if target == Position::Candidate {
            PinnedSyncthingPackage::open_owner_slot(&candidate)
                .map_err(|_| TransitionError::Package)?;
        }
        if engine == target {
            return Ok(());
        }
        package.detach(engine, &self.package)?;
        if db != target {
            native.detach(db, &self.native)?;
            native.attach(target, &self.native)?;
        }
        package.attach(target, &self.package)
    }
}
