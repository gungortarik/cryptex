use super::*;
use std::{fs, path::PathBuf};

struct Profile(PathBuf);

impl Profile {
    fn new() -> Self {
        Self(crate::test_temp::exclusive_temp_dir("cryptex-m9-blobs"))
    }

    fn blobs(&self) -> PathBuf {
        self.0.join("canonical/blobs")
    }
}

impl Drop for Profile {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

#[test]
fn stores_raw_bytes_by_standard_sha256_and_verifies_duplicates_after_reopen() {
    let profile = Profile::new();
    let original = store(&profile.0, &b"abc"[..]).unwrap();
    assert_eq!(fs::read(profile.blobs().join(ABC)).unwrap(), b"abc");
    assert_eq!(store(&profile.0, &b"abc"[..]).unwrap(), original);
    assert_ne!(store(&profile.0, io::empty()).unwrap(), original);
    assert_eq!(fs::metadata(profile.blobs().join(EMPTY)).unwrap().len(), 0);
    assert_eq!(fs::read_dir(profile.blobs()).unwrap().count(), 2);
}

#[test]
fn detects_existing_same_length_corruption_and_preserves_evidence() {
    let profile = Profile::new();
    store(&profile.0, &b"abc"[..]).unwrap();
    let blob = profile.blobs().join(ABC);
    fs::write(&blob, b"xyz").unwrap();
    assert_eq!(
        store(&profile.0, &b"abc"[..]),
        Err(BlobError::IntegrityMismatch)
    );
    assert_eq!(fs::read(blob).unwrap(), b"xyz");
    assert!(!profile.0.join("blob-staging/segment.tmp").exists());
}

#[test]
fn refuses_a_linked_blob_without_changing_the_other_file() {
    let profile = Profile::new();
    store(&profile.0, io::empty()).unwrap();
    let other = profile.0.join("unrelated");
    fs::write(&other, b"abc").unwrap();
    fs::hard_link(&other, profile.blobs().join(ABC)).unwrap();
    assert!(store(&profile.0, &b"abc"[..]).is_err());
    assert_eq!(fs::read(other).unwrap(), b"abc");
}

#[test]
fn failed_source_and_abandoned_stage_never_publish_a_partial_blob() {
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::ErrorKind::UnexpectedEof.into())
        }
    }
    let profile = Profile::new();
    assert_eq!(
        store(&profile.0, (&b"prefix"[..]).chain(Broken)),
        Err(BlobError::Io(io::ErrorKind::UnexpectedEof))
    );
    assert_eq!(fs::read_dir(profile.blobs()).unwrap().count(), 0);
    let scratch = profile.0.join("blob-staging/segment.tmp");
    assert!(!scratch.exists());
    fs::write(&scratch, b"interrupted write").unwrap();
    store(&profile.0, &b"abc"[..]).unwrap();
    assert!(!scratch.exists());
    assert_eq!(fs::read(profile.blobs().join(ABC)).unwrap(), b"abc");
}

#[test]
fn concurrent_multi_chunk_submissions_share_one_complete_blob() {
    let profile = Profile::new();
    let bytes: Vec<_> = (0u8..=255).cycle().take(160_003).collect();
    std::thread::scope(|scope| {
        let submit = || store(&profile.0, bytes.as_slice()).unwrap();
        let other = scope.spawn(submit);
        assert_eq!(submit(), other.join().unwrap());
    });
    let mut entries = fs::read_dir(profile.blobs()).unwrap();
    let blob = entries.next().unwrap().unwrap();
    assert!(entries.next().is_none());
    assert_eq!(fs::read(blob.path()).unwrap(), bytes);
}
