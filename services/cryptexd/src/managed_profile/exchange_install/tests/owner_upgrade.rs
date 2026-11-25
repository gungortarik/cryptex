use super::*;
use crate::managed_profile::exchange_install::owner_upgrade::stage_exchange_upgrade;

const LEAVES: [Leaf; 3] = [Leaf::Config, Leaf::Binding, Leaf::Journal];

fn contents(home: &PrivateDirectory) -> [Option<Vec<u8>>; 3] {
    LEAVES.map(|leaf| home.read(leaf).unwrap())
}

fn seed_previous(mut home: &PrivateDirectory, policy: &ExchangeConfigPolicy<'_>, bootstrap: &[u8]) {
    seed(home, Leaf::Config.name(), bootstrap);
    install_native(home, policy).unwrap();
    let current = home.read(Leaf::Journal).unwrap().unwrap();
    let mut old = Journal::decode(&current).unwrap();
    let release_pin = r#""engine_version":"2.1.7""#;
    assert_eq!(old.binding.matches(release_pin).count(), 1);
    old.binding = old
        .binding
        .replace(release_pin, r#""engine_version":"2.1.5""#);
    home.replace(Leaf::Binding, old.binding.as_bytes()).unwrap();
    home.replace(Leaf::Journal, &old.encode().unwrap()).unwrap();
}

#[test]
fn detached_upgrade_preserves_paired_authority_and_cold_rollback_bytes() {
    with_policy(|policy, bootstrap| {
        let original = scratch();
        let staged = scratch();
        let retained = scratch();
        let before = {
            let home = PrivateDirectory::acquire(&original, true).unwrap();
            seed_previous(&home, policy, bootstrap);
            let before = contents(&home);
            assert!(verify_native(&home, policy).is_err());
            let candidate = PrivateDirectory::acquire(&staged, true).unwrap();
            // Model a stopped attempt after the first derived file was written.
            seed(
                &candidate,
                Leaf::Config.name(),
                before[0].as_deref().unwrap(),
            );
            stage_exchange_upgrade(&home, &candidate, bootstrap, policy).unwrap();
            let first = contents(&candidate);
            stage_exchange_upgrade(&home, &candidate, bootstrap, policy).unwrap();
            assert!(first == contents(&candidate));
            assert!(contents(&home) == before);
            assert_eq!(
                verify_native(&candidate, policy),
                Err(InstallError::HomeMismatch)
            );
            before
        };
        std::fs::rename(&original, &retained).unwrap();
        std::fs::rename(&staged, &original).unwrap();
        {
            let home = PrivateDirectory::acquire(&original, false).unwrap();
            verify_native(&home, policy).unwrap();
        }
        std::fs::rename(&original, &staged).unwrap();
        std::fs::rename(&retained, &original).unwrap();
        {
            let home = PrivateDirectory::acquire(&original, false).unwrap();
            assert!(contents(&home) == before);
        }
        for path in [original, staged] {
            std::fs::remove_dir_all(path).unwrap();
        }
    });
}

#[test]
fn detached_upgrade_refuses_changed_authority_or_unexplained_files_before_writing() {
    with_policies(|policy, changed, bootstrap| {
        let original = scratch();
        let staged = scratch();
        {
            let home = PrivateDirectory::acquire(&original, true).unwrap();
            seed_previous(&home, policy, bootstrap);
            let candidate = PrivateDirectory::acquire(&staged, true).unwrap();
            let before = contents(&home);
            assert!(stage_exchange_upgrade(&home, &candidate, bootstrap, changed).is_err());
            assert!(stage_exchange_upgrade(&home, &candidate, b"<wrong/>", policy).is_err());
            assert!(contents(&candidate).iter().all(Option::is_none));
            seed(&candidate, Leaf::Journal.name(), b"unrelated");
            let partial = contents(&candidate);
            assert!(stage_exchange_upgrade(&home, &candidate, bootstrap, policy).is_err());
            assert!(contents(&candidate) == partial);
            assert!(contents(&home) == before);
            let empty_path = scratch();
            {
                let empty = PrivateDirectory::acquire(&empty_path, true).unwrap();
                (&home).replace(Leaf::Binding, b"damaged").unwrap();
                assert!(stage_exchange_upgrade(&home, &empty, bootstrap, policy).is_err());
                assert!(contents(&empty).iter().all(Option::is_none));
            }
            std::fs::remove_dir_all(empty_path).unwrap();
        }
        for path in [original, staged] {
            std::fs::remove_dir_all(path).unwrap();
        }
    });
}
