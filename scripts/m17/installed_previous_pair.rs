//! Included only in the 1.0.1 service libtest on a disposable native runner.
use super::*;
use crate::bootstrap_journal::BootstrapJournalStore;
use crate::managed_profile::ManagedProfile;
use crate::syncthing_package::PinnedSyncthingPackage;
use cryptex_core::DeviceId;
use cryptex_events::BootstrapApprovalRole;
use std::fs;

#[test]
#[ignore = "requires a fresh account-owned profile and official previous release package"]
fn installed_previous_pair() {
    assert_eq!(
        std::env::var("RUNNER_ENVIRONMENT").unwrap(),
        "github-hosted"
    );
    assert_eq!(crate::syncthing_package::VERSION, "2.1.5");
    let root = PathBuf::from(std::env::var_os("CRYPTEX_TEST_ENROLLED_ROOT").unwrap());
    let instance = crate::instance::try_acquire_instance(&root).unwrap();
    let brain = "11111111-1111-4111-8111-111111111111";
    let environment = crate::SecureStoreEnvironment::Dev;
    #[cfg(target_os = "macos")]
    let store = crate::MacosKeychainStore::new();
    #[cfg(windows)]
    let store = crate::WindowsCredentialStore::new();
    crate::install_runtime::prepare_m1_runtime_foundation(&root).unwrap();
    crate::signed_transfer_probe::create(
        &root,
        &store,
        brain,
        "55555555-5555-4555-8555-555555555555",
    )
    .unwrap();
    let device = DeviceId::new(
        crate::local_device_identity::load_local_device_identity(&root, brain)
            .unwrap()
            .device_id,
    )
    .unwrap();
    let helper = root.join("managed-transport");
    let home = helper.join("00000000000000000000000000000001");
    let ports = NativePorts::new(18384, 22001).unwrap();
    let package = PinnedSyncthingPackage::open(&helper.join("package")).unwrap();
    let mut profile = ManagedProfile::create(&home, environment).unwrap();
    let native = profile.prepare_native(&package, &store, ports).unwrap();
    let transport = native.configured_transport_device_id().to_owned();
    drop(native);
    drop(profile);
    let key = crate::event_signing_key::load_existing_event_signing_key(
        &store,
        environment,
        &root,
        &device,
    )
    .unwrap();
    let peer = DeviceId::new("44444444-4444-4444-8444-444444444444").unwrap();
    let peer_key = ed25519_dalek::SigningKey::from_bytes(&[35; 32]);
    let tx = "b".repeat(64);
    let offer = cryptex_events::create_bootstrap_offer(
        brain,
        &tx,
        &device,
        &transport,
        &[1; 32],
        &key.signing_key,
    )
    .unwrap();
    let proof =
        cryptex_events::create_bootstrap_proof(&offer, &peer, REMOTE, &[2; 32], &peer_key).unwrap();
    let request = cryptex_events::verify_bootstrap_approval_request(
        &[&offer, &proof],
        BootstrapApprovalRole::Initiator,
    )
    .unwrap();
    let first = cryptex_events::sign_bootstrap_approval(&request, &key.signing_key).unwrap();
    let response = cryptex_events::verify_bootstrap_approval_request(
        &[&offer, &proof, &first],
        BootstrapApprovalRole::Responder,
    )
    .unwrap();
    let second = cryptex_events::sign_bootstrap_approval(&response, &peer_key).unwrap();
    let messages = [offer, proof, first, second];
    let journal = BootstrapJournalStore::new(&root).unwrap();
    journal
        .begin_verified_approval_prefix(
            &request,
            &messages[..2],
            "22222222-2222-4222-8222-222222222222",
        )
        .unwrap();
    journal
        .record_local_approval_signature(&request, &messages[2])
        .unwrap();
    journal
        .record_verified_complete_transcript(&tx, &messages)
        .unwrap();
    crate::knowledge_materialization::append_local_peer_approval_anchor(
        &root,
        brain,
        &store,
        environment,
        &tx,
        &messages,
    )
    .unwrap();
    let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
        &root,
        brain,
        &store,
        environment,
    )
    .unwrap();
    let anchor = resolver.local_trust_anchor().unwrap();
    let history =
        crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
            &root, brain, &anchor, None,
        )
        .unwrap();
    let peers = ExchangePeers::from_snapshot(&history, &root, &transport).unwrap();
    assert_eq!(peers.bindings().len(), 1);
    let prefs =
        serde_json::json!({"schema":1,"revision":1,"paused": true,"listen":null,"peers":[]});
    let settings = root.join("connection-settings");
    let _settings = PrivateDirectory::acquire(&settings, true).unwrap();
    {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options
            .open(settings.join("connections.json"))
            .unwrap()
            .write_all(&serde_json::to_vec(&prefs).unwrap())
            .unwrap();
    }
    let policy = ExchangeConfigPolicy::local_only(&peers, ports).unwrap();
    let native = PrivateDirectory::acquire(&home.join("native"), false).unwrap();
    install_native(&native, &policy).unwrap();
    verify_native(&native, &policy).unwrap();
    drop(instance);
}
