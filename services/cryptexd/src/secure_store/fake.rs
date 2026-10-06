//! In-memory SecureStore for unit tests (no OS prompts).

use std::collections::HashMap;
use std::sync::Mutex;

use super::{SecureStore, SecureStoreError, SecureStoreResult, validate_name_pair};

/// Process-local fake store. Secrets are zeroed on overwrite/delete/drop of entries.
#[derive(Default)]
pub struct FakeSecureStore {
    inner: Mutex<HashMap<(String, String), Vec<u8>>>,
}

impl FakeSecureStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.inner.lock().expect("fake store lock").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn zeroize(buf: &mut [u8]) {
    for b in buf {
        *b = 0;
    }
}

impl SecureStore for FakeSecureStore {
    fn put(&self, service: &str, account: &str, secret: &[u8]) -> SecureStoreResult<()> {
        validate_name_pair(service, account)?;
        let mut map = self.inner.lock().expect("fake store lock");
        let key = (service.to_string(), account.to_string());
        if let Some(mut old) = map.remove(&key) {
            zeroize(&mut old);
        }
        map.insert(key, secret.to_vec());
        Ok(())
    }

    fn get(&self, service: &str, account: &str) -> SecureStoreResult<Vec<u8>> {
        validate_name_pair(service, account)?;
        let map = self.inner.lock().expect("fake store lock");
        map.get(&(service.to_string(), account.to_string()))
            .cloned()
            .ok_or(SecureStoreError::Missing)
    }

    fn delete(&self, service: &str, account: &str) -> SecureStoreResult<()> {
        validate_name_pair(service, account)?;
        let mut map = self.inner.lock().expect("fake store lock");
        match map.remove(&(service.to_string(), account.to_string())) {
            Some(mut old) => {
                zeroize(&mut old);
                Ok(())
            }
            None => Err(SecureStoreError::Missing),
        }
    }
}

impl Drop for FakeSecureStore {
    fn drop(&mut self) {
        if let Ok(mut map) = self.inner.lock() {
            for value in map.values_mut() {
                zeroize(value);
            }
            map.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secure_store::{
        AccountLabel, SecureStoreEnvironment, ServiceLabel, payload_bytes_recoverable,
    };

    #[test]
    fn put_get_delete_missing_matrix() {
        let store = FakeSecureStore::new();
        let env = SecureStoreEnvironment::Dev;
        let service = ServiceLabel::device_credentials(env).as_str().to_string();
        let account = AccountLabel::device_private("dev-device-aaa")
            .as_str()
            .to_string();
        let secret = b"SYNTHETIC_FAKE_STORE_SECRET_NOT_REAL::v1".to_vec();

        store.put(&service, &account, &secret).expect("put");
        let got = store.get(&service, &account).expect("get");
        assert_eq!(got, secret);

        store.delete(&service, &account).expect("delete");
        match store.get(&service, &account) {
            Err(SecureStoreError::Missing) => {}
            other => panic!("expected Missing, got {other:?}"),
        }
        assert!(store.is_empty());
    }

    #[test]
    fn rejects_empty_names_before_storage() {
        let store = FakeSecureStore::new();
        assert!(matches!(
            store.put("", "acct", b"x"),
            Err(SecureStoreError::InvalidName(_))
        ));
        assert!(matches!(
            store.get("svc", ""),
            Err(SecureStoreError::InvalidName(_))
        ));
    }

    #[test]
    fn missing_delete_is_typed() {
        let store = FakeSecureStore::new();
        assert_eq!(
            store.delete("Cryptex-Dev", "synthetic-missing"),
            Err(SecureStoreError::Missing)
        );
    }

    #[test]
    fn operation_errors_do_not_echo_secret() {
        let store = FakeSecureStore::new();
        let secret = b"SYNTHETIC_FAKE_STORE_SECRET_NOT_REAL::redaction";
        let err = store.get("Cryptex-Dev", "missing-account").unwrap_err();
        let display = format!("{err}");
        let debug = format!("{err:?}");
        assert!(!payload_bytes_recoverable(secret, &display));
        assert!(!payload_bytes_recoverable(secret, &debug));
    }
}
