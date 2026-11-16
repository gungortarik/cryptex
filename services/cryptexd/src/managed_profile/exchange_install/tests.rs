use super::*;
use crate::managed_profile::exchange_policy::ExchangePeers;
use crate::managed_profile::native::NativePorts;
use crate::managed_profile::native_xml;
use cryptex_events::TrustedDaemonKeyResolver;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

const LOCAL: &str = "U2SYIGU-YWZX757-2WPZGCR-DHD7MLH-ZKV7HLJ-EYO6CXE-K3DU3DU-SDFXYAE";
const REMOTE: &str = "XAAPCAD-GA4SF4J-BNQL3FS-T456WKV-FGLXUKO-CDWHVDW-EYWNRIG-SM5GUAZ";

fn with_policy(f: impl FnOnce(&ExchangeConfigPolicy<'_>, &[u8])) {
    let (source, receiver, negative, store) =
        crate::trusted_import_probe::tests::pair_with_transport(LOCAL, REMOTE);
    {
        let brain = crate::identity_runtime::load_brain_identity(&source).unwrap();
        let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            &source,
            &brain.brain_id,
            &store,
            crate::SecureStoreEnvironment::Dev,
        )
        .unwrap();
        let snapshot =
            crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
                &source,
                &brain.brain_id,
                &resolver.local_trust_anchor().unwrap(),
                None,
            )
            .unwrap();
        let authority = ExchangePeers::from_snapshot(&snapshot, &source, LOCAL).unwrap();
        let ports = NativePorts::new(18384, 22001).unwrap();
        let policy = ExchangeConfigPolicy::new(
            &authority,
            ports,
            "192.168.77.1:22001".parse().unwrap(),
            BTreeMap::from([(REMOTE.into(), "192.168.77.2:22002".parse().unwrap())]),
        )
        .unwrap();
        let bootstrap = native_xml::inspect(
            include_bytes!("../native_xml/exchange/bootstrap-test.xml"),
            ports,
            true,
        )
        .unwrap();
        f(&policy, &bootstrap.bytes);
    }
    for root in [source, receiver, negative] {
        std::fs::remove_dir_all(root).unwrap();
    }
}

fn scratch() -> PathBuf {
    let mut nonce = [0; 16];
    getrandom::getrandom(&mut nonce).unwrap();
    std::env::temp_dir().canonicalize().unwrap().join(format!(
        "cx-exchange-install-{}",
        nonce.iter().map(|b| format!("{b:02x}")).collect::<String>()
    ))
}
fn seed(home: &PrivateDirectory, name: &str, bytes: &[u8]) {
    let mut file = home.create_file(name).unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}

struct Interrupt<F> {
    files: F,
    count: usize,
    at: usize,
    after: bool,
}
impl<F: Files> Files for Interrupt<F> {
    fn home(&self) -> Result<String> {
        self.files.home()
    }
    fn read(&self, leaf: Leaf) -> Result<Option<Vec<u8>>> {
        self.files.read(leaf)
    }
    fn stage(&self, leaf: Leaf) -> Result<Option<Vec<u8>>> {
        self.files.stage(leaf)
    }
    fn sync_committed(&mut self) -> Result<()> {
        self.files.sync_committed()
    }
    fn replace(&mut self, leaf: Leaf, bytes: &[u8]) -> Result<()> {
        self.count += 1;
        if self.count == self.at && !self.after {
            return Err(ProfileError::Io.into());
        }
        self.files.replace(leaf, bytes)?;
        if self.count == self.at && self.after {
            return Err(ProfileError::Io.into());
        }
        Ok(())
    }
}

#[test]
fn native_files_recover_before_and_after_every_commit_boundary() {
    with_policy(|policy, bootstrap| {
        for at in 1..=4 {
            for after in [false, true] {
                let root = scratch();
                {
                    let home = PrivateDirectory::acquire(&root, true).unwrap();
                    seed(&home, Leaf::Config.name(), bootstrap);
                    let mut interrupted = Interrupt {
                        files: &home,
                        count: 0,
                        at,
                        after,
                    };
                    assert!(install(&mut interrupted, policy).is_err());
                    if at != 1 || after {
                        assert!(refuse_exchange_state(&home).is_err());
                    }
                }
                {
                    // Release the lease and reopen ORIGINAL on-disk state.
                    let home = PrivateDirectory::acquire(&root, false).unwrap();
                    install(&mut &home, policy).unwrap();
                    let receipt = (&home).read(Leaf::Journal).unwrap().unwrap();
                    assert!(Journal::decode(&receipt).unwrap().phase == Phase::Committed);
                    let mut no_write = Interrupt {
                        files: &home,
                        count: 0,
                        at: 1,
                        after: false,
                    };
                    install(&mut no_write, policy).unwrap();
                    assert_eq!(no_write.count, 0);
                    assert!(refuse_exchange_state(&home).is_err());
                    for leaf in [Leaf::Config, Leaf::Binding, Leaf::Journal] {
                        assert!(home.open_read(leaf.temporary()).unwrap().is_none());
                    }
                }
                std::fs::remove_dir_all(root).unwrap();
            }
        }
    });
}

#[derive(Clone)]
struct MemoryFiles {
    data: [Option<Vec<u8>>; 3],
    stages: [Option<Vec<u8>>; 3],
    writes: usize,
    syncs: usize,
    fail_sync: bool,
}
impl Files for MemoryFiles {
    fn home(&self) -> Result<String> {
        Ok("fixed-native-home".into())
    }
    fn read(&self, leaf: Leaf) -> Result<Option<Vec<u8>>> {
        Ok(self.data[leaf as usize].clone())
    }
    fn stage(&self, leaf: Leaf) -> Result<Option<Vec<u8>>> {
        Ok(self.stages[leaf as usize].clone())
    }
    fn sync_committed(&mut self) -> Result<()> {
        self.syncs += 1;
        if self.fail_sync {
            return Err(ProfileError::Io.into());
        }
        Ok(())
    }
    fn replace(&mut self, leaf: Leaf, bytes: &[u8]) -> Result<()> {
        self.writes += 1;
        self.data[leaf as usize] = Some(bytes.to_vec());
        self.stages[leaf as usize] = None;
        Ok(())
    }
}
fn completed(policy: &ExchangeConfigPolicy<'_>, bootstrap: &[u8]) -> MemoryFiles {
    let mut files = MemoryFiles {
        data: [Some(bootstrap.to_vec()), None, None],
        stages: [None, None, None],
        syncs: 0,
        fail_sync: false,
        writes: 0,
    };
    install(&mut files, policy).unwrap();
    files.writes = 0;
    files
}

#[test]
fn only_reachable_config_binding_pairs_recover_and_committed_never_rolls_back() {
    with_policy(|policy, bootstrap| {
        let base = completed(policy, bootstrap);
        for phase in [Phase::Pending, Phase::Committed] {
            for config in 0..4 {
                for binding in 0..3 {
                    let mut files = base.clone();
                    let mut journal = Journal::decode(files.data[2].as_ref().unwrap()).unwrap();
                    journal.phase = phase;
                    files.data[2] = Some(journal.encode().unwrap());
                    files.data[0] = match config {
                        0 => Some(bootstrap.to_vec()),
                        1 => base.data[0].clone(),
                        2 => None,
                        _ => Some(b"foreign".to_vec()),
                    };
                    files.data[1] = match binding {
                        0 => None,
                        1 => base.data[1].clone(),
                        _ => Some(b"foreign".to_vec()),
                    };
                    let allowed = config == 1 && binding == 1
                        || phase == Phase::Pending && config <= 1 && binding == 0;
                    assert_eq!(
                        install(&mut files, policy).is_ok(),
                        allowed,
                        "config={config} binding={binding}"
                    );
                    if !allowed {
                        assert_eq!(files.writes, 0);
                    }
                }
            }
        }
    });
}

#[test]
fn journal_is_strict_bounded_profile_bound_and_recompiled_before_writes() {
    with_policy(|policy, bootstrap| {
        let base = completed(policy, bootstrap);
        for field in [
            "schema",
            "native_home",
            "bootstrap",
            "config",
            "binding",
            "unknown",
        ] {
            let mut files = base.clone();
            let mut journal: serde_json::Value =
                serde_json::from_slice(files.data[2].as_ref().unwrap()).unwrap();
            journal[field] = if field == "schema" {
                2.into()
            } else {
                "changed".into()
            };
            files.data[2] = Some(serde_json::to_vec(&journal).unwrap());
            assert!(install(&mut files, policy).is_err(), "{field}");
            assert_eq!(files.writes, 0);
        }
        let duplicate = String::from_utf8(base.data[2].clone().unwrap())
            .unwrap()
            .replacen("{", "{\"schema\":1,", 1);
        assert!(Journal::decode(duplicate.as_bytes()).is_err());
        assert!(Journal::decode(&vec![b' '; MAX_JOURNAL + 1]).is_err());
        let mut files = MemoryFiles {
            data: [Some(bootstrap.to_vec()), Some(Vec::new()), None],
            stages: [None, None, None],
            syncs: 0,
            fail_sync: false,
            writes: 0,
        };
        assert!(install(&mut files, policy).is_err());
        assert_eq!(files.writes, 0);
    });
}

#[test]
fn stale_policy_from_other_signed_brain_refuses_committed_journal_without_writing() {
    with_policy(|policy, bootstrap| {
        let base = completed(policy, bootstrap);
        with_policy(|other, _| {
            let mut files = base.clone();
            assert!(install(&mut files, other).is_err());
            assert_eq!(files.writes, 0);
        });
    });
}

#[test]
fn native_replacement_refuses_partial_staging_and_linked_targets_resumes_exact_stage() {
    for leaf in [Leaf::Config, Leaf::Binding, Leaf::Journal] {
        let root = scratch();
        {
            let home = PrivateDirectory::acquire(&root, true).unwrap();
            seed(&home, leaf.name(), b"old");
            seed(&home, leaf.temporary(), b"partial");
            assert!(home.replace_exchange_file(leaf, b"new").is_err());
            assert_eq!((&home).read(leaf).unwrap().unwrap(), b"old");
            assert!(refuse_exchange_state(&home).is_err());
        }
        std::fs::remove_dir_all(root).unwrap();
        let root = scratch();
        {
            let home = PrivateDirectory::acquire(&root, true).unwrap();
            seed(&home, leaf.name(), b"old");
            seed(&home, leaf.temporary(), b"new");
            home.replace_exchange_file(leaf, b"new").unwrap();
            assert_eq!((&home).read(leaf).unwrap().unwrap(), b"new");
            std::fs::hard_link(root.join(leaf.name()), root.join("alias")).unwrap();
            assert!(home.replace_exchange_file(leaf, b"other").is_err());
            assert_eq!(std::fs::read(root.join("alias")).unwrap(), b"new");
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn bootstrap_refuses_every_exchange_marker_even_empty_before_config_change() {
    for leaf in [Leaf::Config, Leaf::Binding, Leaf::Journal] {
        for temporary in [false, true] {
            if leaf == Leaf::Config && !temporary {
                continue;
            }
            let root = scratch();
            {
                let home = PrivateDirectory::acquire(&root, true).unwrap();
                seed(
                    &home,
                    if temporary {
                        leaf.temporary()
                    } else {
                        leaf.name()
                    },
                    b"",
                );
                assert!(refuse_exchange_state(&home).is_err());
            }
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn complete_stage_resumes_only_at_its_reachable_boundary() {
    with_policy(|policy, bootstrap| {
        let base = completed(policy, bootstrap);
        for step in 1..=4 {
            let mut files = MemoryFiles {
                data: [Some(bootstrap.to_vec()), None, None],
                stages: [None, None, None],
                writes: 0,
                syncs: 0,
                fail_sync: false,
            };
            {
                let mut interrupted = Interrupt {
                    files,
                    count: 0,
                    at: step,
                    after: false,
                };
                assert!(install(&mut interrupted, policy).is_err());
                files = interrupted.files;
            }
            let (leaf, bytes) = match step {
                1 => {
                    let mut j = Journal::decode(base.data[2].as_ref().unwrap()).unwrap();
                    j.phase = Phase::Pending;
                    (Leaf::Journal, j.encode().unwrap())
                }
                2 => (Leaf::Config, base.data[0].clone().unwrap()),
                3 => (Leaf::Binding, base.data[1].clone().unwrap()),
                _ => (Leaf::Journal, base.data[2].clone().unwrap()),
            };
            files.stages[leaf as usize] = Some(bytes.clone());
            let mut good = files.clone();
            install(&mut good, policy).unwrap();
            assert!(good.stages.iter().all(Option::is_none));
            files.stages[leaf as usize] = Some(bytes[..bytes.len() / 2].to_vec());
            files.writes = 0;
            assert!(install(&mut files, policy).is_err());
            assert_eq!(files.writes, 0);
        }
        for leaf in [Leaf::Config, Leaf::Binding, Leaf::Journal] {
            let mut files = base.clone();
            files.stages[leaf as usize] = files.data[leaf as usize].clone();
            assert!(install(&mut files, policy).is_err());
            assert_eq!(files.writes, 0);
        }
    });
}

#[test]
fn visible_commit_cannot_hide_an_unresolved_flush_failure() {
    with_policy(|policy, bootstrap| {
        let mut files = completed(policy, bootstrap);
        files.fail_sync = true;
        let previous_syncs = files.syncs;
        for _ in 0..2 {
            assert!(install(&mut files, policy).is_err());
            assert_eq!(files.writes, 0);
        }
        assert_eq!(files.syncs, previous_syncs + 2);
        files.fail_sync = false;
        install(&mut files, policy).unwrap();
        assert_eq!(files.syncs, previous_syncs + 3);
        assert_eq!(files.writes, 0);
    });
}
