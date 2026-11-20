//! Manual exports use a separate filename namespace from scheduled generations.
use super::{Error, Prepared};
use crate::import_staging::export_fs::Directory;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

impl Prepared {
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
        let mut nonce = [0; 16];
        getrandom::getrandom(&mut nonce).map_err(|_| Error::ExportPublication)?;
        let name = publish(&dir, u128::from_be_bytes(nonce), file)?;
        Ok(target.join(name))
    }
}

fn publish(dir: &Directory, id: u128, mut input: impl Read) -> Result<String, Error> {
    let name = format!("manual-{id:032x}.pgp");
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
    // No replacement. A crash during the copy can leave only ciphertext in
    // the partial file. It is never a completed export or retention candidate.
    dir.promote(&part, &file, dir, &name)
        .map_err(|_| Error::ExportPublication)?;
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

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
