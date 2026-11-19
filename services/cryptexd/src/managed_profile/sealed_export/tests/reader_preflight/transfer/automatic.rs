//! The two native workers share the actual controller exchange trees. The
//! parent coordinates only worker lifetime and never handles attachment bytes.
use super::*;

fn wait_for(mut condition: impl FnMut() -> bool, stage: &str) {
    let deadline = Instant::now() + Duration::from_secs(100);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "native attachment timeout: {stage}"
        );
        thread::sleep(Duration::from_millis(400));
    }
}

pub(super) fn exercise(
    wire: &Path,
    ids: [&str; 2],
    store: &crate::FakeSecureStore,
    relative: &Path,
    note: &str,
    link: &str,
    bytes: &[u8],
) {
    let [a_id, b_id] = ids;
    let left = wire.join("A/profile");
    let right = wire.join("B/profile");
    let digest = format!("{:x}", Sha256::digest(bytes));
    let local = right.join("canonical/blobs").join(&digest);
    let incoming = right
        .join("transport/incoming")
        .join(crate::import_staging::ATTACHMENT_FOLDER);
    let remote = incoming.join(a_id).join(&digest);
    assert!(!remote.exists());
    let pack = right.join("canonical/events").join(relative);
    let original = fs::read(&pack).unwrap();
    fs::remove_file(&local).unwrap();
    let mut a = Side::new(left, a_id);
    let mut b = Side::new(right.clone(), b_id);
    a.tick(store);
    b.tick(store);
    assert!(!local.exists());
    fs::write(wire.join("connect"), []).unwrap();
    wait_for(
        || {
            a.tick(store);
            b.tick(store);
            local.is_file()
        },
        "automatic missing delivery",
    );
    guarded(&b, store)
        .verify_local_attachment(note, link)
        .unwrap();
    assert_eq!(fs::read(&remote).unwrap(), bytes);
    assert_eq!(fs::read(&local).unwrap(), bytes);

    fs::write(wire.join("disconnect"), []).unwrap();
    wait_for(|| wire.join("offline").is_file(), "workers stopped");
    drop(b);
    let mut b = Side::new(right.clone(), b_id);
    let damaged = b"interrupted receiver";
    fs::write(&local, damaged).unwrap();
    b.tick(store);
    assert!(!matches!(
        b.exporter.last_reader_status,
        Some(reader::Status::Verified(_))
    ));
    assert_eq!(fs::read(&local).unwrap(), damaged);
    let quarantine = right.join("blob-staging/quarantine");
    let before = fs::read_dir(&quarantine).unwrap().count();

    crate::attachment_blobs::INTERRUPT_AFTER_QUARANTINE.with(|flag| flag.set(true));
    fs::write(wire.join("reconnect"), []).unwrap();
    wait_for(
        || {
            a.tick(store);
            match b.exporter.tick(
                &b.root,
                &b.instance.admission(),
                &b.guard,
                b_id,
                (store, ENV),
                &AtomicBool::new(false),
            ) {
                Ok(_) => false,
                Err(Error::Io(std::io::ErrorKind::Interrupted, None)) => true,
                Err(err) => panic!("unexpected repair error: {err:?}"),
            }
        },
        "interrupted promotion",
    );
    assert!(!local.exists());
    assert_eq!(fs::read_dir(&quarantine).unwrap().count(), before + 1);

    drop(b);
    let mut b = Side::new(right.clone(), b_id);
    wait_for(
        || {
            a.tick(store);
            b.tick(store);
            local.is_file()
        },
        "reopened recovery",
    );
    b.tick(store);
    drop(b);
    let b = Side::new(right.clone(), b_id);
    guarded(&b, store)
        .verify_local_attachment(note, link)
        .unwrap();
    assert_eq!(fs::read(&local).unwrap(), bytes);
    assert_eq!(
        fs::read_dir(right.join("canonical/blobs")).unwrap().count(),
        1
    );
    assert_eq!(fs::read_dir(&quarantine).unwrap().count(), before + 1);
    assert!(
        fs::read_dir(quarantine)
            .unwrap()
            .any(|entry| fs::read(entry.unwrap().path()).unwrap() == damaged)
    );
    assert_eq!(fs::read(pack).unwrap(), original);
    println!(
        "automatic_attachment_sync=verified missing=restored offline=held reconnect=fresh interrupted_repair=recovered canonical_blobs=1 pack=unchanged"
    );
}
