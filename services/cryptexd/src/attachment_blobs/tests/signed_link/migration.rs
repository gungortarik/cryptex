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

pub(super) fn exercise_existing_profile(root: &Path, old_daemon: &OsStr, key: &SigningKey) {
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
    let (first, _) = note_and_link(key);
    let second = accept_event(
        SignableEvent {
            schema_version: 1,
            event_id: LINK.into(),
            brain_id: BRAIN.into(),
            device_id: DEVICE.into(),
            device_seq: DeviceSeq::from_u64(2).unwrap(),
            created_at: TIMESTAMP.into(),
            event_type: EventType::MemoryNote,
            classification: Classification::Private,
            signing_key_id: first.signing_key_id.clone(),
            payload: EventPayload::MemoryNote(
                MemoryNotePayload::new("Second synthetic note").unwrap(),
            ),
            prev_content_hash: first.content_hash.clone(),
        },
        key,
    )
    .unwrap();
    let device = DeviceId::new(DEVICE).unwrap();
    let signed = [first, second].map(|e| serde_json_canonicalizer::to_vec(&e).unwrap());
    cryptex_storage::append_frame(root, &device, 0, &signed[0]).unwrap();
    cryptex_storage::seal_active_segment(root, &device, 0).unwrap();
    cryptex_storage::append_frame(root, &device, 1, &signed[1]).unwrap();
    let paths = [
        cryptex_storage::sealed_segment_path(root, &device, 0),
        cryptex_storage::active_segment_path(root, &device, 1),
    ];
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
                TrustedTestDevice(key.verifying_key())
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
    println!(
        "existing_profile_migration=verified signed_records=2 crash_points=3 stale_m8_retry=passed"
    );
}
