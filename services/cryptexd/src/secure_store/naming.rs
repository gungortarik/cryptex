//! Cryptex vs Cryptex-Dev service/account labeling (no secret bytes in names).

use std::fmt;

/// Runtime namespace for secure-store item labels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecureStoreEnvironment {
    /// Production Brain credentials.
    Prod,
    /// Development / tests / spikes.
    Dev,
}

impl SecureStoreEnvironment {
    pub fn service_prefix(self) -> &'static str {
        match self {
            Self::Prod => "Cryptex",
            Self::Dev => "Cryptex-Dev",
        }
    }
}

/// OS credential service label. Never embed secret material.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceLabel(String);

impl ServiceLabel {
    pub fn device_credentials(env: SecureStoreEnvironment) -> Self {
        Self(format!("{}-DeviceCredentials", env.service_prefix()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ServiceLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// OS credential account label keyed by public device id (not secret bytes).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountLabel(String);

impl AccountLabel {
    pub fn device_private(device_id: &str) -> Self {
        Self(format!("device-private:{device_id}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AccountLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prod_and_dev_service_labels_differ() {
        let prod = ServiceLabel::device_credentials(SecureStoreEnvironment::Prod);
        let dev = ServiceLabel::device_credentials(SecureStoreEnvironment::Dev);
        assert_eq!(prod.as_str(), "Cryptex-DeviceCredentials");
        assert_eq!(dev.as_str(), "Cryptex-Dev-DeviceCredentials");
        assert_ne!(prod, dev);
    }

    #[test]
    fn account_label_uses_public_device_id_only() {
        let label = AccountLabel::device_private("abc123");
        assert_eq!(label.as_str(), "device-private:abc123");
        assert!(!label.as_str().contains("SECRET"));
    }
}
