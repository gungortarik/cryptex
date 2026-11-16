//! Closed, fresh-authority install/start command. Never loaded from disk as trust.
use super::exchange_install::InstallPolicy;
use super::native::{NativeConfigError, NativePorts};
use super::native_xml::{
    self,
    exchange::{ExchangeConfigError, ExchangeConfigPolicy},
};
use crate::runtime_admission::{ExclusivePermit, TransportGuard};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub(crate) const MAX_COMMAND: usize = 1024 * 1024;

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeIdentity {
    pub(super) credential_id: String,
    pub(super) native_id: String,
    pub(super) cert_hash: String,
    pub(super) key_hash: String,
}
#[cfg(target_os = "macos")]
impl NativeIdentity {
    pub(crate) fn native_id(&self) -> &str {
        &self.native_id
    }
    pub(crate) fn credential_id(&self) -> &str {
        &self.credential_id
    }
}
#[cfg(target_os = "macos")]
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExchangeSeed {
    pub(crate) identity: NativeIdentity,
    pub(crate) bootstrap: String,
}
#[cfg(target_os = "macos")]
impl ExchangeSeed {
    pub(crate) fn encode(&self) -> Result<Vec<u8>, NativeConfigError> {
        let bytes = serde_json::to_vec(self).map_err(|_| NativeConfigError::InvalidConfig)?;
        if bytes.len() > MAX_COMMAND {
            return Err(NativeConfigError::InvalidConfig);
        }
        Ok(bytes)
    }
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, NativeConfigError> {
        if bytes.len() > MAX_COMMAND {
            return Err(NativeConfigError::InvalidConfig);
        }
        cryptex_events::reject_duplicate_json_keys(
            std::str::from_utf8(bytes).map_err(|_| NativeConfigError::InvalidConfig)?,
        )
        .map_err(|_| NativeConfigError::InvalidConfig)?;
        let seed: Self =
            serde_json::from_slice(bytes).map_err(|_| NativeConfigError::InvalidConfig)?;
        if seed.bootstrap.len() > 256 * 1024
            || seed.identity.native_id.len() != 63
            || seed.identity.credential_id.len() != 32
            || seed.identity.cert_hash.len() != 64
            || seed.identity.key_hash.len() != 64
        {
            return Err(NativeConfigError::InvalidConfig);
        }
        Ok(seed)
    }
}
// No Debug on configuration/credential-bearing material.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    schema: u32,
    nonce: [u8; 16],
    root: PathBuf,
    ports: NativePorts,
    identity: NativeIdentity,
    bootstrap: String,
    config: String,
    binding: String,
}

pub(crate) struct ExchangeImage {
    wire: Wire,
    pins: IncomingPins,
}
struct IncomingPins {
    #[cfg(windows)]
    _pins: Vec<std::fs::File>,
    #[cfg(target_os = "macos")]
    pins: crate::import_staging::macos_fs::Directory,
}
impl IncomingPins {
    fn open(root: &Path) -> Result<Self, NativeConfigError> {
        let incoming = root.join("transport/incoming");
        // Existing only: no creation, canonical alias, symlink or junction.
        #[cfg(windows)]
        {
            Ok(Self {
                _pins: crate::import_staging::ImportStagingSession::pin_existing_runtime(&incoming)
                    .map_err(|_| NativeConfigError::InvalidConfig)?,
            })
        }
        #[cfg(target_os = "macos")]
        {
            Ok(Self {
                pins: crate::import_staging::macos_fs::Directory::open(&incoming, true)
                    .map_err(|_| NativeConfigError::InvalidConfig)?,
            })
        }
    }
    fn validate(&self) -> Result<(), NativeConfigError> {
        #[cfg(target_os = "macos")]
        self.pins
            .validate()
            .map_err(|_| NativeConfigError::InvalidConfig)?;
        Ok(())
    }
}
impl ExchangeImage {
    pub(crate) fn from_policy(
        policy: &ExchangeConfigPolicy<'_>,
        permit: &ExclusivePermit,
        identity: NativeIdentity,
        bootstrap: &[u8],
    ) -> Result<(Self, TransportGuard), NativeConfigError> {
        let guard = permit
            .transport_guard(policy.runtime_root())
            .map_err(|_| NativeConfigError::InvalidState)?;
        guard.check().map_err(|_| NativeConfigError::InvalidState)?;
        let config = policy
            .prepare_bootstrap(bootstrap)
            .map_err(|_| NativeConfigError::InvalidConfig)?;
        let binding = policy
            .binding_bytes(&config)
            .map_err(|_| NativeConfigError::InvalidConfig)?;
        let mut nonce = [0; 16];
        getrandom::getrandom(&mut nonce).map_err(|_| NativeConfigError::InvalidState)?;
        let wire = Wire {
            schema: 1,
            nonce,
            root: policy.runtime_root().to_owned(),
            ports: policy.ports(),
            identity,
            bootstrap: String::from_utf8(bootstrap.to_vec())
                .map_err(|_| NativeConfigError::InvalidConfig)?,
            config: String::from_utf8(config).map_err(|_| NativeConfigError::InvalidConfig)?,
            binding: String::from_utf8(binding).map_err(|_| NativeConfigError::InvalidConfig)?,
        };
        let result = Self::checked(wire)?;
        result.encode()?;
        Ok((result, guard))
    }
    fn checked(wire: Wire) -> Result<Self, NativeConfigError> {
        if wire.schema != 1
            || !wire.root.is_absolute()
            || wire.root.as_os_str().len() > 4096
            || wire.bootstrap.len() > 256 * 1024
            || wire.config.len() > 256 * 1024
            || wire.binding.len() > 64 * 1024
        {
            return Err(NativeConfigError::InvalidConfig);
        }
        let result = Self {
            pins: IncomingPins::open(&wire.root)?,
            wire,
        };
        let config = result.inspect(result.wire.config.as_bytes())?;
        if config.native_id != result.wire.identity.native_id {
            return Err(NativeConfigError::IdentityChanged);
        }
        Ok(result)
    }
    pub(crate) fn encode(&self) -> Result<Vec<u8>, NativeConfigError> {
        let bytes = serde_json::to_vec(&self.wire).map_err(|_| NativeConfigError::InvalidConfig)?;
        if bytes.len() > MAX_COMMAND {
            return Err(NativeConfigError::InvalidConfig);
        }
        Ok(bytes)
    }
    pub(crate) fn acknowledgement(&self) -> Result<[u8; 32], NativeConfigError> {
        Ok(Sha256::digest(self.encode()?).into())
    }
    pub(super) fn identity(&self) -> &NativeIdentity {
        &self.wire.identity
    }
    pub(super) fn ports(&self) -> NativePorts {
        self.wire.ports
    }
    pub(super) fn inspect(&self, bytes: &[u8]) -> Result<native_xml::Config, NativeConfigError> {
        self.pins.validate()?;
        native_xml::exchange::inspect_applied(
            bytes,
            self.wire.config.as_bytes(),
            self.wire.bootstrap.as_bytes(),
            self.wire.ports,
            self.wire
                .root
                .join("transport/incoming")
                .to_str()
                .ok_or(NativeConfigError::InvalidConfig)?,
        )
    }
    #[cfg(all(target_os = "macos", feature = "m8-local-startup-check"))]
    pub(crate) fn from_authenticated_parent(
        bytes: &[u8],
        runtime: &Path,
    ) -> Result<Self, NativeConfigError> {
        if !crate::m8_owner_channel::transport_helper::role_active() || bytes.len() > MAX_COMMAND {
            return Err(NativeConfigError::InvalidState);
        }
        cryptex_events::reject_duplicate_json_keys(
            std::str::from_utf8(bytes).map_err(|_| NativeConfigError::InvalidConfig)?,
        )
        .map_err(|_| NativeConfigError::InvalidConfig)?;
        let wire: Wire =
            serde_json::from_slice(bytes).map_err(|_| NativeConfigError::InvalidConfig)?;
        // Directory rejects user-controlled symlinks and retains original edges;
        // only then resolve Apple's permitted /var -> /private/var spelling.
        let root_pin = crate::import_staging::macos_fs::Directory::open(&wire.root, true)
            .map_err(|_| NativeConfigError::InvalidState)?;
        if wire
            .root
            .canonicalize()
            .map_err(|_| NativeConfigError::InvalidState)?
            != runtime
        {
            return Err(NativeConfigError::InvalidState);
        }
        root_pin
            .validate()
            .map_err(|_| NativeConfigError::InvalidState)?;
        Self::checked(wire)
    }
}
impl InstallPolicy for ExchangeImage {
    fn prepare_bootstrap(&self, bytes: &[u8]) -> Result<Vec<u8>, ExchangeConfigError> {
        self.pins
            .validate()
            .map_err(|_| ExchangeConfigError::InvalidRuntimePath)?;
        native_xml::inspect(bytes, self.wire.ports, false)?;
        if !native_xml::exchange::same_normalized(bytes, self.wire.bootstrap.as_bytes())? {
            return Err(ExchangeConfigError::ConfigurationMismatch);
        }
        Ok(self.wire.config.as_bytes().to_vec())
    }
    fn binding_bytes(&self, bytes: &[u8]) -> Result<Vec<u8>, ExchangeConfigError> {
        self.inspect(bytes)
            .map_err(|_| ExchangeConfigError::ConfigurationMismatch)?;
        Ok(self.wire.binding.as_bytes().to_vec())
    }
    fn verify_binding(&self, config: &[u8], binding: &[u8]) -> Result<(), ExchangeConfigError> {
        self.inspect(config)
            .map_err(|_| ExchangeConfigError::ConfigurationMismatch)?;
        if binding != self.wire.binding.as_bytes() {
            return Err(ExchangeConfigError::BindingMismatch);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cryptex_events::TrustedDaemonKeyResolver;

    #[test]
    fn fresh_images_bind_nonce_identity_config_and_admission() {
        const LOCAL: &str = "U2SYIGU-YWZX757-2WPZGCR-DHD7MLH-ZKV7HLJ-EYO6CXE-K3DU3DU-SDFXYAE";
        const REMOTE: &str = "XAAPCAD-GA4SF4J-BNQL3FS-T456WKV-FGLXUKO-CDWHVDW-EYWNRIG-SM5GUAZ";
        let (source, receiver, negative, store) =
            crate::trusted_import_probe::tests::pair_with_transport(LOCAL, REMOTE);
        #[cfg(windows)]
        std::fs::create_dir_all(source.join("transport/incoming")).unwrap();
        #[cfg(target_os = "macos")]
        crate::import_staging::macos_fs::create_dir_all_owned(&source.join("transport/incoming"))
            .unwrap();
        {
            let owner = crate::instance::try_acquire_instance(&source).unwrap();
            let gate = owner.admission();
            let permit = gate
                .exclusive(
                    &source,
                    std::time::Instant::now() + std::time::Duration::from_secs(3),
                    false,
                )
                .unwrap();
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
            let peers = crate::managed_profile::exchange_policy::ExchangePeers::from_snapshot(
                &snapshot, &source, LOCAL,
            )
            .unwrap();
            let ports = NativePorts::new(18384, 22001).unwrap();
            let policy = ExchangeConfigPolicy::new(
                &peers,
                ports,
                "192.168.77.1:22001".parse().unwrap(),
                std::collections::BTreeMap::from([(
                    REMOTE.to_owned(),
                    "192.168.77.2:22002".parse().unwrap(),
                )]),
            )
            .unwrap();
            let bootstrap = native_xml::inspect(
                include_bytes!("native_xml/exchange/bootstrap-test.xml"),
                ports,
                true,
            )
            .unwrap();
            let identity = || NativeIdentity {
                credential_id: "a".repeat(32),
                native_id: LOCAL.into(),
                cert_hash: "b".repeat(64),
                key_hash: "c".repeat(64),
            };
            let (first, guard) =
                ExchangeImage::from_policy(&policy, &permit, identity(), &bootstrap.bytes).unwrap();
            let (second, _) =
                ExchangeImage::from_policy(&policy, &permit, identity(), &bootstrap.bytes).unwrap();
            assert_ne!(
                first.acknowledgement().unwrap(),
                second.acknowledgement().unwrap()
            );
            let serialized = String::from_utf8(bootstrap.bytes.clone())
                .unwrap()
                .replace("><", ">\n  <");
            assert!(first.prepare_bootstrap(serialized.as_bytes()).is_ok());
            assert!(
                first
                    .verify_binding(first.wire.config.as_bytes(), first.wire.binding.as_bytes())
                    .is_ok()
            );
            assert!(first.inspect(bootstrap.bytes.as_slice()).is_err());
            let mut foreign = identity();
            foreign.native_id = REMOTE.into();
            assert!(
                ExchangeImage::from_policy(&policy, &permit, foreign, &bootstrap.bytes).is_err()
            );
            let changed = first.wire.config.replace("sendreceive", "sendonly");
            assert!(first.inspect(changed.as_bytes()).is_err());
            assert!(
                first
                    .verify_binding(first.wire.config.as_bytes(), b"{}")
                    .is_err()
            );
            drop(snapshot);
            drop(permit); // Installation without acknowledged completion holds the runtime.
            assert!(guard.check().is_err());
        }
        for root in [source, receiver, negative] {
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}
