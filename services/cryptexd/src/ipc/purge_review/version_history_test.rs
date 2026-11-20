use super::*;
use crate::canonical_writer::{AppendMemoryNoteArgs, append_memory_note};
use crate::ipc::tests::prepared_search_fixture;
use cryptex_events::Classification;
use cryptex_storage::{active_segment_path, sealed_segment_path};
use std::{fs, path::Path};

fn profile_contains(dir: &Path, needle: &[u8]) -> bool {
    fs::read_dir(dir).unwrap().any(|entry| {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            profile_contains(&entry.path(), needle)
        } else {
            assert!(entry.file_type().unwrap().is_file());
            fs::read(entry.path())
                .unwrap()
                .windows(needle.len())
                .any(|part| part == needle)
        }
    })
}

#[test]
#[ignore = "separate native process with a registered disposable profile"]
fn confirmed_purge_scrubs_archived_versions() {
    let mut fixture = prepared_search_fixture("physical-version-history");
    let root = &fixture.root;
    let device = &fixture.device;
    drop(crate::instance::try_acquire_instance(root).unwrap());
    crate::instance::migration::migrate_profile(root).unwrap();
    let instance = crate::instance::try_acquire_instance(root).unwrap();
    physical::register(root);
    let ctx = &mut fixture.context;
    ctx.admission = instance.admission();
    ctx.purge_reviews = Some(Arc::default());
    let append = |text, threshold| {
        append_memory_note(
            fixture.store.as_ref(),
            SecureStoreEnvironment::Dev,
            AppendMemoryNoteArgs {
                runtime_root: root,
                brain_id: &fixture.brain_id,
                device_id: device,
                classification: Classification::Internal,
                text,
                event_id: None,
                config: &CanonicalWriterConfig {
                    seal_event_threshold: threshold,
                },
            },
        )
        .unwrap()
    };
    let older = append("surviving note before the target", 100);
    let index = older.segment_index;
    let active = active_segment_path(root, device, index);
    let before = fs::read(&active).unwrap();
    let secret = "synthetic historical purge target 61bc90";
    let target = append(secret, 100);
    let early = fs::read(&active).unwrap();
    let later = append("surviving note after the target", 1);
    assert!(later.sealed && later.segment_index == index);
    let sealed = sealed_segment_path(root, device, index);
    let full = fs::read(&sealed).unwrap();
    let other = append("independent pack and its own version", 1);
    assert_ne!(other.segment_index, index);
    let other_pack = sealed_segment_path(root, device, other.segment_index);
    let independent = fs::read(&other_pack).unwrap();
    let archive = root
        .join("transport/incoming/.stversions")
        .join(device.as_filename_stem());
    fs::create_dir_all(&archive).unwrap();
    let archive_path = |stamp| archive.join(format!("{index:020}~20261119-{stamp}.seg"));
    let old = archive_path("100000");
    let short = archive_path("110000");
    let mixed = archive_path("120000");
    let trash = archive.join(format!("{index:020}.seg"));
    let other_version = archive.join(format!("{:020}~20261119-120000.seg", other.segment_index));
    for (path, contents) in [
        (&old, &before),
        (&short, &early),
        (&mixed, &full),
        (&trash, &full),
        (&other_version, &independent),
    ] {
        fs::write(path, contents).unwrap();
    }
    synchronize_local_derived_state(ctx).unwrap();
    let prepare = || {
        let Outcome::Prepared {
            ticket,
            remove_locally: true,
        } = decide(
            ctx,
            Action::Prepare {
                note_id: target.accepted.event_id.clone(),
                note_hash: target.accepted.content_hash.as_str().into(),
            },
        )
        .unwrap()
        else {
            panic!("expected physical deletion review")
        };
        ticket
    };
    assert!(matches!(
        decide(ctx, Action::Cancel { ticket: prepare() }).unwrap(),
        Outcome::Cancelled
    ));
    assert_eq!(fs::read(&short).unwrap(), early);
    assert_eq!(fs::read(&mixed).unwrap(), full);
    assert!(matches!(
        decide(ctx, Action::Confirm { ticket: prepare() }).unwrap(),
        Outcome::Purged { .. }
    ));
    let cleaned = fs::read(&sealed).unwrap();
    let survivor_tail = &full[early.len()..];
    assert!(
        !survivor_tail.is_empty()
            && cleaned.starts_with(&before)
            && cleaned.ends_with(survivor_tail)
    );
    let check_disk = || {
        assert_eq!(fs::read(&old).unwrap(), before);
        assert_eq!(
            fs::read(&short).unwrap(),
            &cleaned[..cleaned.len() - survivor_tail.len()]
        );
        assert_eq!(fs::read(&mixed).unwrap(), cleaned);
        assert_eq!(fs::read(&trash).unwrap(), cleaned);
        assert_eq!(fs::read(&other_version).unwrap(), independent);
        assert_eq!(fs::read(&other_pack).unwrap(), independent);
        assert!(!profile_contains(root, secret.as_bytes()));
    };
    drop(instance);
    check_disk();

    // Exercise the retry after the original was claimed and before publication.
    let reopened = crate::instance::try_acquire_instance(root).unwrap();
    let resolver = LocalDeviceKeyResolver::load(
        root,
        &fixture.brain_id,
        fixture.store.as_ref(),
        SecureStoreEnvironment::Dev,
    )
    .unwrap();
    let anchor = resolver.local_trust_anchor().unwrap();
    let retry = || physical::finish_cleanup(root, &fixture.brain_id, &anchor);
    let work = root
        .join("purge-staging/versions")
        .join(device.as_filename_stem());
    fs::rename(&mixed, work.join(mixed.file_name().unwrap())).unwrap();
    fs::write(work.join(mixed.file_name().unwrap()), &full).unwrap();
    fs::write(work.join("segment.tmp"), b"partial").unwrap();
    retry().unwrap();
    // A corrupt tail must never cause a partial archive to replace the original.
    let mut damaged = full.clone();
    damaged.pop();
    fs::write(&trash, &damaged).unwrap();
    assert!(matches!(retry(), Err(ReviewError::PackIntegrity)));
    assert_eq!(fs::read(&trash).unwrap(), damaged);
    fs::write(&trash, &full).unwrap();
    retry().unwrap();
    retry().unwrap();
    drop(reopened);
    check_disk();
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    fs::remove_dir_all(root).unwrap();
    println!("archive_purge=verified");
}
