//! Manual exports use a separate filename namespace from scheduled generations.
use super::{Error, Prepared};
use crate::import_staging::export_fs::Directory;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

impl Prepared {
    /// Publish the independent public restore checkpoint first. An interrupted
    /// export can leave a checkpoint, but cannot publish ciphertext without one.
    pub fn export_manual_bundle(
        &self,
        codec: &Path,
        recipient: &[u8],
        target: &Path,
        recovery_folder: &Path,
    ) -> Result<(PathBuf, PathBuf), Error> {
        let target_dir = Directory::open(target).map_err(|_| Error::ExportDestination)?;
        let recovery_dir =
            Directory::open(recovery_folder).map_err(|_| Error::ExportDestination)?;
        independent_folders(target, recovery_folder)?;
        let ciphertext = self.encrypt(codec, recipient)?;
        let id = export_id()?;
        let selection = serde_json::to_vec(&self.selection).map_err(|_| Error::Checkpoint)?;
        let selection_name = publish_file(&recovery_dir, id, "json", selection.as_slice())?;
        let name = publish(
            &target_dir,
            id,
            ciphertext.reopen().map_err(|_| Error::Archive)?,
        )?;
        Ok((target.join(name), recovery_folder.join(selection_name)))
    }

    /// Publish a complete ciphertext on the owner's selected volume. Restore
    /// verification is a separate operation against the selected checkpoint.
    pub fn export_manual(
        &self,
        codec: &Path,
        recipient: &[u8],
        target: &Path,
    ) -> Result<PathBuf, Error> {
        let dir = Directory::open(target).map_err(|_| Error::ExportDestination)?;
        let ciphertext = self.encrypt(codec, recipient)?;
        let file = ciphertext.reopen().map_err(|_| Error::Archive)?;
        let name = publish(&dir, export_id()?, file)?;
        Ok(target.join(name))
    }
}

fn export_id() -> Result<u128, Error> {
    let mut nonce = [0; 16];
    getrandom::getrandom(&mut nonce).map_err(|_| Error::ExportPublication)?;
    Ok(u128::from_be_bytes(nonce))
}

/// Compare directory identities, including case aliases on common macOS and Windows volumes.
pub(crate) fn independent_folders(a: &Path, b: &Path) -> Result<(), Error> {
    let a = a.canonicalize().map_err(|_| Error::ExportDestination)?;
    let b = b.canonicalize().map_err(|_| Error::ExportDestination)?;
    for (dir, other) in [(&a, &b), (&b, &a)] {
        for ancestor in other.ancestors() {
            if same_file::is_same_file(dir, ancestor).map_err(|_| Error::ExportDestination)? {
                return Err(Error::ExportDestination);
            }
        }
    }
    Ok(())
}

fn publish(dir: &Directory, id: u128, input: impl Read) -> Result<String, Error> {
    publish_file(dir, id, "pgp", input)
}

fn publish_file(
    dir: &Directory,
    id: u128,
    extension: &str,
    mut input: impl Read,
) -> Result<String, Error> {
    let name = format!("manual-{id:032x}.{extension}");
    let part = format!("manual-{id:032x}.partial");
    if dir.exists(&name).map_err(|_| Error::ExportDestination)? {
        return Err(Error::ExportDestination);
    }
    let mut file = dir
        .open_file(&part, true)
        .map_err(|_| Error::ExportDestination)?;
    if !matches!(io::copy(&mut input, &mut file), Ok(n) if n > 0) {
        dir.discard_partial_manual_export(id, file)
            .map_err(|_| Error::ExportDestination)?;
        return Err(Error::ExportPublication);
    }
    // No replacement. A crash during the copy can leave incomplete data in
    // the partial file. It is never a completed export or retention candidate.
    dir.promote(&part, &file, dir, &name)
        .map_err(|_| Error::ExportPublication)?;
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoint_folder_must_be_independent_of_the_backup_folder() {
        let work = tempfile::tempdir().unwrap();
        let target = work.path().join("target");
        let nested = target.join("nested");
        let other = work.path().join("other");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::create_dir(&other).unwrap();
        for (a, b) in [(&target, &nested), (&nested, &target), (&other, &other)] {
            assert_eq!(independent_folders(a, b), Err(Error::ExportDestination));
        }
        independent_folders(&other, &target).unwrap();
    }

    #[test]
    fn manual_publication_preserves_existing_and_incomplete_exports() {
        let temp = tempfile::tempdir().unwrap();
        let dir = Directory::open(temp.path()).unwrap();
        let saved = publish(&dir, 1, &b"ciphertext"[..]).unwrap();
        assert!(matches!(
            publish(&dir, 1, &b"another"[..]),
            Err(Error::ExportDestination)
        ));
        assert_eq!(
            std::fs::read(temp.path().join(saved)).unwrap(),
            b"ciphertext"
        );
        assert_eq!(publish(&dir, 2, io::empty()), Err(Error::ExportPublication));
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1);
        let interrupted = temp.path().join(format!("manual-{:032x}.partial", 3));
        std::fs::write(&interrupted, b"interrupted").unwrap();
        assert_eq!(
            publish(&dir, 3, &b"retry"[..]),
            Err(Error::ExportDestination)
        );
        assert_eq!(std::fs::read(interrupted).unwrap(), b"interrupted");
        let retry = publish(&dir, 4, &b"retry"[..]).unwrap();
        assert_eq!(std::fs::read(temp.path().join(retry)).unwrap(), b"retry");
    }
}
