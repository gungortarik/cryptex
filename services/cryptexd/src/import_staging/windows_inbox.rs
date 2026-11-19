//! Bounded hints from the enrolled exchange tree; never publication authority.
//! Retained no-delete directory handles prevent pathname replacement. Sources
//! are opened relative to those handles and verified again by the publisher.
use super::*;
use std::collections::BTreeMap;

const MAX_ENTRIES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Error {
    Unsafe,
    Limit,
    Io,
}

#[derive(PartialEq, Eq)]
struct Fingerprint {
    source: SourceSnapshot,
    change_time: i64,
}
impl Fingerprint {
    fn of(file: &File) -> Result<Self, Error> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_BASIC_INFO, FileBasicInfo, GetFileInformationByHandleEx,
        };
        let source = source_snapshot(file).map_err(|_| Error::Unsafe)?;
        if !source.is_regular_file || source.link_count != 1 {
            return Err(Error::Unsafe);
        }
        let mut basic: FILE_BASIC_INFO = unsafe { std::mem::zeroed() };
        if unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle(),
                FileBasicInfo,
                (&mut basic as *mut FILE_BASIC_INFO).cast(),
                std::mem::size_of_val(&basic) as u32,
            )
        } == 0
        {
            return Err(Error::Io);
        }
        Ok(Self {
            source,
            change_time: basic.ChangeTime,
        })
    }
}

pub(crate) struct Inbox {
    incoming: PathBuf,
    _runtime: Vec<DirectoryPin>,
    directories: BTreeMap<PathBuf, Vec<DirectoryPin>>,
    seen: BTreeMap<PathBuf, Fingerprint>,
    pending: Vec<(PathBuf, Fingerprint)>,
}
impl Inbox {
    pub(crate) fn new(root: &Path) -> Result<Self, Error> {
        crate::paths::assert_outside_repository(root).map_err(|_| Error::Unsafe)?;
        Ok(Self {
            incoming: root.join("transport/incoming"),
            _runtime: inspect_directory_chain(root, true).map_err(|_| Error::Unsafe)?,
            directories: BTreeMap::new(),
            seen: BTreeMap::new(),
            pending: Vec::new(),
        })
    }
    fn pin(&mut self, path: &Path) -> Result<(), Error> {
        if !self.directories.contains_key(path) {
            if self.directories.len() >= MAX_ENTRIES {
                return Err(Error::Limit);
            }
            self.directories.insert(
                path.to_owned(),
                inspect_directory_chain(path, true).map_err(|_| Error::Unsafe)?,
            );
        }
        Ok(())
    }
    pub(crate) fn discover(&mut self) -> Result<Vec<PathBuf>, Error> {
        if !self.pending.is_empty() {
            return Ok(self.pending.iter().map(|(p, _)| p.clone()).collect());
        }
        let incoming = self.incoming.clone();
        match fs::symlink_metadata(&incoming) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(_) => return Err(Error::Io),
            Ok(_) => {}
        }
        self.pin(&incoming)?;
        let mut count = 0;
        let mut candidates = BTreeMap::new();
        for directory in fs::read_dir(&incoming).map_err(|_| Error::Io)? {
            count_entry(&mut count)?;
            let path = directory.map_err(|_| Error::Io)?.path();
            if path.file_name().is_some_and(|n| {
                n == ".stfolder"
                    || n == super::READER_CONTROL_FOLDER
                    || n == super::ATTACHMENT_FOLDER
            }) {
                continue;
            }
            self.pin(&path)?;
            let pins = self.directories.get(&path).ok_or(Error::Unsafe)?;
            for entry in fs::read_dir(&path).map_err(|_| Error::Io)? {
                count_entry(&mut count)?;
                let path = entry.map_err(|_| Error::Io)?.path();
                if path.extension().is_none_or(|ext| ext != "seg") {
                    continue;
                }
                let relative = path
                    .strip_prefix(&incoming)
                    .map_err(|_| Error::Unsafe)?
                    .to_owned();
                validate_relative_segment_path(&relative).map_err(|_| Error::Unsafe)?;
                let file =
                    open_source_from_directory_chain(pins, &path).map_err(|_| Error::Unsafe)?;
                let fingerprint = Fingerprint::of(&file)?;
                if self.seen.get(&relative) != Some(&fingerprint) {
                    candidates.insert(relative, fingerprint);
                }
            }
        }
        self.pending = candidates
            .into_iter()
            .take(crate::import_limits::MAX_IMPORT_FILES_PER_BATCH)
            .collect();
        Ok(self.pending.iter().map(|(p, _)| p.clone()).collect())
    }
    /// Waiting/deferred/held work stays pending; only publication or a clean
    /// rejection consumes hints. Nothing is removed from the exchange tree.
    pub(crate) fn consumed(&mut self) -> Result<(), Error> {
        if self.seen.len()
            + self
                .pending
                .iter()
                .filter(|(p, _)| !self.seen.contains_key(p))
                .count()
            > MAX_ENTRIES
        {
            return Err(Error::Limit);
        }
        self.seen.extend(self.pending.drain(..));
        Ok(())
    }
}
fn count_entry(count: &mut usize) -> Result<(), Error> {
    *count += 1;
    if *count > MAX_ENTRIES {
        Err(Error::Limit)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retains_hints_pins_directories_and_rediscovers_changes_and_restart() {
        let root = crate::test_temp::exclusive_temp_dir("windows-inbox");
        let incoming = root.join("transport/incoming");
        let relative = PathBuf::from("6465766963652d61/00000000000000000000.seg");
        let mut inbox = Inbox::new(&root).unwrap();
        assert!(inbox.discover().unwrap().is_empty());
        fs::create_dir_all(incoming.join("6465766963652d61")).unwrap();
        fs::create_dir(incoming.join(".stfolder")).unwrap();
        fs::write(incoming.join(&relative), b"untrusted one").unwrap();
        assert_eq!(inbox.discover().unwrap(), vec![relative.clone()]);
        assert_eq!(inbox.discover().unwrap(), vec![relative.clone()]);
        assert!(fs::rename(&incoming, root.join("old-incoming")).is_err());
        inbox.consumed().unwrap();
        assert!(inbox.discover().unwrap().is_empty());
        fs::write(incoming.join(&relative), b"changed untrusted two").unwrap();
        assert_eq!(inbox.discover().unwrap(), vec![relative.clone()]);
        inbox.consumed().unwrap();
        drop(inbox);
        let mut restarted = Inbox::new(&root).unwrap();
        assert_eq!(restarted.discover().unwrap(), vec![relative]);
        drop(restarted);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn refuses_hardlinked_sources_without_consuming_or_changing_them() {
        let root = crate::test_temp::exclusive_temp_dir("windows-inbox-alias");
        let directory = root.join("transport/incoming/6465766963652d61");
        fs::create_dir_all(&directory).unwrap();
        let source = root.join("preserve");
        fs::write(&source, b"unchanged").unwrap();
        fs::hard_link(&source, directory.join("00000000000000000000.seg")).unwrap();
        let mut inbox = Inbox::new(&root).unwrap();
        assert_eq!(inbox.discover(), Err(Error::Unsafe));
        assert!(inbox.pending.is_empty());
        assert_eq!(fs::read(&source).unwrap(), b"unchanged");
        drop(inbox);
        fs::remove_dir_all(root).unwrap();
    }
}
