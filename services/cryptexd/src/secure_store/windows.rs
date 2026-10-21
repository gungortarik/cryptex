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

/// Copy credential blob bytes, rejecting inconsistent CredRead results.
///
/// `CredentialBlobSize > 0` with a null `CredentialBlob` is treated as a platform
/// integrity failure (fail closed). Empty secrets (`size == 0`) are allowed.
pub(crate) fn copy_credential_blob(size: u32, blob: *const u8) -> SecureStoreResult<Vec<u8>> {
    let len = size as usize;
    if len == 0 {
        return Ok(Vec::new());
    }
    if blob.is_null() {
        return Err(SecureStoreError::Platform {
            op: "CredReadW_inconsistent_blob",
            code: -3,
        });
    }
    let mut buf = vec![0u8; len];
    unsafe {
        std::ptr::copy_nonoverlapping(blob, buf.as_mut_ptr(), len);
    }
    Ok(buf)
}

/// RAII guard: always `CredFree` a non-null credential pointer on drop.
struct CredReadGuard(*mut CREDENTIALW);

impl CredReadGuard {
    fn new(ptr: *mut CREDENTIALW) -> Self {
        Self(ptr)
    }

    fn as_ptr(&self) -> *mut CREDENTIALW {
        self.0
    }
}

impl Drop for CredReadGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CredFree(self.0 as *const _);
            }
            self.0 = std::ptr::null_mut();
        }
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
            // CredReadW failed: do not CredFree (pointer is not owned / null).
            return Err(map_last_error("CredReadW"));
        }
        // Success path: own the pointer and free on every return (incl. errors).
        let guard = CredReadGuard::new(cred_ptr);
        if guard.as_ptr().is_null() {
            return Err(SecureStoreError::Platform {
                op: "CredReadW_null_credential",
                code: -2,
            });
        }
        let (size, blob) = unsafe {
            let cred = &*guard.as_ptr();
            (cred.CredentialBlobSize, cred.CredentialBlob as *const u8)
        };
        copy_credential_blob(size, blob)
        // CredFree via CredReadGuard Drop on all paths.
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
    fn windows_delete_never_written_account_is_typed_missing() {
        let store = WindowsCredentialStore::new();
        let service = ServiceLabel::device_credentials(SecureStoreEnvironment::Dev)
            .as_str()
            .to_string();
        // Unique account that this test never puts, so CredDeleteW must miss.
        let account = AccountLabel::device_private(&format!(
            "win-never-written-{}-{}",
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
        let display = format!("{err}");
        let debug = format!("{err:?}");
        let probe = b"SYNTHETIC_WIN_NEVER_WRITTEN_PROBE_NOT_REAL";
        assert!(!payload_bytes_recoverable(probe, &display));
        assert!(!payload_bytes_recoverable(probe, &debug));
    }

    #[test]
    fn map_not_found_is_missing_not_platform() {
        // ERROR_NOT_FOUND is 1168 on Windows.
        assert_eq!(ERROR_NOT_FOUND, 1168);
    }

    #[test]
    fn copy_credential_blob_rejects_size_with_null_pointer() {
        let err = copy_credential_blob(4, std::ptr::null()).unwrap_err();
        assert!(matches!(
            err,
            SecureStoreError::Platform {
                op: "CredReadW_inconsistent_blob",
                code: -3
            }
        ));
    }

    #[test]
    fn copy_credential_blob_allows_empty_and_copies_bytes() {
        assert_eq!(
            copy_credential_blob(0, std::ptr::null()).unwrap(),
            Vec::<u8>::new()
        );
        let data = [1u8, 2, 3];
        assert_eq!(
            copy_credential_blob(3, data.as_ptr()).unwrap(),
            vec![1, 2, 3]
        );
    }
}
