//! Explicit native integration fixture only; absent from every product binary.
use super::*;
use std::path::{Component, Path};

#[derive(Clone)]
pub(crate) struct TransferTestPolicy {
    peer: Node,
    folder: Node,
    self_id: String,
}

fn bytes(node: &Node) -> Result<Vec<u8>> {
    // XML child ordering is not authority; native config serialization can sort
    // peer entries. Keep every attribute/value/duplicate in the comparison.
    fn normalized(node: &Node) -> Result<Node> {
        let mut node = node.clone();
        node.children = node
            .children
            .iter()
            .map(normalized)
            .collect::<Result<_>>()?;
        node.children.sort_by_cached_key(|child| {
            let mut writer = Writer::new(Vec::new());
            child
                .write(&mut writer)
                .expect("parsed XML writes to a Vec");
            writer.into_inner()
        });
        Ok(node)
    }
    let mut writer = Writer::new(Vec::new());
    normalized(node)?.write(&mut writer)?;
    Ok(writer.into_inner())
}

impl TransferTestPolicy {
    pub(crate) fn create(
        config: &[u8],
        ports: NativePorts,
        peer_id: &str,
        address: &str,
        folder_id: &str,
        incoming: &Path,
    ) -> Result<(Self, Vec<u8>)> {
        let checked = inspect(config, ports, false)?;
        let endpoint: std::net::SocketAddrV4 = address
            .strip_prefix("tcp://")
            .ok_or(NativeConfigError::InvalidConfig)?
            .parse()
            .map_err(|_| NativeConfigError::InvalidConfig)?;
        if !native_id_shape(peer_id)
            || peer_id == checked.native_id
            || !(endpoint.ip().is_private() || endpoint.ip().is_loopback())
            || endpoint.port() == 0
            || folder_id.len() != 42
            || !folder_id.starts_with("cx-native-")
            || !folder_id[10..]
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            || !incoming.is_absolute()
            || incoming
                .components()
                .any(|c| matches!(c, Component::ParentDir))
            || !incoming.ends_with("transport/incoming")
            || crate::paths::assert_outside_repository(incoming).is_err()
        {
            return Err(NativeConfigError::InvalidConfig);
        }
        let mut root = parse(config)?;
        if root
            .one("options")?
            .one("overwriteRemoteDeviceNamesOnConnect")?
            .value()?
            != "false"
        {
            return Err(NativeConfigError::InvalidConfig);
        }
        let defaults = root.one("defaults")?;
        let mut peer = defaults.one("device")?.clone();
        peer.attrs.insert("id".into(), peer_id.into());
        // Native hello must not fill an empty name and change the pinned config.
        peer.attrs
            .insert("name".into(), "Synthetic native relay".into());
        peer.attrs.insert("introducer".into(), "false".into());
        peer.set("address", address)?;
        peer.set("autoAcceptFolders", "false")?;
        let mut folder = defaults.one("folder")?.clone();
        for (k, v) in [
            ("id", folder_id),
            (
                "path",
                incoming.to_str().ok_or(NativeConfigError::InvalidConfig)?,
            ),
            ("type", "receiveonly"),
            ("rescanIntervalS", "1"),
            ("fsWatcherEnabled", "false"),
        ] {
            folder.attrs.insert(k.into(), v.into());
        }
        let mut member = folder.one("device")?.clone();
        folder.children.retain(|n| n.name != "device");
        member.set("encryptionPassword", "")?;
        for id in [checked.native_id.as_str(), peer_id] {
            member.attrs.insert("id".into(), id.into());
            folder.children.push(member.clone());
        }
        let policy = Self {
            peer,
            folder,
            self_id: checked.native_id,
        };
        root.children.push(policy.peer.clone());
        root.children.push(policy.folder.clone());
        let encoded = bytes(&root)?;
        policy.checked_bootstrap_config(&encoded)?;
        Ok((policy, encoded))
    }

    pub(crate) fn checked_bootstrap_config(&self, config: &[u8]) -> Result<Vec<u8>> {
        let mut root = parse(config)?;
        if root
            .one("options")?
            .one("overwriteRemoteDeviceNamesOnConnect")?
            .value()?
            != "false"
        {
            return Err(NativeConfigError::InvalidConfig);
        }
        let peer_id = &self.peer.attrs["id"];
        let peers: Vec<_> = root
            .children
            .iter()
            .filter(|n| n.name == "device" && n.attrs.get("id") == Some(peer_id))
            .collect();
        let folders: Vec<_> = root
            .children
            .iter()
            .filter(|n| n.name == "folder")
            .collect();
        if peers.len() != 1
            || folders.len() != 1
            || bytes(peers[0])? != bytes(&self.peer)?
            || bytes(folders[0])? != bytes(&self.folder)?
        {
            return Err(NativeConfigError::InvalidConfig);
        }
        root.children.retain(|n| {
            n.name != "folder" && !(n.name == "device" && n.attrs.get("id") == Some(peer_id))
        });
        if root.one("device")?.attrs.get("id") != Some(&self.self_id) {
            return Err(NativeConfigError::IdentityChanged);
        }
        bytes(&root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    const PEER: &str = "U2SYIGU-YWZX757-2WPZGCR-DHD7MLH-ZKV7HLJ-EYO6CXE-K3DU3DU-SDFXYAE";
    fn base() -> (Vec<u8>, NativePorts, std::path::PathBuf) {
        let raw = super::super::super::native::tests::synthetic_xml().replace(
            "<defaults><device><autoAcceptFolders>false</autoAcceptFolders></device></defaults>",
            "<defaults><device><address>dynamic</address><autoAcceptFolders>false</autoAcceptFolders></device><folder><device id=\"\"><encryptionPassword></encryptionPassword></device></folder></defaults>");
        let raw = raw.replace("</options>", "<overwriteRemoteDeviceNamesOnConnect>false</overwriteRemoteDeviceNamesOnConnect></options>");
        let ports = NativePorts::new(18384, 22001).unwrap();
        (
            inspect(raw.as_bytes(), ports, true).unwrap().bytes,
            ports,
            std::env::temp_dir().join("synthetic-transfer-policy/transport/incoming"),
        )
    }
    #[test]
    fn exact_fixture_exception_does_not_enable_product_policy() {
        let (base, ports, incoming) = base();
        let (policy, bytes) = TransferTestPolicy::create(
            &base,
            ports,
            PEER,
            "tcp://192.168.177.1:24000",
            "cx-native-0123456789abcdef0123456789abcdef",
            &incoming,
        )
        .unwrap();
        assert!(inspect(&bytes, ports, false).is_err());
        assert!(
            inspect(
                &policy.checked_bootstrap_config(&bytes).unwrap(),
                ports,
                false
            )
            .is_ok()
        );
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("name=\"Synthetic native relay\""));
        for (from, to) in [
            ("receiveonly", "sendreceive"),
            ("tcp://192.168.177.1:24000", "dynamic"),
            ("rescanIntervalS=\"1\"", "rescanIntervalS=\"2\""),
            (
                "<encryptionPassword></encryptionPassword>",
                "<encryptionPassword>x</encryptionPassword>",
            ),
        ] {
            assert!(text.contains(from));
            assert!(
                policy
                    .checked_bootstrap_config(text.replace(from, to).as_bytes())
                    .is_err()
            );
        }
        // Even when the fixture nodes match, the normal network/auth gate runs.
        let weakened = text.replace("<relaysEnabled>false", "<relaysEnabled>true");
        let stripped = policy
            .checked_bootstrap_config(weakened.as_bytes())
            .unwrap();
        assert!(inspect(&stripped, ports, false).is_err());
        let duplicated = text.replace(
            "</configuration>",
            &format!(
                "{}{}</configuration>",
                String::from_utf8(super::bytes(&policy.peer).unwrap()).unwrap(),
                String::from_utf8(super::bytes(&policy.folder).unwrap()).unwrap()
            ),
        );
        assert!(
            policy
                .checked_bootstrap_config(duplicated.as_bytes())
                .is_err()
        );
    }
    #[test]
    fn peer_endpoint_and_destination_are_narrowly_scoped() {
        let (base, ports, incoming) = base();
        let folder = "cx-native-0123456789abcdef0123456789abcdef";
        for address in [
            "tcp://8.8.8.8:22000",
            "tcp://localhost:22000",
            "tcp://192.168.1.1:0",
            "dynamic",
            "tcp://[::1]:22000",
        ] {
            assert!(
                TransferTestPolicy::create(&base, ports, PEER, address, folder, &incoming).is_err()
            );
        }
        for path in [
            PathBuf::from("relative/transport/incoming"),
            incoming.join("../other"),
            incoming.join("canonical"),
        ] {
            assert!(
                TransferTestPolicy::create(
                    &base,
                    ports,
                    PEER,
                    "tcp://127.0.0.1:22000",
                    folder,
                    &path
                )
                .is_err()
            );
        }
        assert!(
            TransferTestPolicy::create(
                &base,
                ports,
                "invalid",
                "tcp://127.0.0.1:22000",
                folder,
                &incoming
            )
            .is_err()
        );
    }
}
