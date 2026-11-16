//! Explicit local-only enrollment, separate from normal application opening.
//! Existing/partial installations refuse; no peer approval or canonical writes.
use crate::import_staging::export_fs::Directory;
use crate::secure_store::SecureStoreEnvironment;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Arguments,
    Runtime,
    Busy,
    Identity,
    AlreadyExists,
    Storage,
    Package,
    Native,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "transport_enrollment_{self:?}")
    }
}
impl std::error::Error for Error {}

/// Caller explicitly selects a previously initialized development runtime.
/// This is not called by Desktop, owner IPC, or normal daemon startup.
pub fn enroll_local_dev(root: &Path, package: Option<&Path>) -> Result<(), Error> {
    #[cfg(windows)]
    if !package.is_some_and(Path::is_absolute) {
        return Err(Error::Arguments);
    }
    #[cfg(target_os = "macos")]
    if package.is_some() {
        return Err(Error::Arguments);
    }
    crate::paths::assert_outside_repository(root).map_err(|_| Error::Arguments)?;
    if !root.is_absolute() {
        return Err(Error::Arguments);
    }
    let namespace = Directory::open(root).map_err(|_| Error::Runtime)?;
    let _instance = crate::instance::try_acquire_instance(root).map_err(|_| Error::Busy)?;
    if namespace
        .exists("managed-transport")
        .map_err(|_| Error::Storage)?
    {
        return Err(Error::AlreadyExists);
    }
    #[cfg(windows)]
    let store = crate::WindowsCredentialStore::new();
    #[cfg(target_os = "macos")]
    let store = crate::MacosKeychainStore::new();
    let brain = crate::identity_runtime::load_brain_identity(root).map_err(|_| Error::Identity)?;
    crate::local_device_identity::LocalDeviceKeyResolver::load(
        root,
        &brain.brain_id,
        &store,
        SecureStoreEnvironment::Dev,
    )
    .map_err(|_| Error::Identity)?;
    namespace.validate().map_err(|_| Error::Runtime)?;
    #[cfg(windows)]
    {
        use super::{ManagedProfile, native::NativePorts};
        use crate::syncthing_package::PinnedSyncthingPackage;
        let source = PinnedSyncthingPackage::open(package.ok_or(Error::Arguments)?)
            .map_err(|_| Error::Package)?;
        let a = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .map_err(|_| Error::Native)?;
        let b = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .map_err(|_| Error::Native)?;
        let ports = NativePorts::new(
            a.local_addr().map_err(|_| Error::Native)?.port(),
            b.local_addr().map_err(|_| Error::Native)?.port(),
        )
        .map_err(|_| Error::Native)?;
        let helper = namespace
            .create_child_exclusive("managed-transport")
            .map_err(|_| Error::Storage)?;
        let destination = helper
            .create_child_exclusive("package")
            .map_err(|_| Error::Storage)?;
        source
            .install_fresh(&destination)
            .map_err(|_| Error::Package)?;
        let package = PinnedSyncthingPackage::open(&root.join("managed-transport/package"))
            .map_err(|_| Error::Package)?;
        let mut profile = ManagedProfile::create(
            &root.join("managed-transport/00000000000000000000000000000001"),
            SecureStoreEnvironment::Dev,
        )
        .map_err(|_| Error::Storage)?;
        drop((a, b)); // A later collision refuses; no process is ever adopted.
        profile
            .prepare_native(&package, &store, ports)
            .map_err(|_| Error::Native)?;
        destination.sync().map_err(|_| Error::Storage)?;
        helper.sync().map_err(|_| Error::Storage)?;
    }
    #[cfg(target_os = "macos")]
    {
        crate::m8_owner_channel::transport_helper::enroll_product(&namespace, root)
            .map_err(|_| Error::Native)?;
    }
    namespace.sync().map_err(|_| Error::Runtime)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn package(root: &Path) -> Option<&Path> {
        if cfg!(windows) { Some(root) } else { None }
    }
    #[test]
    fn transport_enrollment_missing_runtime_is_not_created() {
        let root = crate::test_temp::exclusive_temp_dir("cryptex-enrollment");
        let missing = root.join("missing");
        assert_eq!(
            enroll_local_dev(&missing, package(&root)),
            Err(Error::Runtime)
        );
        assert!(!missing.exists());
        std::fs::remove_dir(root).unwrap();
    }
    #[test]
    fn transport_enrollment_refuses_partial_and_absent_identity() {
        let root = crate::test_temp::exclusive_temp_dir("cryptex-enrollment");
        assert_eq!(
            enroll_local_dev(&root, package(&root)),
            Err(Error::Identity)
        );
        assert!(!root.join("managed-transport").exists());
        std::fs::create_dir(root.join("managed-transport")).unwrap();
        assert_eq!(
            enroll_local_dev(&root, package(&root)),
            Err(Error::AlreadyExists)
        );
        assert_eq!(
            std::fs::read_dir(root.join("managed-transport"))
                .unwrap()
                .count(),
            0
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn transport_enrollment_exclusive_directory_refuses_reuse() {
        let root = crate::test_temp::exclusive_temp_dir("cryptex-enrollment");
        let directory = Directory::open(&root).unwrap();
        let child = directory.create_child_exclusive("fresh").unwrap();
        assert!(directory.create_child_exclusive("fresh").is_err());
        assert!(directory.create_child_exclusive("../outside").is_err());
        child.validate().unwrap();
        drop((child, directory));
        std::fs::remove_dir_all(root).unwrap();
    }
}
