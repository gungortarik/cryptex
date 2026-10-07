//! Daemon-owned OS secure-store facade (ADR-0014 ACCEPTED — P1).
//!
//! Only `cryptexd` may hold device private material through this module.
//! Errors are typed and redacted: Display/Debug must never recover secret bytes.

mod fake;
mod naming;

#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

pub use fake::FakeSecureStore;
pub use naming::{AccountLabel, SecureStoreEnvironment, ServiceLabel};

#[cfg(windows)]
pub use windows::WindowsCredentialStore;

#[cfg(target_os = "macos")]
pub use macos::MacosKeychainStore;

use std::fmt;

/// Typed secure-store failures. Variants must not carry secret payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecureStoreError {
    /// No matching item exists for the requested service/account.
    Missing,
    /// Service or account naming rejected before any platform call.
    InvalidName(String),
    /// Sanitized platform failure: operation label + numeric status only.
    Platform { op: &'static str, code: i64 },
}

impl fmt::Display for SecureStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => write!(f, "secure_store_missing_item"),
            Self::InvalidName(reason) => write!(f, "secure_store_invalid_name: {reason}"),
            Self::Platform { op, code } => {
                write!(f, "secure_store_platform_error op={op} code={code}")
            }
        }
    }
}

impl std::error::Error for SecureStoreError {}

pub type SecureStoreResult<T> = Result<T, SecureStoreError>;

/// Daemon-authority secret storage. Implementations must never log secret bytes.
pub trait SecureStore: Send + Sync {
    fn put(&self, service: &str, account: &str, secret: &[u8]) -> SecureStoreResult<()>;
    fn get(&self, service: &str, account: &str) -> SecureStoreResult<Vec<u8>>;
    fn delete(&self, service: &str, account: &str) -> SecureStoreResult<()>;
}

/// True when `text` lets an observer recover `secret` (UTF-8 and common Debug/hex forms).
pub fn payload_bytes_recoverable(secret: &[u8], text: &str) -> bool {
    if secret.is_empty() {
        return false;
    }
    if let Ok(utf8) = std::str::from_utf8(secret)
        && !utf8.is_empty()
        && text.contains(utf8)
    {
        return true;
    }
    let spaced: String = secret
        .iter()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    if text.contains(&spaced) {
        return true;
    }
    let compact: String = secret
        .iter()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(",");
    if text.contains(&compact) {
        return true;
    }
    if secret.len() >= 4 {
        let hex_lower: String = secret.iter().map(|b| format!("{b:02x}")).collect();
        if text.to_ascii_lowercase().contains(&hex_lower) {
            return true;
        }
    }
    false
}

/// Apple `errSecItemNotFound`.
pub const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

/// Map a Keychain OSStatus: only item-not-found becomes [`SecureStoreError::Missing`].
pub fn classify_keychain_status(op: &'static str, status: i32) -> SecureStoreError {
    if status == ERR_SEC_ITEM_NOT_FOUND {
        SecureStoreError::Missing
    } else {
        SecureStoreError::Platform {
            op,
            code: i64::from(status),
        }
    }
}

pub(crate) fn validate_name_pair(service: &str, account: &str) -> SecureStoreResult<()> {
    if service.is_empty() {
        return Err(SecureStoreError::InvalidName("empty_service".into()));
    }
    if account.is_empty() {
        return Err(SecureStoreError::InvalidName("empty_account".into()));
    }
    if service.contains('\0') || account.contains('\0') {
        return Err(SecureStoreError::InvalidName("nul_in_name".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_surfaces_do_not_embed_secret_bytes() {
        let secret = b"SYNTHETIC_DAEMON_SECRET_NOT_REAL::unit::v1";
        let errors = [
            SecureStoreError::Missing,
            SecureStoreError::InvalidName("empty_service".into()),
            SecureStoreError::Platform {
                op: "CredReadW",
                code: 5,
            },
        ];
        for err in errors {
            let display = format!("{err}");
            let debug = format!("{err:?}");
            assert!(
                !payload_bytes_recoverable(secret, &display),
                "Display leaked for {err}"
            );
            assert!(
                !payload_bytes_recoverable(secret, &debug),
                "Debug leaked for {debug}"
            );
        }
    }

    #[test]
    fn item_not_found_maps_to_missing() {
        assert_eq!(
            classify_keychain_status("generic_password", ERR_SEC_ITEM_NOT_FOUND),
            SecureStoreError::Missing
        );
    }

    #[test]
    fn auth_failed_stays_platform() {
        // errSecAuthFailed
        assert_eq!(
            classify_keychain_status("generic_password", -25293),
            SecureStoreError::Platform {
                op: "generic_password",
                code: -25293
            }
        );
    }

    #[test]
    fn payload_detector_flags_debug_decimal_lists() {
        let secret = b"ABC";
        assert!(payload_bytes_recoverable(secret, &format!("{secret:?}")));
    }
}
