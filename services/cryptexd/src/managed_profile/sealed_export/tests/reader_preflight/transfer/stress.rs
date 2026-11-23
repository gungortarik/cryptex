//! Each invocation is a fresh process. Only the native workers exchange packs.
use super::*;
use crate::canonical_writer::{AppendMemoryNoteArgs, CanonicalWriterConfig, append_memory_note};
use crate::incoming_publication::{publish_incoming_with_instance, rebuild_imported_state};
use crate::secure_store::{FileBackedSecureStore, SecureStore};
use cryptex_events::Classification;
use std::collections::BTreeMap;

const BURST: usize = 4;

fn identity(side: &Side) -> DeviceId {
    DeviceId::new(
        load_local_device_identity(&side.root, BRAIN)
            .unwrap()
            .device_id,
    )
    .unwrap()
}

fn fixture(author: usize, round: usize, index: usize) -> (String, String) {
    (
        format!("b831d41c-c7a3-44f0-8a5e-{author:04x}{round:04x}{index:04x}"),
        format!("Synthetic M16 peer {author} round {round} item {index}"),
    )
}

fn check_notes(side: &Side, store: &dyn SecureStore, rounds: [usize; 2]) {
    let hits = lifecycle::search(side, store);
    let mut expected = BTreeMap::new();
    for (author, limit) in rounds.into_iter().enumerate() {
        for round in 1..=limit {
            for index in 0..BURST {
                let (id, text) = fixture(author, round, index);
                expected.insert(id, text);
            }
        }
    }
    // Each enrolled peer also contributes one signed genesis note.
    assert_eq!(hits.len(), expected.len() + 2);
    for hit in hits {
        if let Some(text) = expected.remove(&hit.event_id) {
            assert_eq!(hit.text, text);
            assert_eq!(hit.classification, "public");
        }
    }
    assert!(expected.is_empty());
}

fn exchange(receiver: &mut Side, sender: &Side, store: &dyn SecureStore, round: usize) {
    let relative =
        PathBuf::from(identity(sender).as_filename_stem()).join(format!("{round:020}.seg"));
    let original = fs::read(sender.root.join("canonical/events").join(&relative)).unwrap();
    let arrived = receiver.root.join("transport/incoming").join(&relative);
    let deadline = Instant::now() + Duration::from_secs(90);
    while !arrived.try_exists().unwrap() {
        assert!(
            Instant::now() < deadline,
            "native transfer did not arrive in round {round}"
        );
        thread::sleep(Duration::from_millis(150));
    }
    assert_eq!(
        Sha256::digest(fs::read(arrived).unwrap()),
        Sha256::digest(&original)
    );
    let mut admit = || {
        publish_incoming_with_instance(
            &receiver.root,
            std::slice::from_ref(&relative),
            &mut receiver.instance,
            store,
            ENV,
        )
        .unwrap()
    };
    let first = admit();
    assert_eq!(first.published_segments, 1);
    assert_eq!(
        first.published_events,
        if round == 0 { 2 } else { BURST as u64 }
    );
    let replay = admit();
    assert_eq!(
        (
            replay.published_events,
            replay.published_segments,
            replay.identical_segments
        ),
        (0, 0, 1)
    );
    assert_eq!(
        Sha256::digest(fs::read(receiver.root.join("canonical/events").join(relative)).unwrap()),
        Sha256::digest(original)
    );
}

pub(in crate::managed_profile) fn run() {
    let base = PathBuf::from(std::env::var_os("CRYPTEX_TEST_WIRE").unwrap());
    let ids =
        ["CRYPTEX_TEST_DEVICE_A", "CRYPTEX_TEST_DEVICE_B"].map(|name| std::env::var(name).unwrap());
    let roots = [base.join("A/profile"), base.join("B/profile")];
    let store = FileBackedSecureStore::open(base.join("fixture-credentials")).unwrap();
    let round: usize = std::env::var("CRYPTEX_M16_ROUND").unwrap().parse().unwrap();
    let phase = std::env::var("CRYPTEX_M16_PHASE").unwrap();
    if phase == "enroll" {
        assert_eq!(round, 0);
        for root in &roots {
            fs::create_dir(root).unwrap();
        }
        crate::trusted_import_probe::tests::enroll_transport_pair(
            &roots[0], &roots[1], &ids[0], &ids[1], &store,
        );
    }
    let [mut a, mut b] = std::array::from_fn(|i| Side::new(roots[i].clone(), &ids[i]));
    match phase.as_str() {
        "enroll" => {
            for side in [&mut a, &mut b] {
                assert_eq!(side.tick(&store).copied, 1);
            }
        }
        "offline" => {
            assert!(round > 0);
            for (author, side) in [&mut a, &mut b].into_iter().enumerate() {
                check_notes(side, &store, [round - 1; 2]);
                let device = identity(side);
                for index in 0..BURST {
                    let (id, text) = fixture(author, round, index);
                    let outcome = append_memory_note(
                        &store,
                        ENV,
                        AppendMemoryNoteArgs {
                            runtime_root: &side.root,
                            brain_id: BRAIN,
                            device_id: &device,
                            classification: Classification::Public,
                            text: &text,
                            event_id: Some(&id),
                            config: &CanonicalWriterConfig::default(),
                        },
                    )
                    .unwrap();
                    assert_eq!(outcome.segment_index, round as u64);
                }
                assert_eq!(side.tick(&store).copied, 1);
                rebuild_imported_state(&side.root, BRAIN, &store, ENV).unwrap();
                let mut expected = [round - 1; 2];
                expected[author] = round;
                check_notes(side, &store, expected);
            }
        }
        "online" => {
            exchange(&mut a, &b, &store, round);
            exchange(&mut b, &a, &store, round);
            for side in [&a, &b] {
                check_notes(side, &store, [round; 2]);
            }
            assert_eq!(lifecycle::search(&a, &store), lifecycle::search(&b, &store));
        }
        "reopen" => {
            for side in [&a, &b] {
                check_notes(side, &store, [round; 2]);
            }
            assert_eq!(lifecycle::search(&a, &store), lifecycle::search(&b, &store));
        }
        _ => panic!("unknown sync stress phase"),
    }
    println!("m16_exchange phase={phase} round={round} verified=true");
}
