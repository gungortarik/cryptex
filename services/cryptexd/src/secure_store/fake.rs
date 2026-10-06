//! In-memory SecureStore for unit tests (no OS prompts).

use std::collections::HashMap;
use std::sync::Mutex;

use super::{SecureStore, SecureStoreError, SecureStoreResult, validate_name_pair};

/// Process-local fake store. Secrets are zeroed on overwrite/delete/drop of entries.
#[derive(Default)]
pub struct FakeSecureStore {
    inner: Mutex<HashMap<(String, String), Vec<u8>>>,
    /// When set, the next `put` returns this error and clears the latch.
    fail_next_put: Mutex<Option<SecureStoreError>>,
    /// When set, the next `delete` returns this error and clears the latch.
    fail_next_delete: Mutex<Option<SecureStoreError>>,
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

    /// Arm a one-shot put failure for injection tests.
    pub fn fail_next_put_with(&self, err: SecureStoreError) {
        *self.fail_next_put.lock().expect("fail latch") = Some(err);
    }

    /// Arm a one-shot delete failure for injection tests.
    pub fn fail_next_delete_with(&self, err: SecureStoreError) {
        *self.fail_next_delete.lock().expect("fail latch") = Some(err);
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
        if let Some(err) = self.fail_next_put.lock().expect("fail latch").take() {
            return Err(err);
        }
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
        if let Some(err) = self.fail_next_delete.lock().expect("fail latch").take() {
            return Err(err);
        }
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

    #[test]
    fn fail_next_put_is_one_shot() {
        let store = FakeSecureStore::new();
        store.fail_next_put_with(SecureStoreError::Platform {
            op: "injected",
            code: 7,
        });
        assert!(matches!(
            store.put("Cryptex-Dev", "acct", b"x"),
            Err(SecureStoreError::Platform {
                op: "injected",
                code: 7
            })
        ));
        store
            .put("Cryptex-Dev", "acct", b"x")
            .expect("second put ok");
    }

    #[test]
    fn fail_next_delete_is_one_shot() {
        let store = FakeSecureStore::new();
        store.put("Cryptex-Dev", "acct", b"x").expect("put");
        store.fail_next_delete_with(SecureStoreError::Platform {
            op: "injected_delete",
            code: 11,
        });
        assert!(matches!(
            store.delete("Cryptex-Dev", "acct"),
            Err(SecureStoreError::Platform {
                op: "injected_delete",
                code: 11
            })
        ));
        store
            .delete("Cryptex-Dev", "acct")
            .expect("second delete ok");
    }
}
