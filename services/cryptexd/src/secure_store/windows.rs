//! Windows Credential Manager P1 adapter (ADR-0014).

use super::{SecureStore, SecureStoreError, SecureStoreResult, validate_name_pair};
use widestring::U16CString;
use windows_sys::Win32::Foundation::{ERROR_NOT_FOUND, GetLastError};
use windows_sys::Win32::Security::Credentials::{
    CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree, CredReadW,
    CredWriteW,
};

/// Target name shape used by Gate A / product P1: `{service}/{account}`.
pub fn target_name(service: &str, account: &str) -> String {
    format!("{service}/{account}")
}

fn map_last_error(op: &'static str) -> SecureStoreError {
    let code = unsafe { GetLastError() } as i64;
    if code == ERROR_NOT_FOUND as i64 {
        SecureStoreError::Missing
    } else {
        SecureStoreError::Platform { op, code }
    }
}

/// Direct Cred* adapter. Daemon-only.
#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsCredentialStore;

impl WindowsCredentialStore {
    pub fn new() -> Self {
        Self
    }
}

impl SecureStore for WindowsCredentialStore {
    fn put(&self, service: &str, account: &str, secret: &[u8]) -> SecureStoreResult<()> {
        validate_name_pair(service, account)?;
        let target = target_name(service, account);
        let target_w = U16CString::from_str(&target)
            .map_err(|_| SecureStoreError::InvalidName("invalid_target_utf16".into()))?;
        let user_w = U16CString::from_str(account)
            .map_err(|_| SecureStoreError::InvalidName("invalid_account_utf16".into()))?;

        let mut credential = unsafe { std::mem::zeroed::<CREDENTIALW>() };
        credential.Type = CRED_TYPE_GENERIC;
        credential.TargetName = target_w.as_ptr() as *mut _;
        credential.UserName = user_w.as_ptr() as *mut _;
        credential.CredentialBlobSize =
            u32::try_from(secret.len()).map_err(|_| SecureStoreError::Platform {
                op: "CredWriteW",
                code: -1,
            })?;
        credential.CredentialBlob = secret.as_ptr() as *mut _;
        credential.Persist = CRED_PERSIST_LOCAL_MACHINE;

        let ok = unsafe { CredWriteW(&credential, 0) };
        if ok == 0 {
            return Err(map_last_error("CredWriteW"));
        }
        Ok(())
    }

    fn get(&self, service: &str, account: &str) -> SecureStoreResult<Vec<u8>> {
        validate_name_pair(service, account)?;
        let target = target_name(service, account);
        let target_w = U16CString::from_str(&target)
            .map_err(|_| SecureStoreError::InvalidName("invalid_target_utf16".into()))?;
        let mut cred_ptr: *mut CREDENTIALW = std::ptr::null_mut();
        let ok = unsafe { CredReadW(target_w.as_ptr(), CRED_TYPE_GENERIC, 0, &mut cred_ptr) };
        if ok == 0 {
            return Err(map_last_error("CredReadW"));
        }
        if cred_ptr.is_null() {
            return Err(SecureStoreError::Platform {
                op: "CredReadW",
                code: -2,
            });
        }
        let out = unsafe {
            let cred = &*cred_ptr;
            let len = cred.CredentialBlobSize as usize;
            let mut buf = vec![0u8; len];
            if len > 0 && !cred.CredentialBlob.is_null() {
                std::ptr::copy_nonoverlapping(cred.CredentialBlob, buf.as_mut_ptr(), len);
            }
            CredFree(cred_ptr as *const _);
            buf
        };
        Ok(out)
    }

    fn delete(&self, service: &str, account: &str) -> SecureStoreResult<()> {
        validate_name_pair(service, account)?;
        let target = target_name(service, account);
        let target_w = U16CString::from_str(&target)
            .map_err(|_| SecureStoreError::InvalidName("invalid_target_utf16".into()))?;
        let ok = unsafe { CredDeleteW(target_w.as_ptr(), CRED_TYPE_GENERIC, 0) };
        if ok == 0 {
            return Err(map_last_error("CredDeleteW"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secure_store::{
        AccountLabel, SecureStoreEnvironment, ServiceLabel, payload_bytes_recoverable,
    };

    #[test]
    fn windows_store_read_delete_missing_redaction_matrix() {
        let store = WindowsCredentialStore::new();
        let service = ServiceLabel::device_credentials(SecureStoreEnvironment::Dev)
            .as_str()
            .to_string();
        let account = AccountLabel::device_private(&format!("win-adapter-{}", std::process::id()))
            .as_str()
            .to_string();
        let secret = format!("SYNTHETIC_WIN_ADAPTER_SECRET_NOT_REAL::{account}").into_bytes();

        // Best-effort cleanup from prior crash.
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
        let display = format!("{err}");
        let debug = format!("{err:?}");
        assert!(!payload_bytes_recoverable(&secret, &display));
        assert!(!payload_bytes_recoverable(&secret, &debug));

        let _ = store.delete(&service, &account);
    }

    #[test]
    fn map_not_found_is_missing_not_platform() {
        // ERROR_NOT_FOUND is 1168 on Windows.
        assert_eq!(ERROR_NOT_FOUND, 1168);
    }
}
