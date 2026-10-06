//! Platform status classification for spike measurement accuracy.
//!
//! Keychain item-not-found must not be conflated with access-denied or other failures.

/// Apple Security Framework `errSecItemNotFound`.
pub const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

/// Sanitized classification of a Keychain OSStatus for P1 adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KeychainClassified {
    Missing,
    /// Numeric status only — never attach secret payloads or raw platform strings
    /// that might embed credential material.
    Platform {
        op: &'static str,
        status: i32,
    },
}

impl KeychainClassified {
    pub fn code_label(&self) -> String {
        match self {
            KeychainClassified::Missing => "secure_store_missing_item".to_string(),
            KeychainClassified::Platform { op, status } => {
                format!("secure_store_platform_error code={op}_status={status}")
            }
        }
    }
}

/// Map a Keychain OSStatus to Missing only for the real item-not-found code.
pub fn classify_keychain_status(op: &'static str, status: i32) -> KeychainClassified {
    if status == ERR_SEC_ITEM_NOT_FOUND {
        KeychainClassified::Missing
    } else {
        KeychainClassified::Platform { op, status }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_not_found_maps_to_missing() {
        assert_eq!(
            classify_keychain_status("find_generic_password", ERR_SEC_ITEM_NOT_FOUND),
            KeychainClassified::Missing
        );
        assert_eq!(
            classify_keychain_status("delete_generic_password", -25300),
            KeychainClassified::Missing
        );
    }

    #[test]
    fn auth_failed_is_platform_not_missing() {
        // errSecAuthFailed
        let c = classify_keychain_status("find_generic_password", -25293);
        assert!(matches!(
            c,
            KeychainClassified::Platform { status: -25293, .. }
        ));
        assert!(!matches!(c, KeychainClassified::Missing));
    }

    #[test]
    fn interaction_not_allowed_is_platform_not_missing() {
        // errSecInteractionNotAllowed
        let c = classify_keychain_status("find_generic_password", -25308);
        assert!(matches!(c, KeychainClassified::Platform { .. }));
    }

    #[test]
    fn missing_entitlement_is_platform_not_missing() {
        // errSecMissingEntitlement
        let c = classify_keychain_status("find_generic_password", -34018);
        assert!(matches!(
            c,
            KeychainClassified::Platform { status: -34018, .. }
        ));
    }

    #[test]
    fn platform_label_is_sanitized_numeric() {
        let c = classify_keychain_status("delete_generic_password", -25293);
        let label = c.code_label();
        assert!(label.contains("-25293"));
        // Op names may contain the word "password"; payloads must not appear.
        assert!(!label.contains("SYNTHETIC"));
        assert!(!label.contains("CredentialBlob"));
        assert!(!label.contains('['));
    }
}
