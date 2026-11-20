//! Restore only the stopped disposable receiver. The independently delivered
//! owner notice remains available outside this profile in the paired transport.
use super::*;
use crate::test_temp::event_hashes;
use cryptex_projection::{SearchHit, derived_projection_dir};

pub(super) fn exercise(
    root: &Path,
    snapshot: &Path,
    transport: &str,
    keys: &dyn SecureStore,
    survivors: &[SearchHit],
    ledger: &Path,
) {
    let parent = snapshot.parent().unwrap();
    assert_eq!(root, parent.join("purge-profile"));
    assert_eq!(snapshot, parent.join("before-purge"));
    assert!(physical::paired(root));
    let expected_packs = event_hashes(root);
    let approval = fs::read(ledger).unwrap();
    assert!(body_copies(root).is_empty());
    let old_index = derived_projection_dir(snapshot);
    assert!(!body_copies(&old_index).is_empty());
    let index_files: Vec<_> = fs::read_dir(old_index)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            assert!(entry.file_type().unwrap().is_file());
            (entry.file_name(), fs::read(entry.path()).unwrap())
        })
        .collect();
    fs::rename(root, parent.join("clean-replica")).unwrap();
    fs::rename(snapshot, root).unwrap();
    assert!(!ledger.try_exists().unwrap());
    assert_ne!(event_hashes(root), expected_packs);

    let reconcile_and_close = || {
        let mut peer = Side::new(root.to_owned(), transport);
        peer.tick(keys);
        assert_eq!(lifecycle::search(&peer, keys), survivors);
        assert_eq!(fs::read(ledger).unwrap(), approval);
        assert_eq!(event_hashes(root), expected_packs);
        drop(peer);
        assert_eq!(body_copies(root), Vec::<PathBuf>::new());
    };
    reconcile_and_close();

    // Roll only the stopped SQLite projection back, keeping the current signed
    // history and approval. No new notice is applicable on this second start.
    let index = derived_projection_dir(root);
    fs::rename(&index, parent.join("clean-projection")).unwrap();
    fs::create_dir(&index).unwrap();
    for (name, bytes) in index_files {
        fs::write(index.join(name), bytes).unwrap();
    }
    assert!(!body_copies(&index).is_empty());
    assert_eq!(event_hashes(root), expected_packs);
    reconcile_and_close();
    println!("purge_snapshot_restore=verified full=clean projection_only=clean survivors=2");
}
