//! Bounded discovery hints only. Metadata never grants publication/trust.
use super::*;
use std::collections::BTreeMap;
use std::os::unix::fs::MetadataExt;

const MAX_DISCOVERY_ENTRIES: usize = 4096;
#[derive(Clone, PartialEq, Eq)]
struct Fingerprint {
    device: u64,
    inode: u64,
    bytes: u64,
    changed: (i64, i64),
    modified: (i64, i64),
}
impl Fingerprint {
    fn of(file: &File) -> io::Result<Self> {
        let m = file.metadata()?;
        Ok(Self {
            device: m.dev(),
            inode: m.ino(),
            bytes: m.len(),
            changed: (m.ctime(), m.ctime_nsec()),
            modified: (m.mtime(), m.mtime_nsec()),
        })
    }
}
pub(crate) struct Inbox {
    root: PathBuf,
    namespace: macos_fs::RuntimeIdentity,
    seen: BTreeMap<PathBuf, Fingerprint>,
    pending: Vec<(PathBuf, Fingerprint)>,
}
impl Inbox {
    pub(crate) fn new(root: &Path) -> io::Result<Self> {
        Ok(Self {
            root: root.to_owned(),
            namespace: macos_fs::RuntimeIdentity::capture(root)?,
            seen: BTreeMap::new(),
            pending: Vec::new(),
        })
    }
    pub(crate) fn discover(&mut self) -> io::Result<Vec<PathBuf>> {
        self.namespace.validate()?;
        if !self.pending.is_empty() {
            return Ok(self.pending.iter().map(|(p, _)| p.clone()).collect());
        }
        let incoming = self.root.join("transport/incoming");
        let directories = match fs::read_dir(&incoming) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        self.namespace.pin(&incoming)?;
        let mut count = 0usize;
        let mut candidates = BTreeMap::new();
        for directory in directories {
            count += 1;
            if count > MAX_DISCOVERY_ENTRIES {
                return Err(discovery_limit());
            }
            let directory = directory?;
            let path = directory.path();
            if path
                .file_name()
                .is_some_and(|name| name == ".stfolder" || name == super::READER_CONTROL_FOLDER)
            {
                continue;
            }
            if !fs::symlink_metadata(&path)?.is_dir() {
                return Err(discovery_object());
            }
            self.namespace.pin(&path)?;
            for entry in fs::read_dir(&path)? {
                count += 1;
                if count > MAX_DISCOVERY_ENTRIES {
                    return Err(discovery_limit());
                }
                let path = entry?.path();
                if path.extension().is_none_or(|ext| ext != "seg") {
                    continue;
                }
                let relative = path
                    .strip_prefix(&incoming)
                    .map_err(|_| discovery_object())?
                    .to_owned();
                validate_relative_segment_path(&relative).map_err(|_| discovery_object())?;
                let chain =
                    inspect_directory_chain(path.parent().ok_or_else(discovery_object)?, true)
                        .map_err(|_| discovery_object())?;
                let file = open_source_from_directory_chain(&chain, &path)
                    .map_err(|_| discovery_object())?;
                let fingerprint = Fingerprint::of(&file)?;
                if self.seen.get(&relative) != Some(&fingerprint) {
                    candidates.insert(relative, fingerprint);
                }
            }
        }
        self.namespace.validate()?;
        self.pending = candidates
            .into_iter()
            .take(crate::import_limits::MAX_IMPORT_FILES_PER_BATCH)
            .collect();
        Ok(self.pending.iter().map(|(p, _)| p.clone()).collect())
    }
    /// Only an actual clean publication/rejection consumes a discovery hint.
    /// Waiting, deferred and held outcomes keep it for verified handling.
    pub(crate) fn consumed(&mut self) -> io::Result<()> {
        self.namespace.validate()?;
        if self.seen.len()
            + self
                .pending
                .iter()
                .filter(|(p, _)| !self.seen.contains_key(p))
                .count()
            > MAX_DISCOVERY_ENTRIES
        {
            return Err(discovery_limit());
        }
        self.seen.extend(self.pending.drain(..));
        Ok(())
    }
}
fn discovery_object() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "incoming_discovery_unsafe_object",
    )
}
fn discovery_limit() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "incoming_discovery_limit")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_retains_waiting_hints_and_rechecks_consumed_file_changes() {
        let root = crate::test_temp::exclusive_temp_dir("mac-inbox-hints");
        let relative = PathBuf::from("6465766963652d61/00000000000000000000.seg");
        let incoming = root.join("transport/incoming");
        fs::create_dir_all(incoming.join("6465766963652d61")).unwrap();
        std::fs::write(incoming.join(&relative), b"untrusted hint one").unwrap();
        let mut inbox = Inbox::new(&root).unwrap();
        assert_eq!(inbox.discover().unwrap(), vec![relative.clone()]);
        assert_eq!(inbox.discover().unwrap(), vec![relative.clone()]);
        inbox.consumed().unwrap();
        assert!(inbox.discover().unwrap().is_empty());
        std::fs::write(incoming.join(&relative), b"changed untrusted hint two").unwrap();
        assert_eq!(inbox.discover().unwrap(), vec![relative]);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn discovery_refuses_symlinks_and_replaced_observed_directories() {
        let root = crate::test_temp::exclusive_temp_dir("mac-inbox-replaced");
        let incoming = root.join("transport/incoming");
        std::fs::create_dir_all(incoming.join("6465766963652d61")).unwrap();
        let mut inbox = Inbox::new(&root).unwrap();
        assert!(inbox.discover().unwrap().is_empty());
        std::fs::rename(&incoming, root.join("preserved-incoming")).unwrap();
        std::fs::create_dir_all(incoming.join("6465766963652d61")).unwrap();
        assert!(inbox.discover().is_err());
        assert!(root.join("preserved-incoming").is_dir());
        std::fs::remove_dir_all(root).unwrap();
    }
}
