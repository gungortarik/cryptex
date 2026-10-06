//! macOS Keychain P1 adapter (ADR-0014).
//!
//! Gate A spike verified these APIs. Gate C product physical verification on a
//! Mac host remains a separate M1 completion requirement.

use super::{validate_name_pair, SecureStore, SecureStoreError, SecureStoreResult};
use crate::secure_store::classify_keychain_status;
use security_framework::base::Error as KeychainError;
use security_framework::passwords::{
    delete_generic_password, generic_password, set_generic_password, PasswordOptions,
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
        payload_bytes_recoverable, AccountLabel, SecureStoreEnvironment, ServiceLabel,
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

    #[test]
    fn macos_delete_never_written_account_is_typed_missing() {
        let store = MacosKeychainStore::new();
        let service = ServiceLabel::device_credentials(SecureStoreEnvironment::Dev)
            .as_str()
            .to_string();
        let account = AccountLabel::device_private(&format!(
            "mac-never-written-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
        .as_str()
        .to_string();

        match store.delete(&service, &account) {
            Err(SecureStoreError::Missing) => {}
            other => panic!("expected Missing on delete of never-written account, got {other:?}"),
        }
        match store.get(&service, &account) {
            Err(SecureStoreError::Missing) => {}
            other => panic!("expected Missing on get of never-written account, got {other:?}"),
        }

        let err = store.delete(&service, &account).unwrap_err();
        let probe = b"SYNTHETIC_MAC_NEVER_WRITTEN_PROBE_NOT_REAL";
        assert!(!payload_bytes_recoverable(probe, &format!("{err}")));
        assert!(!payload_bytes_recoverable(probe, &format!("{err:?}")));
    }
}
