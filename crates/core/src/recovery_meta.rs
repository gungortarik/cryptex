use serde::{Deserialize, Serialize};

/// Schema version for Recovery Kit **public** metadata (ADR-0015 §2.5).
pub const RECOVERY_KIT_PUBLIC_SCHEMA_VERSION: u32 = 1;

/// Declared Recovery Kit purposes — not embedded secrets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryPurpose {
    BackupDecrypt,
    TrustBootstrap,
}

/// Public, non-secret Recovery Kit metadata. Offline signing secrets must not appear here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryKitPublicMetadata {
    pub schema_version: u32,
    pub kit_id: String,
    pub brain_id: String,
    pub purposes: Vec<RecoveryPurpose>,
}

impl RecoveryKitPublicMetadata {
    pub fn new(
        kit_id: impl Into<String>,
        brain_id: impl Into<String>,
        purposes: Vec<RecoveryPurpose>,
    ) -> Self {
        Self {
            schema_version: RECOVERY_KIT_PUBLIC_SCHEMA_VERSION,
            kit_id: kit_id.into(),
            brain_id: brain_id.into(),
            purposes,
        }
    }
}
