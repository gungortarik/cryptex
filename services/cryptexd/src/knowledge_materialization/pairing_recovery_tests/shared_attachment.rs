use super::*;
use crate::attachment_blobs::LocalLinkError;
use crate::ipc::attachment::SelectedFile;
use cryptex_events::attachment_link::LinkError;
use std::io::Read;

#[test]
fn cutoff_preserves_the_other_notes_shared_attachment() {
    let profiles = PairProfiles::new();
    profiles.pair(0, [true, true], false);
    profiles.import_peer_fixture(0);
    let root = &profiles.profiles[0];
    drop(crate::instance::try_acquire_instance(root).unwrap());
    crate::instance::migration::migrate_profile(root).unwrap();
    let instance = crate::instance::try_acquire_instance(root).unwrap();
    let bytes = b"shared attachment across a device cutoff";
    let policy = profiles.policy();
    let select_note = |side: usize, seq| {
        policy
            .history
            .records()
            .iter()
            .find_map(|record| match record {
                AuthenticatedSecurityRecord::V1(note)
                    if record.device_id() == profiles.devices[side].as_str()
                        && record.device_seq() == seq =>
                {
                    Some(SelectedFile {
                        note_id: note.event_id.clone(),
                        note_hash: note.content_hash.as_str().to_owned(),
                        bytes: bytes.to_vec(),
                    })
                }
                _ => None,
            })
            .unwrap()
    };
    let retained = select_note(0, 1);
    let excluded = select_note(1, 3);
    drop(policy);
    for file in [&retained, &excluded] {
        crate::canonical_writer::isolated_attachment::append(
            root,
            &profiles.store,
            SecureStoreEnvironment::Dev,
            file,
        )
        .unwrap();
    }
    let (mut db, _) = crate::projection::open_projection_db(root).unwrap();
    refresh(&profiles, &mut db).unwrap();
    let vault = profiles.root.join("vault");
    fs::create_dir(&vault).unwrap();
    let project = |db: &ProjectionDb| {
        rebuild_vault_with_retrieval_safety(root, BRAIN, &[], &mut resolver(&profiles), db, &vault)
            .unwrap();
    };
    let policy = profiles.policy();
    let link_for = |file: &SelectedFile| {
        let links: Vec<_> = policy
            .history
            .records()
            .iter()
            .filter_map(|record| match record {
                AuthenticatedSecurityRecord::Attachment(link, fields)
                    if fields.note_event_id() == file.note_id =>
                {
                    Some(link.event_id.clone())
                }
                _ => None,
            })
            .collect();
        assert_eq!(links.len(), 1);
        links.into_iter().next().unwrap()
    };
    let retained_link = link_for(&retained);
    let excluded_link = link_for(&excluded);
    assert_ne!(retained_link, excluded_link);
    drop(policy);
    let copies = vault.join("Cryptex/attachments");
    let retained_copy = copies.join(&retained_link).join("attachment.bin");
    let excluded_copy = copies.join(&excluded_link).join("attachment.bin");
    project(&db);
    assert_eq!(fs::read(&retained_copy).unwrap(), bytes);
    assert_eq!(fs::read(&excluded_copy).unwrap(), bytes);

    restrict_peer_after_first_record(&profiles);
    drop((instance, db));
    let _reopened = crate::instance::try_acquire_instance(root).unwrap();
    let (mut db, _) = crate::projection::open_projection_db(root).unwrap();
    refresh(&profiles, &mut db).unwrap();
    let state = evaluate_retrieval_safety(root, BRAIN, &[], &mut resolver(&profiles), &db);
    assert!(matches!(
        state.open_local_attachment(&excluded.note_id, &excluded_link),
        Err(LocalLinkError::Reference(LinkError::MissingLink))
    ));
    let mut content = Vec::new();
    state
        .open_local_attachment(&retained.note_id, &retained_link)
        .unwrap()
        .read_to_end(&mut content)
        .unwrap();
    assert_eq!(content, bytes);
    drop(state);
    let blobs = root.join("canonical/blobs");
    assert_eq!(fs::read_dir(&blobs).unwrap().count(), 1);
    assert_eq!(
        fs::read(blobs.join(format!("{:x}", Sha256::digest(bytes)))).unwrap(),
        bytes
    );
    project(&db);
    assert!(!excluded_copy.try_exists().unwrap());
    assert_eq!(fs::read_dir(&copies).unwrap().count(), 1);
    assert_eq!(fs::read(retained_copy).unwrap(), bytes);
    println!(
        "shared_attachment_cutoff=verified canonical_files=1 remaining_links=1 vault_copies=1 reopened=true"
    );
}
