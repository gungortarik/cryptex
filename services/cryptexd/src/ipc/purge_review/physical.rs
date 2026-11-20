//! Opt-in for the native disposable-profile harness. Production has no entry point.
use super::ReviewError;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub(crate) mod retained;

static PROFILE: OnceLock<PathBuf> = OnceLock::new();

pub(crate) fn register(root: &Path) {
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("cryptex-ipc-search-physical-")
    );
    PROFILE.set(root.to_owned()).unwrap();
}

static PAIR: OnceLock<[PathBuf; 2]> = OnceLock::new();

pub(crate) fn register_pair(base: &Path) {
    assert_eq!(base.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        base.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("cryptex-m9-transfer-")
    );
    PAIR.set([base.join("A/purge-profile"), base.join("B/purge-profile")])
        .unwrap();
}

pub(crate) fn paired(root: &Path) -> bool {
    PAIR.get()
        .is_some_and(|pair| pair.iter().any(|p| p == root))
}

pub(crate) fn enabled(root: &Path) -> bool {
    PROFILE.get().is_some_and(|p| p == root) || paired(root)
}

/// Rebuild from the approved, authenticated survivors before reclaiming SQLite pages.
/// May be repeated after a crash between pack replacement and derived cleanup.
pub(crate) fn finish_cleanup(
    root: &Path,
    brain: &str,
    anchor: &cryptex_events::LocalTrustAnchor,
) -> Result<(), ReviewError> {
    if !enabled(root) {
        return Err(ReviewError::Disabled);
    }
    {
        let snapshot =
            crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
                root, brain, anchor, None,
            )
            .map_err(|_| ReviewError::CleanupPending)?;
        snapshot.clean_purged_blobs()?;
        snapshot.clean_archived_versions()?;
    }
    let mut db = cryptex_projection::ProjectionDb::open_or_create(root)
        .map_err(|_| ReviewError::CleanupPending)?;
    db.connection()
        .execute_batch("PRAGMA secure_delete=ON")
        .map_err(|_| ReviewError::CleanupPending)?;
    crate::knowledge_materialization::rebuild_knowledge_from_local_approval_history(
        root, brain, anchor, &mut db,
    )
    .map_err(|_| ReviewError::CleanupPending)?;
    // The projection rebuild also reconstructs the external-content FTS table.
    let conn = db.connection();
    conn.execute_batch("VACUUM")
        .map_err(|_| ReviewError::CleanupPending)?;
    let busy: i64 = conn
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get(0))
        .map_err(|_| ReviewError::CleanupPending)?;
    if busy != 0 {
        return Err(ReviewError::CleanupPending);
    }
    drop(db);
    retained::clean_registered(root, brain, anchor)
}

/// Simulate the already-tested explicit Desktop prepare/confirm sequence for
/// a synthetic note while its peer's transport worker is stopped.
pub(crate) fn confirm_fixture(
    root: &Path,
    instance: &crate::instance::InstanceLock,
    store: std::sync::Arc<dyn crate::secure_store::SecureStore>,
    id: &str,
    hash: &str,
) {
    use crate::ipc::{SearchContext, test_desktop_process_identity};
    use std::sync::Arc;
    assert!(paired(root));
    let ctx = SearchContext {
        runtime_root: root.into(),
        admission: instance.admission(),
        operation: None,
        store,
        environment: crate::secure_store::SecureStoreEnvironment::Dev,
        expected_desktop: Arc::new(test_desktop_process_identity().unwrap()),
        projection_sync: None,
        connections: None,
        #[cfg(windows)]
        transport_status: None,
        #[cfg(target_os = "macos")]
        transport_identity: None,
        isolated_attachment_writer: false,
        purge_reviews: Some(Arc::default()),
        transport_device_id_override: None,
    };
    let super::Outcome::Prepared {
        ticket,
        remove_locally: true,
    } = super::decide(
        &ctx,
        super::Action::Prepare {
            note_id: id.into(),
            note_hash: hash.into(),
        },
    )
    .unwrap()
    else {
        panic!("expected local deletion review");
    };
    assert!(matches!(
        super::decide(&ctx, super::Action::Confirm { ticket }).unwrap(),
        super::Outcome::Purged { .. }
    ));
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Notice {
    bridge: String,
    approval: String,
}

/// Test pair only. The harness provides distinct send-only and receive-only
/// Syncthing folders outside the sealed segment exchange. All names derive
/// from verified history. Reopening retries any unacknowledged publication.
pub(crate) fn reconcile(
    identity: &str,
    root: &Path,
    brain: &str,
    anchor: &cryptex_events::LocalTrustAnchor,
    cleaned: &mut Option<(PathBuf, Vec<String>)>,
) -> Result<(), ReviewError> {
    use crate::import_staging::export_fs::Directory;
    use cryptex_events::AuthenticatedSecurityRecord;
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};
    if !paired(root) {
        return Err(ReviewError::Disabled);
    }
    let snapshot =
        crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
            root,
            brain,
            anchor,
            Some(Instant::now() + Duration::from_secs(5)),
        )
        .map_err(|_| ReviewError::Unavailable)?;
    let parent = Directory::open(root.parent().ok_or(ReviewError::Disabled)?)?;
    let outgoing = parent.child("out", false)?;
    let incoming = parent.child("in", false)?;
    let mut candidates = Vec::new();
    let peers = crate::managed_profile::exchange_policy::ExchangePeers::from_snapshot(
        &snapshot, root, identity,
    )
    .map_err(|_| ReviewError::Unauthorized)?;
    let [binding] = peers.bindings() else {
        return Err(ReviewError::Unauthorized);
    };
    {
        for record in snapshot.reader_device_chain(binding.device_id.as_str()) {
            let AuthenticatedSecurityRecord::V1(note) = record else {
                continue;
            };
            let name = format!("{}.purge", note.event_id);
            if incoming.exists(&name)? {
                let mut bytes = Vec::new();
                incoming
                    .open_file(&name, false)?
                    .take(8193)
                    .read_to_end(&mut bytes)
                    .map_err(|_| ReviewError::ReceiptStorage)?;
                if bytes.len() > 8192 {
                    return Err(ReviewError::ReceiptIntegrity);
                }
                let notice: Notice =
                    serde_json::from_slice(&bytes).map_err(|_| ReviewError::ReceiptIntegrity)?;
                candidates.push(notice);
            }
        }
    }
    for record in snapshot.reader_device_chain(anchor.device_id().as_str()) {
        let AuthenticatedSecurityRecord::PurgeBridge(bridge, ..) = record else {
            continue;
        };
        let profile = Directory::open(root)?;
        let ledger = profile
            .child("canonical", false)?
            .child(super::receipt::FOLDER, false)?;
        let mut approval = String::new();
        ledger
            .open_file(&format!("{}.json", bridge.event_id), false)?
            .take(1025)
            .read_to_string(&mut approval)
            .map_err(|_| ReviewError::ReceiptStorage)?;
        let notice = Notice {
            bridge: String::from_utf8(bridge.raw_record.clone())
                .map_err(|_| ReviewError::PackIntegrity)?,
            approval,
        };
        let bytes = serde_json::to_vec(&notice).map_err(|_| ReviewError::ReceiptIntegrity)?;
        let name = format!("{}.purge", bridge.event_id);
        if outgoing.exists(&name)? {
            let mut existing = Vec::new();
            outgoing
                .open_file(&name, false)?
                .take(8193)
                .read_to_end(&mut existing)
                .map_err(|_| ReviewError::ReceiptStorage)?;
            if bytes != existing {
                return Err(ReviewError::ReceiptIntegrity);
            }
        } else {
            let stage = profile.child("purge-staging", true)?;
            if stage.exists("segment.tmp")? {
                stage.discard_scratch(stage.open_file("segment.tmp", false)?)?;
            }
            let mut file = stage.open_file("segment.tmp", true)?;
            file.write_all(&bytes)
                .map_err(|_| ReviewError::ReceiptStorage)?;
            stage.promote("segment.tmp", &file, &outgoing, &name)?;
        }
    }
    drop(snapshot);
    if !candidates.is_empty() {
        *cleaned = None;
    }
    for notice in candidates {
        super::receipt::receive(
            root,
            brain,
            anchor,
            notice.bridge.as_bytes(),
            notice.approval.as_bytes(),
        )?;
    }
    // A restore can roll SQLite back independently of the signed packs and
    // any on-disk completion marker. Recheck every reopened transport session.
    let snapshot =
        crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
            root,
            brain,
            anchor,
            Some(Instant::now() + Duration::from_secs(5)),
        )
        .map_err(|_| ReviewError::CleanupPending)?;
    let remote = snapshot.clean_purged_exchange(anchor.device_id())?;
    snapshot.clean_archived_versions()?;
    drop(snapshot);
    if remote.is_empty() {
        *cleaned = None;
    } else if !cleaned
        .as_ref()
        .is_some_and(|(path, ids)| path == root && *ids == remote)
    {
        let staging = Directory::open(root)?.child("purge-staging", true)?;
        let _lock = staging.lock_exclusive("cleanup.lock")?;
        finish_cleanup(root, brain, anchor)?;
        *cleaned = Some((root.to_owned(), remote));
    }
    Ok(())
}
