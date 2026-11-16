//! Trust-derived configuration, not a process-start or filesystem capability.
//! The controller must retain runtime admission and native directory pins when
//! installing these bytes. A derived binding is never accepted as peer authority:
//! restart must reconstruct this policy from a fresh locked canonical snapshot.
use std::net::SocketAddrV4;
use std::path::Component;

use cryptex_events::reject_duplicate_json_keys;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::*;
use crate::managed_profile::exchange_policy::ExchangePeers;

const MAX_PEERS: usize = 64;
const MAX_BINDING: usize = 64 * 1024;

// Explicit pinned 2.1.5 schema; NEVER clone the mutable defaults into active
// peers/folders. Defaults can contain arbitrary paths, versioners or ignores.
const PEER: &str = r#"<device id="" name="Cryptex peer" compression="metadata" introducer="false" skipIntroductionRemovals="false" introducedBy=""><address></address><paused>false</paused><autoAcceptFolders>false</autoAcceptFolders><maxSendKbps>0</maxSendKbps><maxRecvKbps>0</maxRecvKbps><maxRequestKiB>0</maxRequestKiB><untrusted>false</untrusted><remoteGUIPort>0</remoteGUIPort><numConnections>0</numConnections></device>"#;
const FOLDER: &str = r#"<folder id="" label="Cryptex exchange" path="" type="sendreceive" rescanIntervalS="30" fsWatcherEnabled="false" fsWatcherDelayS="10" fsWatcherTimeoutS="0" ignorePerms="true" autoNormalize="false"><filesystemType>basic</filesystemType><minDiskFree unit="%">1</minDiskFree><versioning><cleanupIntervalS>3600</cleanupIntervalS><fsPath></fsPath><fsType>basic</fsType></versioning><copiers>0</copiers><pullerMaxPendingKiB>0</pullerMaxPendingKiB><hashers>0</hashers><order>random</order><ignoreDelete>false</ignoreDelete><scanProgressIntervalS>0</scanProgressIntervalS><pullerPauseS>0</pullerPauseS><pullerDelayS>1</pullerDelayS><maxConflicts>10</maxConflicts><disableSparseFiles>false</disableSparseFiles><paused>false</paused><markerName>.stfolder</markerName><copyOwnershipFromParent>false</copyOwnershipFromParent><modTimeWindowS>0</modTimeWindowS><maxConcurrentWrites>16</maxConcurrentWrites><disableFsync>false</disableFsync><blockPullOrder>standard</blockPullOrder><copyRangeMethod>standard</copyRangeMethod><caseSensitiveFS>false</caseSensitiveFS><junctionsAsDirs>false</junctionsAsDirs><syncOwnership>false</syncOwnership><sendOwnership>false</sendOwnership><syncXattrs>false</syncXattrs><sendXattrs>false</sendXattrs><blockIndexing>true</blockIndexing><xattrFilter><maxSingleEntrySize>1024</maxSingleEntrySize><maxTotalSize>4096</maxTotalSize></xattrFilter></folder>"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExchangeConfigError {
    AuthorityUnavailable,
    InvalidLanEndpoints,
    InvalidRuntimePath,
    CapacityExceeded,
    ConfigurationMismatch,
    BindingMismatch,
}
impl std::fmt::Display for ExchangeConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::AuthorityUnavailable => "exchange_authority_unavailable",
            Self::InvalidLanEndpoints => "exchange_lan_endpoints_invalid",
            Self::InvalidRuntimePath => "exchange_runtime_path_invalid",
            Self::CapacityExceeded => "exchange_config_capacity_exceeded",
            Self::ConfigurationMismatch => "exchange_configuration_mismatch",
            Self::BindingMismatch => "exchange_binding_mismatch",
        })
    }
}
impl std::error::Error for ExchangeConfigError {}
impl From<NativeConfigError> for ExchangeConfigError {
    fn from(_: NativeConfigError) -> Self {
        Self::ConfigurationMismatch
    }
}
type ExchangeResult<T> = std::result::Result<T, ExchangeConfigError>;

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct PeerBinding {
    device_id: String,
    signing_key_id: String,
    fingerprint_sha256: [u8; 32],
    transport_id: String,
    endpoint: SocketAddrV4,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Binding {
    schema: u32,
    engine_version: String,
    brain_id: String,
    local_transport: String,
    incoming_path: String,
    folder_id: String,
    ports: NativePorts,
    listen: SocketAddrV4,
    peers: Vec<PeerBinding>,
    config_sha256: String,
}

/// No Clone/Deserialize: all use of a plan borrows current canonical authority.
/// It performs no filesystem mutation or launch. In particular lexical path
/// derivation below is not proof against links; installation needs native pins.
pub(crate) struct ExchangeConfigPolicy<'a> {
    authority: &'a ExchangePeers<'a>,
    ports: NativePorts,
    listen: SocketAddrV4,
    endpoints: BTreeMap<String, SocketAddrV4>,
    folder_id: String,
    incoming: String,
    peers: Vec<Node>,
    folder: Node,
}

fn canonical_bytes(node: &Node) -> Result<Vec<u8>> {
    fn normalize(node: &Node) -> Result<Node> {
        let mut node = node.clone();
        let mut children = node
            .children
            .iter()
            .map(|child| {
                let child = normalize(child)?;
                let mut writer = Writer::new(Vec::new());
                child.write(&mut writer)?;
                Ok((writer.into_inner(), child))
            })
            .collect::<Result<Vec<_>>>()?;
        children.sort_by(|a, b| a.0.cmp(&b.0));
        node.children = children.into_iter().map(|(_, node)| node).collect();
        Ok(node)
    }
    let mut writer = Writer::new(Vec::new());
    normalize(node)?.write(&mut writer)?;
    let mut bytes = writer.into_inner();
    bytes.push(b'\n');
    if bytes.len() > MAX_CONFIG {
        return Err(NativeConfigError::InvalidConfig);
    }
    Ok(bytes)
}

fn lan(endpoint: SocketAddrV4) -> bool {
    endpoint.ip().is_private() && endpoint.port() != 0
}

impl<'a> ExchangeConfigPolicy<'a> {
    pub(crate) fn new(
        authority: &'a ExchangePeers<'a>,
        ports: NativePorts,
        listen: SocketAddrV4,
        endpoints: BTreeMap<String, SocketAddrV4>,
    ) -> ExchangeResult<Self> {
        authority
            .revalidate()
            .map_err(|_| ExchangeConfigError::AuthorityUnavailable)?;
        ports.validate()?;
        if authority.bindings().is_empty() {
            return Err(ExchangeConfigError::AuthorityUnavailable);
        }
        if authority.bindings().len() > MAX_PEERS {
            return Err(ExchangeConfigError::CapacityExceeded);
        }
        let mut seen = std::collections::BTreeSet::from([listen]);
        if !lan(listen)
            || listen.port() != ports.sync
            || endpoints.len() != authority.bindings().len()
            || authority.bindings().iter().any(|peer| {
                endpoints
                    .get(&peer.transport_device_id)
                    .is_none_or(|address| !lan(*address) || !seen.insert(*address))
            })
        {
            return Err(ExchangeConfigError::InvalidLanEndpoints);
        }
        let root = authority.runtime_root();
        if !root.is_absolute()
            || root
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
            || crate::paths::assert_outside_repository(root).is_err()
        {
            return Err(ExchangeConfigError::InvalidRuntimePath);
        }
        let incoming = root.join("transport/incoming");
        let incoming = incoming
            .to_str()
            .ok_or(ExchangeConfigError::InvalidRuntimePath)?;
        if incoming.len() > 1024 || !incoming.chars().all(xml_character) {
            return Err(ExchangeConfigError::InvalidRuntimePath);
        }
        let folder_id = format!(
            "cryptex-{:x}",
            Sha256::digest(authority.brain_id().as_bytes())
        );
        let mut folder = parse(FOLDER.as_bytes())?;
        folder.attrs.insert("id".into(), folder_id.clone());
        folder.attrs.insert("path".into(), incoming.into());
        for id in std::iter::once(authority.local_transport()).chain(
            authority
                .bindings()
                .iter()
                .map(|peer| peer.transport_device_id.as_str()),
        ) {
            let mut member = parse(br#"<device id="" introducedBy=""><encryptionPassword></encryptionPassword></device>"#)?;
            member.attrs.insert("id".into(), id.into());
            folder.children.push(member);
        }
        let peers = authority
            .bindings()
            .iter()
            .map(|binding| {
                let mut peer = parse(PEER.as_bytes())?;
                peer.attrs
                    .insert("id".into(), binding.transport_device_id.clone());
                peer.set(
                    "address",
                    &format!("tcp://{}", endpoints[&binding.transport_device_id]),
                )?;
                Ok(peer)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            authority,
            ports,
            listen,
            endpoints,
            folder_id,
            incoming: incoming.into(),
            peers,
            folder,
        })
    }

    fn current(&self) -> ExchangeResult<()> {
        self.authority
            .revalidate()
            .map_err(|_| ExchangeConfigError::AuthorityUnavailable)
    }

    /// Only a self-only validated bootstrap may be converted. An already active
    /// or drifted configuration is never repaired by deleting unexpected nodes.
    pub(crate) fn prepare_bootstrap(&self, bytes: &[u8]) -> ExchangeResult<Vec<u8>> {
        self.current()?;
        let config = inspect(bytes, self.ports, false)?;
        if config.native_id != self.authority.local_transport() {
            return Err(ExchangeConfigError::ConfigurationMismatch);
        }
        let mut root = parse(&config.bytes)?;
        let options = root.one_mut("options")?;
        if options
            .one("overwriteRemoteDeviceNamesOnConnect")?
            .value()?
            != "false"
        {
            return Err(ExchangeConfigError::ConfigurationMismatch);
        }
        options.set("listenAddress", &format!("tcp://{}", self.listen))?;
        root.children.extend(self.peers.iter().cloned());
        root.children.push(self.folder.clone());
        let bytes = canonical_bytes(&root)?;
        self.inspect_exchange(&bytes)?;
        Ok(bytes)
    }

    /// Complete active peer/folder equality, including every child/attribute,
    /// followed by the ordinary bootstrap network/authentication checks.
    pub(crate) fn inspect_exchange(&self, bytes: &[u8]) -> ExchangeResult<Config> {
        self.current()?;
        let mut root = parse(bytes)?;
        let folders: Vec<_> = root
            .children
            .iter()
            .filter(|n| n.name == "folder")
            .collect();
        if folders.len() != 1 || canonical_bytes(folders[0])? != canonical_bytes(&self.folder)? {
            return Err(ExchangeConfigError::ConfigurationMismatch);
        }
        let mut expected: BTreeMap<_, _> = self
            .peers
            .iter()
            .map(|peer| (peer.attrs["id"].as_str(), peer))
            .collect();
        let mut self_count = 0;
        for peer in root.children.iter().filter(|n| n.name == "device") {
            let id = peer
                .attrs
                .get("id")
                .ok_or(ExchangeConfigError::ConfigurationMismatch)?;
            if id == self.authority.local_transport() {
                self_count += 1;
            } else {
                let required = expected
                    .remove(id.as_str())
                    .ok_or(ExchangeConfigError::ConfigurationMismatch)?;
                if canonical_bytes(peer)? != canonical_bytes(required)? {
                    return Err(ExchangeConfigError::ConfigurationMismatch);
                }
            }
        }
        if self_count != 1 || !expected.is_empty() {
            return Err(ExchangeConfigError::ConfigurationMismatch);
        }
        let options = root.one_mut("options")?;
        if options.one("listenAddress")?.value()? != format!("tcp://{}", self.listen)
            || options
                .one("overwriteRemoteDeviceNamesOnConnect")?
                .value()?
                != "false"
        {
            return Err(ExchangeConfigError::ConfigurationMismatch);
        }
        options.set(
            "listenAddress",
            &format!("tcp://127.0.0.1:{}", self.ports.sync),
        )?;
        root.children.retain(|n| {
            n.name != "folder"
                && (n.name != "device"
                    || n.attrs
                        .get("id")
                        .is_some_and(|id| id == self.authority.local_transport()))
        });
        let mut config = inspect(&canonical_bytes(&root)?, self.ports, false)?;
        config.bytes = canonical_bytes(&parse(bytes)?)?;
        Ok(config)
    }

    fn binding(&self, bytes: &[u8]) -> ExchangeResult<Binding> {
        let config = self.inspect_exchange(bytes)?;
        Ok(Binding {
            schema: 1,
            engine_version: crate::syncthing_package::VERSION.into(),
            brain_id: self.authority.brain_id().into(),
            local_transport: self.authority.local_transport().into(),
            incoming_path: self.incoming.clone(),
            folder_id: self.folder_id.clone(),
            ports: self.ports,
            listen: self.listen,
            peers: self
                .authority
                .bindings()
                .iter()
                .map(|peer| PeerBinding {
                    device_id: peer.device_id.as_str().into(),
                    signing_key_id: peer.signing_key_id.as_str().into(),
                    fingerprint_sha256: peer.fingerprint_sha256,
                    transport_id: peer.transport_device_id.clone(),
                    endpoint: self.endpoints[&peer.transport_device_id],
                })
                .collect(),
            config_sha256: format!("{:x}", Sha256::digest(&config.bytes)),
        })
    }

    pub(crate) fn binding_bytes(&self, config: &[u8]) -> ExchangeResult<Vec<u8>> {
        let bytes = serde_json::to_vec(&self.binding(config)?)
            .map_err(|_| ExchangeConfigError::BindingMismatch)?;
        if bytes.len() > MAX_BINDING {
            return Err(ExchangeConfigError::CapacityExceeded);
        }
        Ok(bytes)
    }

    /// Parse a bounded untrusted derived receipt and compare with fresh canonical
    /// authority AND full validated configuration. Receipt alone cannot reopen.
    pub(crate) fn verify_binding(&self, config: &[u8], bytes: &[u8]) -> ExchangeResult<()> {
        if bytes.is_empty() || bytes.len() > MAX_BINDING {
            return Err(ExchangeConfigError::BindingMismatch);
        }
        let text = std::str::from_utf8(bytes).map_err(|_| ExchangeConfigError::BindingMismatch)?;
        reject_duplicate_json_keys(text).map_err(|_| ExchangeConfigError::BindingMismatch)?;
        let binding: Binding =
            serde_json::from_slice(bytes).map_err(|_| ExchangeConfigError::BindingMismatch)?;
        if binding != self.binding(config)? {
            return Err(ExchangeConfigError::BindingMismatch);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
