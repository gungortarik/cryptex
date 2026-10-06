use serde::{Deserialize, Deserializer, Serialize};

use crate::error::{CoreError, CoreResult};

/// Opaque install / local-instance identity (ADR-0015 §2.2).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct InstallId(String);

/// Opaque trusted-device principal identity (ADR-0015 §2.2).
///
/// Logical IDs remain case-sensitive. On-disk sequence filenames use a
/// deterministic lowercase-hex encoding of the UTF-8 ID bytes so values that
/// differ only by case (e.g. `Device-A` vs `device-a`) cannot collide on
/// case-insensitive Windows/macOS filesystems (ADR-0015 path-safety).
///
/// `Deserialize` routes through the same checks as [`DeviceId::new`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct DeviceId(String);

impl InstallId {
    pub fn new(value: impl Into<String>) -> CoreResult<Self> {
        parse_path_safe_id(value.into(), "install_id").map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl DeviceId {
    pub fn new(value: impl Into<String>) -> CoreResult<Self> {
        parse_path_safe_id(value.into(), "device_id").map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Collision-safe filename stem for the sequence store (`{stem}.json` / `.lock`).
    ///
    /// Encodes UTF-8 bytes as lowercase hex so case-distinct IDs never share a
    /// path on case-insensitive volumes.
    pub fn as_filename_stem(&self) -> String {
        encode_filename_stem(&self.0)
    }
}

impl<'de> Deserialize<'de> for InstallId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        InstallId::new(raw).map_err(serde::de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for DeviceId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        DeviceId::new(raw).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for InstallId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn encode_filename_stem(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 2);
    for byte in value.as_bytes() {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn parse_path_safe_id(value: String, label: &str) -> CoreResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CoreError::InvalidId(format!("{label} must be non-empty")));
    }
    if trimmed.len() > 64 {
        return Err(CoreError::InvalidId(format!(
            "{label} exceeds maximum length 64"
        )));
    }
    if trimmed != value {
        return Err(CoreError::InvalidId(format!(
            "{label} must not have leading or trailing whitespace"
        )));
    }
    if trimmed.ends_with('.') || trimmed.ends_with(' ') {
        return Err(CoreError::InvalidId(format!(
            "{label} must not end with '.' or space (Windows filename rule)"
        )));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(CoreError::InvalidId(format!(
            "{label} must be ASCII alphanumeric, hyphen, or underscore only"
        )));
    }
    if is_windows_reserved_stem(trimmed) {
        return Err(CoreError::InvalidId(format!(
            "{label} collides with a Windows reserved device name"
        )));
    }
    Ok(trimmed.to_string())
}

fn is_windows_reserved_stem(value: &str) -> bool {
    let stem = value.split_once('.').map(|(left, _)| left).unwrap_or(value);
    let upper = stem.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence::DeviceSequenceCutoff;
    use serde::Deserialize;

    #[test]
    fn rejects_empty_and_path_unsafe_ids() {
        assert!(InstallId::new("").is_err());
        assert!(DeviceId::new("a/b").is_err());
        assert!(DeviceId::new("..hidden").is_err());
        assert!(DeviceId::new("dev-1").is_ok());
        assert!(DeviceId::new("CON").is_err());
        assert!(DeviceId::new("com1").is_err());
        assert!(DeviceId::new("good.bad").is_err());
        assert!(DeviceId::new("trailing.").is_err());
        assert!(DeviceId::new("has space").is_err());
        assert!(DeviceId::new("ok_Name-123").is_ok());
    }

    #[test]
    fn filename_stem_is_case_collision_safe() {
        let upper = DeviceId::new("Device-A").unwrap();
        let lower = DeviceId::new("device-a").unwrap();
        assert_ne!(upper.as_str(), lower.as_str());
        assert_ne!(
            upper.as_filename_stem(),
            lower.as_filename_stem(),
            "case-distinct DeviceIds must not share a filename stem"
        );
        assert!(
            upper
                .as_filename_stem()
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        assert_eq!(upper.as_filename_stem(), encode_filename_stem("Device-A"));
        assert_eq!(lower.as_filename_stem(), encode_filename_stem("device-a"));
    }

    #[test]
    fn deserialize_rejects_unsafe_device_and_install_ids() {
        let bad_device = serde_json::from_str::<DeviceId>(r#""../etc-passwd""#);
        assert!(bad_device.is_err(), "path traversal must not deserialize");

        let bad_dots = serde_json::from_str::<DeviceId>(r#""..hidden""#);
        assert!(bad_dots.is_err());

        let reserved = serde_json::from_str::<DeviceId>(r#""CON""#);
        assert!(reserved.is_err());

        let slash = serde_json::from_str::<InstallId>(r#""a/b""#);
        assert!(slash.is_err());

        let good: DeviceId = serde_json::from_str(r#""device-safe-1""#).unwrap();
        assert_eq!(good.as_str(), "device-safe-1");
        assert_eq!(
            good.as_filename_stem(),
            encode_filename_stem("device-safe-1")
        );
    }

    #[test]
    fn nested_cutoff_deserialize_rejects_unsafe_device_id() {
        #[derive(Debug, Deserialize)]
        struct Wrap {
            cutoff: DeviceSequenceCutoff,
        }

        let err = serde_json::from_str::<Wrap>(
            r#"{
  "cutoff": {
    "device_id": "../sneaky",
    "cutoff": 7
  }
}"#,
        );
        assert!(
            err.is_err(),
            "nested DeviceSequenceCutoff must not accept unsafe device_id"
        );

        let ok: Wrap = serde_json::from_str(
            r#"{
  "cutoff": {
    "device_id": "device-nested-1",
    "cutoff": 7
  }
}"#,
        )
        .unwrap();
        assert_eq!(ok.cutoff.device_id.as_str(), "device-nested-1");
        assert_eq!(ok.cutoff.cutoff, 7);
    }
}
