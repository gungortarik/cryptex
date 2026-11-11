use super::*;
use std::fs;
use std::time::Duration;

const CERT: &[u8] = include_bytes!("fixtures/signer.der");
const RAW: &[u8] = include_bytes!("fixtures/valid.json");
const CMS: &[u8] = include_bytes!("fixtures/valid.cms");

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(10)
}
fn trust() -> ReleaseTrust {
    ReleaseTrust {
        signer_der: CERT,
        release_id: "fixture-v1",
        architecture: "arm64",
        publisher: PublisherDefinition {
            team: "TESTTEAM01",
            desktop_identifier: "com.example.cryptex.desktop",
            daemon_identifier: "com.example.cryptex.daemon",
        },
    }
}
fn verified() -> VerifiedInventory {
    VerifiedInventory::verify(RAW, CMS, trust(), deadline()).unwrap()
}

#[test]
fn cms_requires_exact_signed_content_and_independently_pinned_signer() {
    verified();
    let mut edited = RAW.to_vec();
    edited[0] = b' ';
    assert!(matches!(
        VerifiedInventory::verify(&edited, CMS, trust(), deadline()),
        Err(Error::Signature)
    ));
    let mut edited = CMS.to_vec();
    let len = edited.len();
    edited[len - 1] ^= 1;
    assert!(matches!(
        VerifiedInventory::verify(RAW, &edited, trust(), deadline()),
        Err(Error::Signature)
    ));
    let mut policy = trust();
    policy.signer_der = b"untrusted certificate";
    assert!(matches!(
        VerifiedInventory::verify(RAW, CMS, policy, deadline()),
        Err(Error::Signature)
    ));
    assert!(matches!(
        VerifiedInventory::verify(RAW, &[], trust(), deadline()),
        Err(Error::Limit)
    ));
    assert!(matches!(
        VerifiedInventory::verify(RAW, CMS, trust(), Instant::now()),
        Err(Error::Deadline)
    ));
    let mut policy = trust();
    policy.architecture = "x86_64";
    assert!(matches!(
        VerifiedInventory::verify(RAW, CMS, policy, deadline()),
        Err(Error::Release)
    ));
}

#[test]
fn signed_but_invalid_metadata_is_rejected() {
    for (raw, cms, expected) in [
        (
            include_bytes!("fixtures/duplicate.json").as_slice(),
            include_bytes!("fixtures/duplicate.cms").as_slice(),
            Error::Schema,
        ),
        (
            include_bytes!("fixtures/traversal.json").as_slice(),
            include_bytes!("fixtures/traversal.cms").as_slice(),
            Error::Path,
        ),
        (
            include_bytes!("fixtures/wrong-release.json").as_slice(),
            include_bytes!("fixtures/wrong-release.cms").as_slice(),
            Error::Release,
        ),
    ] {
        assert!(
            matches!(VerifiedInventory::verify(raw, cms, trust(), deadline()), Err(e) if e == expected)
        );
    }
}

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        let mut nonce = [0u8; 16];
        getrandom::getrandom(&mut nonce).unwrap();
        let name: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
        let root = std::env::temp_dir().join(format!("cx-inventory-{name}"));
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        fs::create_dir_all(root.join("Contents/MacOS")).unwrap();
        fs::create_dir(root.join("Contents/Resources")).unwrap();
        fs::write(root.join("Contents/MacOS/Desktop"), b"desktop").unwrap();
        fs::write(root.join("Contents/MacOS/Daemon"), b"daemon").unwrap();
        fs::write(root.join("Contents/Resources/view.html"), b"verified view").unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn snapshot_retains_authenticated_bytes_after_source_replacement() {
    let root = Fixture::new();
    let bundle = verified().snapshot(&root.0, deadline()).unwrap();
    let path = root.0.join("Contents/Resources/view.html");
    fs::remove_file(&path).unwrap();
    fs::write(&path, b"replacement").unwrap();
    assert_eq!(
        bundle.content("Contents/Resources/view.html").unwrap(),
        b"verified view"
    );
    assert!(bundle.content("../outside").is_none());
    assert!(matches!(
        verified().snapshot(&root.0, deadline()),
        Err(Error::Changed)
    ));
    let image = super::super::image_policy::inventory_image(
        bundle.inventory(),
        super::super::image_policy::Role::Daemon,
    )
    .unwrap();
    assert_eq!(image.cdhash(), &[2; 20]);
}

#[test]
fn modification_between_read_and_publication_is_rejected() {
    let root = Fixture::new();
    let result = verified().snapshot_with(&root.0, deadline(), |name| {
        if name == "Contents/MacOS/Daemon" {
            fs::write(root.0.join(name), b"edited").unwrap();
        }
    });
    assert!(matches!(result, Err(Error::Changed)));
}

#[test]
fn changed_short_growing_symlinked_and_missing_members_never_return_a_bundle() {
    for mode in [
        "same_size",
        "short",
        "long",
        "symlink",
        "directory_symlink",
        "missing",
        "hardlink",
    ] {
        let root = Fixture::new();
        let member = root.0.join("Contents/MacOS/Daemon");
        match mode {
            "same_size" => fs::write(&member, b"edited").unwrap(),
            "short" => fs::write(&member, b"x").unwrap(),
            "long" => fs::write(&member, b"daemon growth").unwrap(),
            "symlink" => {
                fs::remove_file(&member).unwrap();
                std::os::unix::fs::symlink("Desktop", &member).unwrap();
            }
            "directory_symlink" => {
                let dir = root.0.join("Contents/MacOS");
                fs::rename(&dir, root.0.join("Contents/moved")).unwrap();
                std::os::unix::fs::symlink("moved", dir).unwrap();
            }
            "hardlink" => fs::hard_link(&member, root.0.join("alias")).unwrap(),
            "missing" => fs::remove_file(&member).unwrap(),
            _ => unreachable!(),
        }
        assert!(verified().snapshot(&root.0, deadline()).is_err(), "{mode}");
    }
}

#[test]
fn path_grammar_and_expired_reads_are_closed() {
    for name in [
        "/Contents/a",
        "Contents/a/../b",
        "Contents/a//b",
        "Contents/a\\b",
        "Contents/a\0",
        "Contents/.",
        "Contents/../outside",
    ] {
        assert_eq!(validate_path(name), Err(Error::Path));
    }
    let root = Fixture::new();
    assert!(matches!(
        verified().snapshot(&root.0, Instant::now()),
        Err(Error::Deadline)
    ));
}
