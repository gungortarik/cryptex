use super::*;
use crate::instance::migration::{MigrationError, migrate_profile};
use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

#[test]
#[ignore = "spawned by the migration test with a disposable profile and an exit checkpoint"]
fn interrupted_child() {
    let root = std::env::var_os("CRYPTEX_TEST_M9_PROFILE").unwrap();
    migrate_profile(Path::new(&root)).unwrap();
    panic!("interruption checkpoint was not reached");
}

pub(super) fn exercise_existing_profile(root: &Path, old_daemon: &OsStr) {
    let current = std::env::var_os("CRYPTEX_M9_CANDIDATE_DAEMON").unwrap();
    let run = |bin: &OsStr, command: &str| {
        Command::new(bin)
            .args(["lifecycle", command, "--runtime-root"])
            .arg(root)
            .output()
            .unwrap()
    };
    let lock = crate::instance::try_acquire_instance(root).unwrap();
    let brain = cryptex_core::BrainIdentity::new(BRAIN, TIMESTAMP).unwrap();
    crate::identity_runtime::write_brain_identity(root, &brain).unwrap();
    use crate::canonical_writer::{
        AppendMemoryNoteArgs, CanonicalWriterConfig, append_memory_note,
    };
    use crate::local_device_identity::{LocalDeviceKeyResolver, enroll_local_device};
    use crate::secure_store::{FakeSecureStore, SecureStoreEnvironment};
    let credentials = FakeSecureStore::new();
    let env = SecureStoreEnvironment::Dev;
    let identity = enroll_local_device(root, &credentials, env).unwrap();
    let device = DeviceId::new(identity.device_id.clone()).unwrap();
    let capture = |classification, text, threshold| {
        append_memory_note(
            &credentials,
            env,
            AppendMemoryNoteArgs {
                runtime_root: root,
                brain_id: BRAIN,
                device_id: &device,
                classification,
                text,
                event_id: None,
                config: &CanonicalWriterConfig {
                    seal_event_threshold: threshold,
                },
            },
        )
        .unwrap()
    };
    let first = capture(
        Classification::Restricted,
        "First synthetic migration note",
        1,
    );
    let second = capture(
        Classification::Private,
        "Second synthetic migration note",
        100,
    );
    assert!(first.sealed && !second.sealed);
    let paths = [
        cryptex_storage::sealed_segment_path(root, &device, first.segment_index),
        cryptex_storage::active_segment_path(root, &device, second.segment_index),
    ];
    let signed =
        [first, second].map(|event| serde_json_canonicalizer::to_vec(&event.accepted).unwrap());
    let before = paths.each_ref().map(|p| fs::read(p).unwrap());
    let check = || {
        let packs = paths.each_ref().map(|p| fs::read(p).unwrap());
        assert_eq!(packs, before);
        let bodies = packs.map(|pack| {
            let (frames, consumed) = cryptex_storage::scan_complete_frames(&pack).unwrap();
            assert_eq!((frames.len(), consumed), (1, pack.len()));
            frames[0][cryptex_events::FRAME_HEADER_BYTES..].to_vec()
        });
        assert_eq!(bodies, signed);
        assert_eq!(
            verify_v2_history(
                &[&bodies[0], &bodies[1]],
                LocalDeviceKeyResolver::load(root, BRAIN, &credentials, env).unwrap()
            )
            .unwrap()
            .records()
            .len(),
            2
        );
        assert_eq!(
            crate::identity_runtime::load_brain_identity(root).unwrap(),
            brain
        );
    };
    assert_eq!(migrate_profile(root), Err(MigrationError::Busy));
    assert!(root.join("daemon.lock").is_file());
    drop(lock);
    for step in ["prepared", "retired", "published"] {
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "attachment_blobs::tests::signed_link::migration::interrupted_child",
            ])
            .env("CRYPTEX_TEST_M9_PROFILE", root)
            .env("CRYPTEX_TEST_M9_MIGRATION_EXIT", step)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(76), "{step}");
        check();
        if step == "retired" {
            // M8 takes the vacant name after this crash. Resumption must
            // retire its *new* file under lock before publishing the barrier.
            assert!(run(old_daemon, "probe").status.success());
        }
    }
    for _ in 0..2 {
        let result = run(&current, "migrate-profile-m9");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            String::from_utf8_lossy(&result.stdout).contains("production_v2_activation=blocked")
        );
    }
    assert!(!root.join(crate::instance::migration::STAGED).exists());
    let denied = run(old_daemon, "probe");
    assert_eq!(denied.status.code(), Some(2));
    let code = if cfg!(windows) { 5 } else { 21 };
    assert!(
        String::from_utf8_lossy(&denied.stderr)
            .trim_end()
            .ends_with(&format!("(os error {code})"))
    );
    assert!(run(&current, "probe").status.success());
    check();
    let _instance = crate::instance::try_acquire_instance(root).unwrap();
    let next = capture(Classification::Internal, "Post-migration v1 capture", 100);
    assert_eq!(next.accepted.device_seq.to_u64().unwrap(), 3);
    assert_eq!(next.accepted.device_id, identity.device_id);
    let previous: AcceptedEvent = serde_json::from_slice(&signed[1]).unwrap();
    assert_eq!(next.accepted.prev_content_hash, previous.content_hash);
    assert_eq!(next.accepted.signing_key_id, previous.signing_key_id);
    println!(
        "existing_profile_migration=verified signed_records=2 crash_points=3 stale_m8_retry=passed next_v1_sequence=3"
    );
}
