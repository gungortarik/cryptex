//! macOS Keychain P1 adapter (ADR-0014).
//!
//! Gate A spike verified these APIs. Gate C product physical verification on a
//! Mac host remains a separate M1 completion requirement.

use super::{SecureStore, SecureStoreError, SecureStoreResult, validate_name_pair};
use crate::secure_store::classify_keychain_status;
use security_framework::base::Error as KeychainError;
use security_framework::passwords::{
    PasswordOptions, delete_generic_password, generic_password, set_generic_password,
};

fn map_keychain_err(op: &'static str, err: KeychainError) -> SecureStoreError {
    classify_keychain_status(op, err.code())
}

/// Direct Security Framework generic-password adapter. Daemon-only.
#[derive(Debug, Default, Clone, Copy)]
pub struct MacosKeychainStore;

impl MacosKeychainStore {
    pub fn new() -> Self {
        Self
    }
}

impl SecureStore for MacosKeychainStore {
    fn put(&self, service: &str, account: &str, secret: &[u8]) -> SecureStoreResult<()> {
        validate_name_pair(service, account)?;
        set_generic_password(service, account, secret)
            .map_err(|e| map_keychain_err("set_generic_password", e))
    }

    fn get(&self, service: &str, account: &str) -> SecureStoreResult<Vec<u8>> {
        validate_name_pair(service, account)?;
        generic_password(PasswordOptions::new_generic_password(service, account))
            .map_err(|e| map_keychain_err("generic_password", e))
    }

    fn delete(&self, service: &str, account: &str) -> SecureStoreResult<()> {
        validate_name_pair(service, account)?;
        delete_generic_password(service, account)
            .map_err(|e| map_keychain_err("delete_generic_password", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secure_store::{
        AccountLabel, SecureStoreEnvironment, ServiceLabel, payload_bytes_recoverable,
    };

    #[test]
    fn macos_store_read_delete_missing_redaction_matrix() {
        let store = MacosKeychainStore::new();
        let service = ServiceLabel::device_credentials(SecureStoreEnvironment::Dev)
            .as_str()
            .to_string();
        let account = AccountLabel::device_private(&format!("mac-adapter-{}", std::process::id()))
            .as_str()
            .to_string();
        let secret = format!("SYNTHETIC_MAC_ADAPTER_SECRET_NOT_REAL::{account}").into_bytes();

        let _ = store.delete(&service, &account);

        store.put(&service, &account, &secret).expect("put");
        let got = store.get(&service, &account).expect("get");
        assert_eq!(got, secret);

        store.delete(&service, &account).expect("delete");
        match store.get(&service, &account) {
            Err(SecureStoreError::Missing) => {}
            other => panic!("expected Missing after delete, got {other:?}"),
        }

        let err = store.get(&service, &account).unwrap_err();
        assert!(!payload_bytes_recoverable(&secret, &format!("{err}")));
        assert!(!payload_bytes_recoverable(&secret, &format!("{err:?}")));

        let _ = store.delete(&service, &account);
    }
}
