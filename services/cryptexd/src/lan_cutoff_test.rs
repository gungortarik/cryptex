//! Disposable, interactive two-host fixture, invoked only by the LAN packet.
//! All signed messages and segments move through the two native transport folders.
use super::*;
use crate::bootstrap_journal::{BootstrapJournalStore, RestrictionReason};
use crate::canonical_writer::{AppendMemoryNoteArgs, CanonicalWriterConfig, append_memory_note};
use crate::event_signing_key::load_existing_event_signing_key;
use cryptex_core::{BrainIdentity, DeviceId};
use cryptex_events::{BootstrapApprovalRole, Classification};
use std::{
    fs,
    io::Read,
    time::{Duration, Instant},
};

const ENV: SecureStoreEnvironment = SecureStoreEnvironment::Dev;
const EARLY: &str = "Disposable LAN accepted prefix";
const LATE: &str = "Disposable LAN restricted tail";

struct Profile(PathBuf);
impl Drop for Profile {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove disposable profile");
    }
}

struct Wire {
    outbound: PathBuf,
    inbound: PathBuf,
    round: u8,
}

impl Wire {
    fn send(&self, name: &str, bytes: &[u8]) {
        use std::io::Write;
        let path = self.outbound.join(format!("r{}-{name}", self.round));
        let temporary = self
            .outbound
            .join(format!(".syncthing.r{}-{name}.tmp", self.round));
        let mut file = fs::File::create_new(&temporary).unwrap();
        file.write_all(bytes).unwrap();
        drop(file);
        fs::rename(temporary, path).unwrap();
    }
    fn read(&self, name: &str) -> Vec<u8> {
        let path = self.inbound.join(format!("r{}-{name}", self.round));
        let start = Instant::now();
        loop {
            match fs::symlink_metadata(&path) {
                Ok(meta) => {
                    assert!(meta.is_file() && meta.len() <= 65536);
                    let mut bytes = Vec::new();
                    fs::File::open(&path)
                        .unwrap()
                        .take(65537)
                        .read_to_end(&mut bytes)
                        .unwrap();
                    assert!(bytes.len() <= 65536);
                    return bytes;
                }
                Err(err)
                    if err.kind() == std::io::ErrorKind::NotFound
                        && start.elapsed() < Duration::from_secs(150) =>
                {
                    std::thread::sleep(Duration::from_millis(150));
                }
                Err(err) => panic!("round {} missing {name}: {err}", self.round),
            }
        }
    }
}

fn local_note(
    root: &Path,
    brain: &str,
    device: &DeviceId,
    store: &crate::FakeSecureStore,
    text: &str,
) -> u64 {
    let record = append_memory_note(
        store,
        ENV,
        AppendMemoryNoteArgs {
            runtime_root: root,
            brain_id: brain,
            device_id: device,
            classification: Classification::Public,
            text,
            event_id: None,
            config: &CanonicalWriterConfig {
                seal_event_threshold: 1,
            },
        },
    )
    .unwrap();
    assert!(record.sealed);
    record.segment_index
}

fn round(wire: &Wire, sender: bool, brain: &str, transports: [&str; 2]) {
    let profile = Profile(crate::test_temp::exclusive_temp_dir("cryptex-lan-cutoff"));
    let root = &profile.0;
    let store = crate::FakeSecureStore::new();
    crate::identity_runtime::write_brain_identity(
        root,
        &BrainIdentity::new(brain, "2026-11-18T00:00:00Z").unwrap(),
    )
    .unwrap();
    let identity = crate::local_device_identity::enroll_local_device(root, &store, ENV).unwrap();
    let device = DeviceId::new(identity.device_id).unwrap();
    local_note(root, brain, &device, &store, "Disposable peer genesis");
    let key = load_existing_event_signing_key(&store, ENV, root, &device).unwrap();
    let mut nonce = [0; 32];
    getrandom::getrandom(&mut nonce).unwrap();
    let tx = format!("{:064x}", wire.round);
    let mut messages = Vec::new();
    for (index, name) in ["offer", "proof", "approval-a", "approval-b"]
        .into_iter()
        .enumerate()
    {
        let refs = messages.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let msg = if sender == (index % 2 == 0) {
            let local = match index {
                0 => cryptex_events::create_bootstrap_offer(
                    brain,
                    &tx,
                    &device,
                    transports[0],
                    &nonce,
                    &key.signing_key,
                )
                .unwrap(),
                1 => cryptex_events::create_bootstrap_proof(
                    refs[0],
                    &device,
                    transports[0],
                    &nonce,
                    &key.signing_key,
                )
                .unwrap(),
                _ => {
                    let role = if sender {
                        BootstrapApprovalRole::Initiator
                    } else {
                        BootstrapApprovalRole::Responder
                    };
                    let req =
                        cryptex_events::verify_bootstrap_approval_request(&refs, role).unwrap();
                    assert_eq!(req.local_identity().device_id, device);
                    assert_eq!(req.peer_transport_device_id(), transports[1]);
                    cryptex_events::sign_bootstrap_approval(&req, &key.signing_key).unwrap()
                }
            };
            wire.send(name, &local);
            local
        } else {
            wire.read(name)
        };
        messages.push(msg);
    }
    let prefix = if sender { 2 } else { 3 };
    let role = if sender {
        BootstrapApprovalRole::Initiator
    } else {
        BootstrapApprovalRole::Responder
    };
    let refs = messages.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let req = cryptex_events::verify_bootstrap_approval_request(&refs[..prefix], role).unwrap();
    let peer = req.peer_identity().device_id.clone();
    let journal = BootstrapJournalStore::new(root).unwrap();
    let event_id = if sender {
        "22222222-2222-4222-8222-222222222222"
    } else {
        "33333333-3333-4333-8333-333333333333"
    };
    journal
        .begin_verified_approval_prefix(&req, &messages[..prefix], event_id)
        .unwrap();
    journal
        .record_local_approval_signature(&req, &messages[prefix])
        .unwrap();
    journal
        .record_verified_complete_transcript(&tx, &messages)
        .unwrap();
    crate::knowledge_materialization::append_local_peer_approval_anchor(
        root, brain, &store, ENV, &tx, &messages,
    )
    .unwrap();
    let segment = |device: &DeviceId, idx: u64| {
        PathBuf::from(device.as_filename_stem()).join(format!("{idx:020}.seg"))
    };
    if sender {
        let end = local_note(root, brain, &device, &store, EARLY);
        assert!(end < 8);
        for i in 0..=end {
            wire.send(
                &format!("prefix-{i}"),
                &fs::read(root.join("canonical/events").join(segment(&device, i))).unwrap(),
            );
        }
        wire.send("tip", &[end as u8]);
        assert_eq!(wire.read("cutoff"), b"confirmed");
        let next = local_note(root, brain, &device, &store, LATE);
        assert_eq!(next, end + 1);
        wire.send(
            "tail",
            &fs::read(root.join("canonical/events").join(segment(&device, next))).unwrap(),
        );
        assert_eq!(wire.read("verdict"), b"refused-twice");
        wire.send("ack", b"received");
        return;
    }
    let _ = crate::canonical_writer::acquire_canonical_write_lock_set(
        root,
        &[device.clone(), peer.clone()],
    )
    .unwrap();
    {
        let mut instance = crate::instance::try_acquire_instance(root).unwrap();
        recover_live_with_instance(root, &mut instance, &store, ENV).unwrap();
    }
    let tip = wire.read("tip");
    assert!(tip.len() == 1 && tip[0] < 8);
    let end = u64::from(tip[0]);
    let inbox = root.join("transport/incoming");
    fs::create_dir_all(inbox.join(peer.as_filename_stem())).unwrap();
    let mut paths = Vec::new();
    for i in 0..=end {
        let path = segment(&peer, i);
        fs::write(inbox.join(&path), wire.read(&format!("prefix-{i}"))).unwrap();
        paths.push(path);
    }
    assert_eq!(
        publish_incoming(root, &paths, &store, ENV)
            .unwrap()
            .published_segments,
        paths.len()
    );
    assert!(tests::notes_at(root, brain, &store).contains(EARLY));
    tests::restrict_peer(
        root,
        &store,
        brain,
        &device,
        &peer,
        RestrictionReason::Compromised,
    );
    let before = tests::event_hashes(root);
    wire.send("cutoff", b"confirmed");
    let tail = segment(&peer, end + 1);
    fs::write(inbox.join(&tail), wire.read("tail")).unwrap();
    for _ in 0..2 {
        let mut instance = crate::instance::try_acquire_instance(root).unwrap();
        recover_live_with_instance(root, &mut instance, &store, ENV).unwrap();
        let result = publish_incoming_with_instance(
            root,
            std::slice::from_ref(&tail),
            &mut instance,
            &store,
            ENV,
        );
        assert!(
            matches!(&result,Err(IncomingPublicationError::Rejected(ImportStagingError::Invalid(reason))) if reason=="staged content lies beyond the post-import effective trust cutoff"),
            "{result:?}"
        );
        assert_eq!(tests::event_hashes(root), before);
        let notes = tests::notes_at(root, brain, &store);
        assert!(notes.contains(EARLY) && !notes.contains(LATE));
    }
    wire.send("verdict", b"refused-twice");
    assert_eq!(wire.read("ack"), b"received");
}

#[test]
#[ignore = "invoked by the paired disposable LAN packet"]
fn paired_native_exchange() {
    let setting = |name| std::env::var(format!("CRYPTEX_CUTOFF_{name}")).unwrap();
    let side = setting("SIDE");
    assert!(side == "A" || side == "B");
    let local = setting("LOCAL_ID");
    let peer = setting("PEER_ID");
    for n in 1..=2 {
        let wire = Wire {
            outbound: setting("OUT").into(),
            inbound: setting("IN").into(),
            round: n,
        };
        round(
            &wire,
            (side == "A") == (n == 1),
            &setting("BRAIN"),
            [&local, &peer],
        );
    }
}
