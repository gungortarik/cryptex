// Compiled only in disposable test checkouts of the two release revisions.
// Both releases independently verify the same public anchor and signed history.
use super::*;
use cryptex_core::DeviceId;
use cryptex_events::{LocalTrustAnchor, SigningKeyId};
use std::fs;

#[cfg(feature = "m17-owner-transport")]
#[test]
#[ignore = "requires both official packages in a private disposable directory"]
fn recognize_retained_package() {
    use crate::syncthing_package::{
        PackageError, PinnedSyncthingPackage, verify_previous_owner_package,
    };
    let root = PathBuf::from(std::env::var_os("CRYPTEX_M17_RELEASE_PAIR").unwrap());
    let previous = root.join("2.1.5");
    let candidate = root.join("2.1.7");
    verify_previous_owner_package(&previous).unwrap();
    assert!(PinnedSyncthingPackage::open(&previous).is_err());
    PinnedSyncthingPackage::open(&candidate).unwrap();
    assert!(verify_previous_owner_package(&candidate).is_err());
    let notice = previous.join("AUTHORS.txt");
    let mut bytes = fs::read(&notice).unwrap();
    bytes[0] ^= 1;
    fs::write(notice, bytes).unwrap();
    assert_eq!(
        verify_previous_owner_package(&previous),
        Err(PackageError::DigestMismatch)
    );
}

#[cfg(feature = "m17-owner-transport")]
#[test]
#[ignore = "requires a stopped pair of official native engines"]
fn stage_stopped_engine() {
    let root = PathBuf::from(std::env::var_os("CRYPTEX_M17_ENGINE_SCENARIO").unwrap());
    let source = root.join("second");
    assert!(source.join("pilot-owned.json").is_file());
    assert!(source.join("cert.pem").is_file());
    assert!(source.join("key.pem").is_file());
    let staged = root.join("staged-engine");
    assert!(staged.read_dir().unwrap().next().is_none());
    crate::import_staging::export_fs::cold_native::stage(&source, &staged).unwrap();
}

#[test]
#[ignore = "requires two native release builds and an exclusive temporary directory"]
fn paired_exchange_release_handoff() {
    let root = PathBuf::from(std::env::var_os("CRYPTEX_PAIRED_HANDOFF").unwrap());
    let phase = std::env::var("CRYPTEX_PAIRED_PHASE").unwrap();
    let source = root.join("source");
    let receiver = root.join("receiver");
    let anchor_path = root.join("public-anchor.json");
    if phase == "write_previous" {
        assert_eq!(crate::syncthing_package::VERSION, "2.1.5");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&receiver).unwrap();
        let store = crate::FakeSecureStore::new();
        crate::trusted_import_probe::tests::enroll_transport_pair(
            &source, &receiver, LOCAL, REMOTE, &store,
        );
        let brain = crate::identity_runtime::load_brain_identity(&source).unwrap();
        let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            &source,
            &brain.brain_id,
            &store,
            crate::SecureStoreEnvironment::Dev,
        )
        .unwrap();
        let anchor = resolver.local_trust_anchor().unwrap();
        let proof = (
            anchor.device_id().as_str(),
            anchor.signing_key_id().as_str(),
            anchor.public_key(),
        );
        fs::write(&anchor_path, serde_json::to_vec(&proof).unwrap()).unwrap();
    }
    let (device, key, public): (String, String, [u8; 32]) =
        serde_json::from_slice(&fs::read(anchor_path).unwrap()).unwrap();
    let anchor = LocalTrustAnchor::new(
        DeviceId::new(device).unwrap(),
        SigningKeyId::parse(&key).unwrap(),
        public,
    );
    let brain = crate::identity_runtime::load_brain_identity(&source).unwrap();
    let snapshot =
        crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
            &source,
            &brain.brain_id,
            &anchor,
            None,
        )
        .unwrap();
    let peers = ExchangePeers::from_snapshot(&snapshot, &source, LOCAL).unwrap();
    assert_eq!(peers.bindings().len(), 1);
    let ports = NativePorts::new(18384, 22001).unwrap();
    let policy = ExchangeConfigPolicy::new(
        &peers,
        ports,
        "192.168.77.1:22001".parse().unwrap(),
        BTreeMap::from([(REMOTE.into(), "192.168.77.2:22002".parse().unwrap())]),
    )
    .unwrap();
    let bootstrap = native_xml::inspect(
        include_bytes!("../../native_xml/exchange/bootstrap-test.xml"),
        ports,
        true,
    )
    .unwrap();
    let original = root.join("native");
    let home = PrivateDirectory::acquire(&original, phase == "write_previous").unwrap();
    match phase.as_str() {
        "write_previous" => {
            seed(&home, Leaf::Config.name(), &bootstrap.bytes);
            install_native(&home, &policy).unwrap();
            verify_native(&home, &policy).unwrap();
        }
        #[cfg(feature = "m17-owner-transport")]
        "stage_candidate" => {
            assert_eq!(crate::syncthing_package::VERSION, "2.1.7");
            assert!(verify_native(&home, &policy).is_err());
            let stage = PrivateDirectory::acquire(&root.join("staged"), true).unwrap();
            crate::managed_profile::exchange_install::owner_upgrade::stage_exchange_upgrade(
                &home,
                &stage,
                &bootstrap.bytes,
                &policy,
            )
            .unwrap();
        }
        "verify_candidate" | "verify_restored" => {
            let pin = if phase == "verify_candidate" {
                "2.1.7"
            } else {
                "2.1.5"
            };
            assert_eq!(crate::syncthing_package::VERSION, pin);
            verify_native(&home, &policy).unwrap();
        }
        _ => panic!("unexpected test phase"),
    }
}
