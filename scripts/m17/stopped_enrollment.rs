//! Injected only into the disposable candidate service test build.
use super::*;
use crate::managed_profile::connection_settings::{Error, Location};
use crate::secure_store::{FakeSecureStore, SecureStoreEnvironment};
use cryptex_events::TrustedDaemonKeyResolver;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[test]
#[ignore = "requires the previous release reader and both verified packages"]
fn stage_enrolled_previous_release() {
    let root = PathBuf::from(std::env::var_os("CRYPTEX_PAIRED_HANDOFF").unwrap());
    let source = root.join("source");
    let receiver = root.join("receiver");
    let profile_path = source
        .join("managed-transport")
        .join("00000000000000000000000000000001");
    let environment = SecureStoreEnvironment::Dev;
    let store = FakeSecureStore::new();
    let ports = NativePorts::new(18384, 22001).unwrap();
    #[cfg(target_os = "macos")]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let helper = source.join("managed-transport");
        for (name, bytes) in [
            (
                "transport.marker",
                b"m8-native-transport-helper-v1\n".to_vec(),
            ),
            ("ports.json", serde_json::to_vec(&ports).unwrap()),
        ] {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(helper.join(name))
                .unwrap()
                .write_all(&bytes)
                .unwrap();
        }
    }
    let mut profile = ManagedProfile::create(&profile_path, environment).unwrap();
    let xml = include_bytes!("native_xml/exchange/bootstrap-test.xml");
    let prepared = profile
        .prepare_with_generator(&store, ports, |generation| {
            for (leaf, bytes) in [
                ("config.xml", xml.as_slice()),
                ("cert.pem", b"-----BEGIN CERTIFICATE-----\nSYNTHETIC\n"),
                ("key.pem", b"-----BEGIN PRIVATE KEY-----\nSYNTHETIC\n"),
            ] {
                ensure_same_or_new(generation, leaf, bytes)?;
            }
            Ok(())
        })
        .unwrap();
    let local = prepared.configured_transport_device_id().to_owned();
    drop(prepared);
    drop(profile);
    let remote = "XAAPCAD-GA4SF4J-BNQL3FS-T456WKV-FGLXUKO-CDWHVDW-EYWNRIG-SM5GUAZ";
    crate::trusted_import_probe::tests::enroll_transport_pair(
        &source, &receiver, &local, remote, &store,
    );
    let brain = crate::identity_runtime::load_brain_identity(&source).unwrap();
    let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
        &source,
        &brain.brain_id,
        &store,
        environment,
    )
    .unwrap();
    let anchor = resolver.local_trust_anchor().unwrap();
    fs::write(
        root.join("public-anchor.json"),
        serde_json::to_vec(&(
            anchor.device_id().as_str(),
            anchor.signing_key_id().as_str(),
            anchor.public_key(),
        ))
        .unwrap(),
    )
    .unwrap();
    let receiver_id =
        crate::local_device_identity::load_local_device_identity(&receiver, &brain.brain_id)
            .unwrap();
    let saved = serde_json::to_vec(&serde_json::json!({
        "schema": 1, "revision": 4, "paused": false, "listen": "192.168.77.1",
        "peers": [{"device_id": receiver_id.device_id, "transport_id": remote, "endpoint": "192.168.77.2:22002"}]
    })).unwrap();
    let old_test = "managed_profile::exchange_install::tests::paired_release_handoff::paired_exchange_release_handoff";
    let output = Command::new(std::env::var_os("CRYPTEX_PREVIOUS_TEST").unwrap())
        .args([old_test, "--exact", "--ignored"])
        .env("CRYPTEX_PAIRED_PHASE", "install_enrolled_previous")
        .output()
        .unwrap();
    assert!(
        output.status.success()
            && String::from_utf8_lossy(&output.stdout).contains(&format!("test {old_test} ... ok"))
    );
    let native = profile_path.join("native");
    let settings_path = source.join("connection-settings");
    let settings = PrivateDirectory::acquire(&settings_path, true).unwrap();
    let settings_file = settings_path.join("connections.json");
    ensure_same_or_new(&settings, "connections.json", &saved).unwrap();
    drop(settings);
    let pkg = PinnedSyncthingPackage::open(&root.join("2.1.7")).unwrap();
    let stage = source.join("managed-transport-update-native");
    let attempt = |store: &dyn SecureStore| {
        Location::stage_stopped_owner_transport(&source, &pkg, store, environment)
    };
    let old_journal = fs::read(native.join("exchange-install.json")).unwrap();
    let lock = crate::instance::try_acquire_instance(&source).unwrap();
    assert_eq!(attempt(&store), Err(Error::Held));
    drop(lock);
    assert!(attempt(&FakeSecureStore::new()).is_err());
    assert!(!stage.exists());
    #[cfg(target_os = "macos")]
    {
        let active = source.join("managed-transport").join("transport.active");
        fs::write(&active, b"outstanding native helper").unwrap();
        assert!(attempt(&store).is_err());
        assert!(!stage.exists());
        fs::remove_file(active).unwrap();
    }
    let mut changed: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    changed["peers"][0]["endpoint"] = "192.168.77.3:22002".into();
    fs::write(&settings_file, serde_json::to_vec(&changed).unwrap()).unwrap();
    assert!(attempt(&store).is_err());
    assert!(!stage.join("exchange-install.json").exists());
    fs::write(&settings_file, &saved).unwrap();
    fs::write(native.join("index.db"), b"synthetic cold database").unwrap();
    attempt(&store).unwrap();
    attempt(&store).unwrap();
    for leaf in ["config.xml", "index.db", "cert.pem", "key.pem"] {
        assert!(fs::read(stage.join(leaf)).unwrap() == fs::read(native.join(leaf)).unwrap());
    }
    assert!(old_journal == fs::read(native.join("exchange-install.json")).unwrap());
    assert!(old_journal != fs::read(stage.join("exchange-install.json")).unwrap());
    assert!(fs::read(&settings_file).unwrap() == saved);
}
