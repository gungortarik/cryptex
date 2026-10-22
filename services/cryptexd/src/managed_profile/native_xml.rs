//! Bounded strict XML policy for the pinned native configuration, not a generic
//! user config importer. No DTD/entity resolution, arbitrary XML or schema upgrade.
use std::collections::BTreeMap;

use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};

use super::native::{NativeConfigError, NativePorts};
use crate::syncthing_rest::{ApiCredential, native_id_shape};

const MAX_CONFIG: usize = 256 * 1024;
type Result<T> = std::result::Result<T, NativeConfigError>;

fn xml_character(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r')
        || matches!(c as u32, 0x20..=0xd7ff | 0xe000..=0xfffd | 0x10000..=0x10ffff)
}

#[derive(Clone)]
struct Node {
    name: String,
    attrs: BTreeMap<String, String>,
    text: String,
    children: Vec<Node>,
}

impl Node {
    fn from_start(start: &BytesStart<'_>) -> Result<Self> {
        fn name(s: &str) -> Result<String> {
            if s.is_empty()
                || s.len() > 64
                || !s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
            {
                return Err(NativeConfigError::InvalidConfig);
            }
            Ok(s.to_owned())
        }
        let mut attrs = BTreeMap::new();
        for attribute in start.attributes() {
            let attribute = attribute.map_err(|_| NativeConfigError::InvalidConfig)?;
            let key = name(attribute.key.0)?;
            if key == "xmlns" || attrs.len() >= 32 {
                return Err(NativeConfigError::InvalidConfig);
            }
            let value = attribute
                .normalized_value(quick_xml::XmlVersion::Explicit1_0)
                .map_err(|_| NativeConfigError::InvalidConfig)?
                .into_owned();
            if !value.chars().all(xml_character) {
                return Err(NativeConfigError::InvalidConfig);
            }
            if attrs.insert(key, value).is_some() {
                return Err(NativeConfigError::InvalidConfig);
            }
        }
        Ok(Self {
            name: name(start.name().0)?,
            attrs,
            text: String::new(),
            children: Vec::new(),
        })
    }

    fn one(&self, name: &str) -> Result<&Self> {
        let mut matches = self.children.iter().filter(|node| node.name == name);
        let found = matches.next().ok_or(NativeConfigError::InvalidConfig)?;
        if matches.next().is_some() {
            return Err(NativeConfigError::InvalidConfig);
        }
        Ok(found)
    }

    fn one_mut(&mut self, name: &str) -> Result<&mut Self> {
        if self
            .children
            .iter()
            .filter(|node| node.name == name)
            .count()
            != 1
        {
            return Err(NativeConfigError::InvalidConfig);
        }
        Ok(self
            .children
            .iter_mut()
            .find(|node| node.name == name)
            .unwrap())
    }

    fn value(&self) -> Result<&str> {
        if !self.children.is_empty() || !self.attrs.is_empty() {
            return Err(NativeConfigError::InvalidConfig);
        }
        Ok(self.text.trim())
    }

    fn set(&mut self, name: &str, value: &str) -> Result<()> {
        let node = self.one_mut(name)?;
        node.value()?;
        node.text = value.to_owned();
        Ok(())
    }

    fn write(&self, writer: &mut Writer<Vec<u8>>) -> Result<()> {
        let mut start = BytesStart::new(self.name.as_str());
        for (key, value) in &self.attrs {
            start.push_attribute((key.as_str(), value.as_str()));
        }
        writer
            .write_event(Event::Start(start))
            .map_err(|_| NativeConfigError::InvalidConfig)?;
        if self.children.is_empty() {
            writer
                .write_event(Event::Text(BytesText::new(self.text.trim())))
                .map_err(|_| NativeConfigError::InvalidConfig)?;
        } else {
            for child in &self.children {
                child.write(writer)?;
            }
        }
        writer
            .write_event(Event::End(BytesEnd::new(self.name.as_str())))
            .map_err(|_| NativeConfigError::InvalidConfig)
    }
}

fn parse(bytes: &[u8]) -> Result<Node> {
    if bytes.len() > MAX_CONFIG {
        return Err(NativeConfigError::InvalidConfig);
    }
    let xml = std::str::from_utf8(bytes).map_err(|_| NativeConfigError::InvalidConfig)?;
    // XML 1.0 controls are forbidden even when a non-validating reader accepts
    // them; non-UTF8 encodings and declarations with changed semantics refuse.
    if !xml.chars().all(xml_character) {
        return Err(NativeConfigError::InvalidConfig);
    }
    let mut reader = Reader::from_str(xml);
    let mut stack: Vec<Node> = Vec::new();
    let mut root = None;
    let mut count = 0;
    let mut declaration_seen = false;
    loop {
        let event = reader
            .read_event()
            .map_err(|_| NativeConfigError::InvalidConfig)?;
        match event {
            Event::Start(ref start) | Event::Empty(ref start) => {
                count += 1;
                if count > 4096 || stack.len() >= 16 || root.is_some() {
                    return Err(NativeConfigError::InvalidConfig);
                }
                let node = Node::from_start(start)?;
                if matches!(event, Event::Empty(_)) {
                    attach(&mut stack, &mut root, node)?;
                } else {
                    stack.push(node);
                }
            }
            Event::End(end) => {
                let node = stack.pop().ok_or(NativeConfigError::InvalidConfig)?;
                if end.name().0 != node.name {
                    return Err(NativeConfigError::InvalidConfig);
                }
                attach(&mut stack, &mut root, node)?;
            }
            Event::Text(text) => {
                let text = text.xml10_content();
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&text);
                } else if !text.trim().is_empty() {
                    return Err(NativeConfigError::InvalidConfig);
                }
            }
            Event::Decl(decl) if count == 0 && !declaration_seen => {
                declaration_seen = true;
                if decl
                    .version()
                    .map_err(|_| NativeConfigError::InvalidConfig)?
                    != "1.0"
                    || decl
                        .encoding()
                        .is_some_and(|enc| !enc.is_ok_and(|enc| enc.eq_ignore_ascii_case("utf-8")))
                {
                    return Err(NativeConfigError::InvalidConfig);
                }
            }
            Event::Eof if stack.is_empty() => break,
            // No DTDs, processing instructions, CDATA, general references or
            // comments are needed by pinned native generation. Reject rather
            // than resolve/import unknown XML semantics or external resources.
            _ => return Err(NativeConfigError::InvalidConfig),
        }
    }
    root.ok_or(NativeConfigError::InvalidConfig)
}

fn attach(stack: &mut [Node], root: &mut Option<Node>, node: Node) -> Result<()> {
    if !node.children.is_empty() && !node.text.trim().is_empty() {
        return Err(NativeConfigError::InvalidConfig);
    }
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
    } else if root.replace(node).is_some() {
        return Err(NativeConfigError::InvalidConfig);
    }
    Ok(())
}

const SETTINGS: &[(&str, &str)] = &[
    ("globalAnnounceEnabled", "false"),
    ("localAnnounceEnabled", "false"),
    ("relaysEnabled", "false"),
    ("natEnabled", "false"),
    ("startBrowser", "false"),
    ("autoUpgradeIntervalH", "0"),
    ("upgradeToPreReleases", "false"),
    ("urAccepted", "-1"),
    ("crashReportingEnabled", "false"),
    ("auditEnabled", "false"),
];

pub(super) struct Config {
    pub(super) bytes: Vec<u8>,
    pub(super) key: Vec<u8>,
    pub(super) native_id: String,
}

pub(super) fn inspect(bytes: &[u8], ports: NativePorts, prepare: bool) -> Result<Config> {
    ports.validate()?;
    let mut root = parse(bytes)?;
    if root.name != "configuration"
        || root.attrs.len() != 1
        || root.attrs.get("version").map(String::as_str) != Some("52")
    {
        return Err(NativeConfigError::InvalidConfig);
    }
    if root.children.len() != 5
        || root
            .children
            .iter()
            .any(|n| !["device", "gui", "ldap", "options", "defaults"].contains(&n.name.as_str()))
    {
        return Err(NativeConfigError::InvalidConfig);
    }
    // Exactly one self device and no active folder/peer are admitted at this
    // bootstrap stage. The template inside defaults is not an active folder.
    let device = root.one("device")?;
    let native_id = device
        .attrs
        .get("id")
        .filter(|id| native_id_shape(id))
        .ok_or(NativeConfigError::InvalidConfig)?
        .clone();
    if device.attrs.get("introducer").map(String::as_str) != Some("false")
        || device
            .attrs
            .get("introducedBy")
            .is_some_and(|s| !s.is_empty())
        || device.one("autoAcceptFolders")?.value()? != "false"
    {
        return Err(NativeConfigError::InvalidConfig);
    }
    let ldap = root.one("ldap")?;
    if !ldap.attrs.is_empty() || !ldap.children.is_empty() || !ldap.text.trim().is_empty() {
        return Err(NativeConfigError::InvalidConfig);
    }
    root.one("defaults")?
        .one("device")?
        .one("autoAcceptFolders")?
        .value()
        .and_then(|v| {
            if v == "false" {
                Ok(v)
            } else {
                Err(NativeConfigError::InvalidConfig)
            }
        })?;
    let gui = root.one_mut("gui")?;
    if gui
        .attrs
        .keys()
        .any(|s| !["enabled", "tls", "sendBasicAuthPrompt"].contains(&s.as_str()))
        || gui.children.iter().any(|n| {
            ![
                "address",
                "metricsWithoutAuth",
                "apikey",
                "theme",
                "sessionCookieDurationS",
                "sessionCookiePath",
            ]
            .contains(&n.name.as_str())
        })
    {
        return Err(NativeConfigError::InvalidConfig);
    }
    let names: std::collections::BTreeSet<_> =
        gui.children.iter().map(|node| node.name.as_str()).collect();
    if names.len() != gui.children.len() {
        return Err(NativeConfigError::InvalidConfig);
    }
    let key_node = gui.one("apikey")?;
    let key_text = key_node.value()?;
    if key_node.text != key_text {
        return Err(NativeConfigError::InvalidConfig);
    }
    let key = key_text.as_bytes().to_vec();
    if key.len() < 32 || ApiCredential::from_bytes(&key).is_err() {
        return Err(NativeConfigError::InvalidConfig);
    }
    for (key, value) in [("enabled", "true"), ("tls", "false")] {
        if prepare {
            gui.attrs.insert(key.to_owned(), value.to_owned());
        }
        if gui.attrs.get(key).map(String::as_str) != Some(value) {
            return Err(NativeConfigError::InvalidConfig);
        }
    }
    let address = format!("127.0.0.1:{}", ports.api);
    if prepare {
        gui.set("address", &address)?;
        gui.set("metricsWithoutAuth", "false")?;
    }
    if gui.one("address")?.value()? != address || gui.one("metricsWithoutAuth")?.value()? != "false"
    {
        return Err(NativeConfigError::InvalidConfig);
    }
    let options = root.one_mut("options")?;
    let listen = format!("tcp://127.0.0.1:{}", ports.sync);
    for (name, value) in SETTINGS
        .iter()
        .copied()
        .chain(std::iter::once(("listenAddress", listen.as_str())))
    {
        if prepare {
            options.set(name, value)?;
        }
        if options.one(name)?.value()? != value {
            return Err(NativeConfigError::InvalidConfig);
        }
    }
    let mut writer = Writer::new(Vec::new());
    root.write(&mut writer)?;
    let mut bytes = writer.into_inner();
    bytes.push(b'\n');
    if bytes.len() > MAX_CONFIG {
        return Err(NativeConfigError::InvalidConfig);
    }
    Ok(Config {
        bytes,
        key,
        native_id,
    })
}

#[cfg(all(test, windows))]
mod tests {
    use super::super::native::tests::synthetic_xml;
    use super::*;
    fn ports() -> NativePorts {
        NativePorts::new(18384, 22001).unwrap()
    }

    #[test]
    fn prepare_is_deterministic_preserves_key_and_refuses_unprepared_config() {
        let raw = synthetic_xml();
        assert!(inspect(raw.as_bytes(), ports(), false).is_err());
        let prepared = inspect(raw.as_bytes(), ports(), true).unwrap();
        let rechecked = inspect(&prepared.bytes, ports(), false).unwrap();
        assert_eq!(prepared.bytes, rechecked.bytes);
        assert_eq!(prepared.key, rechecked.key);
        assert!(!prepared.bytes.contains(&b'\r'));
    }

    #[test]
    fn malformed_duplicate_schema_folder_peer_and_xml_extensions_refuse() {
        let raw = synthetic_xml();
        let cases = [
            raw.replace("version=\"52\"","version=\"51\""),
            raw.replace("version=\"52\"","version=\"52\" version=\"52\""),
            raw.replace("</configuration>","<folder id=\"unexpected\"></folder></configuration>"),
            raw.replace("<ldap></ldap>","<ldap></ldap><device></device>"),
            raw.replace("<ldap></ldap>","<ldap><address>external.example</address></ldap>"),
            raw.replace("<metricsWithoutAuth>false</metricsWithoutAuth>","<metricsWithoutAuth>false</metricsWithoutAuth><metricsWithoutAuth>true</metricsWithoutAuth>"),
            raw.replace("<natEnabled>true</natEnabled>","<natEnabled>true</natEnabled><natEnabled>false</natEnabled>"),
            raw.replace("<gui enabled=\"true\"", "<gui insecureAdminAccess=\"true\" enabled=\"true\""),
            raw.replace("<gui", "<namespaced:gui"),
            format!("<!DOCTYPE configuration [<!ENTITY x SYSTEM 'file:///private'>]>{raw}"),
            raw.replace("<ldap></ldap>","<ldap><![CDATA[untrusted]]></ldap>"),
            raw.replace("<ldap></ldap>","<?unexpected test?><ldap></ldap>"),
            raw.replace("<ldap></ldap>","<ldap>&untrusted;</ldap>"),
            raw.replace("<ldap></ldap>","<ldap>\0</ldap>"),
            format!("{raw}{raw}"),
        ];
        for case in cases {
            assert!(inspect(case.as_bytes(), ports(), true).is_err());
        }
        assert!(inspect(&vec![b'x'; 256 * 1024 + 1], ports(), true).is_err());
    }

    #[test]
    fn ready_config_network_and_auth_weakening_refuse() {
        let prepared = inspect(synthetic_xml().as_bytes(), ports(), true).unwrap();
        let text = String::from_utf8(prepared.bytes).unwrap();
        for (from, to) in [
            ("127.0.0.1:18384", "0.0.0.0:18384"),
            ("tcp://127.0.0.1:22001", "default"),
            (
                "<globalAnnounceEnabled>false",
                "<globalAnnounceEnabled>true",
            ),
            ("<relaysEnabled>false", "<relaysEnabled>true"),
            (
                "<crashReportingEnabled>false",
                "<crashReportingEnabled>true",
            ),
            ("<autoUpgradeIntervalH>0", "<autoUpgradeIntervalH>1"),
            ("tls=\"false\"", "tls=\"true\""),
            ("<metricsWithoutAuth>false", "<metricsWithoutAuth>true"),
            (
                "<apikey>",
                "<insecureAdminAccess>true</insecureAdminAccess><apikey>",
            ),
        ] {
            assert!(text.contains(from));
            assert!(inspect(text.replace(from, to).as_bytes(), ports(), false).is_err());
        }
    }

    #[test]
    fn zero_colliding_ports_and_deep_xml_refuse() {
        assert!(NativePorts::new(0, 1).is_err());
        assert!(NativePorts::new(2, 2).is_err());
        let deep = format!("{}{}", "<n>".repeat(17), "</n>".repeat(17));
        assert!(parse(deep.as_bytes()).is_err());
    }

    #[test]
    fn unicode_escaped_attributes_roundtrip_but_controls_and_key_rewriting_refuse() {
        let raw =
            synthetic_xml().replace("<device id=", "<device name=\"Sentetik &amp; Türkçe\" id=");
        let prepared = inspect(raw.as_bytes(), ports(), true).unwrap();
        assert_eq!(
            inspect(&prepared.bytes, ports(), false).unwrap().bytes,
            prepared.bytes
        );
        let control = raw.replace("Sentetik &amp; Türkçe", "invalid&#0;attribute");
        assert!(inspect(control.as_bytes(), ports(), true).is_err());
        let control = raw.replace("Sentetik &amp; Türkçe", "invalid\u{ffff}attribute");
        assert!(inspect(control.as_bytes(), ports(), true).is_err());
        let changed = raw.replace("<apikey>", "<apikey> ");
        assert!(inspect(changed.as_bytes(), ports(), true).is_err());
    }
}
