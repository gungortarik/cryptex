//! P1: direct platform APIs (no keyring facade).

use crate::matrix::{
    CandidateId, CellStatus, MatrixCell, MatrixReport, Platform, SyntheticItem, secrets_match,
};

pub fn run(platform: Platform) -> Result<MatrixReport, String> {
    match platform {
        Platform::Windows => run_windows(),
        Platform::Macos => run_macos(),
        Platform::Unsupported => Ok(unsupported()),
    }
}

fn unsupported() -> MatrixReport {
    MatrixReport {
        candidate: CandidateId::P1,
        platform: Platform::Unsupported,
        provider_note: "no first-class provider on this host".to_string(),
        cells: waiting_all("unsupported OS for Cryptex V1 spike"),
    }
}

fn waiting_all(note: &str) -> Vec<MatrixCell> {
    ["store", "read", "delete", "missing-item", "error-redaction"]
        .into_iter()
        .map(|name| MatrixCell {
            name,
            status: CellStatus::Waiting,
            note: note.to_string(),
        })
        .collect()
}

#[derive(Debug)]
enum P1Error {
    Missing,
    Platform(String),
}

impl std::fmt::Display for P1Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            P1Error::Missing => write!(f, "secure_store_missing_item"),
            P1Error::Platform(code) => write!(f, "secure_store_platform_error code={code}"),
        }
    }
}

#[cfg(windows)]
mod win {
    use super::P1Error;
    use widestring::U16CString;
    use windows_sys::Win32::Foundation::{ERROR_NOT_FOUND, GetLastError};
    use windows_sys::Win32::Security::Credentials::{
        CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree,
        CredReadW, CredWriteW,
    };

    pub fn target_name(service: &str, account: &str) -> String {
        format!("{service}/{account}")
    }

    pub fn store(target: &str, account: &str, secret: &[u8]) -> Result<(), P1Error> {
        let target_w =
            U16CString::from_str(target).map_err(|_| P1Error::Platform("invalid_target".into()))?;
        let user_w = U16CString::from_str(account)
            .map_err(|_| P1Error::Platform("invalid_account".into()))?;

        let mut credential = unsafe { std::mem::zeroed::<CREDENTIALW>() };
        credential.Type = CRED_TYPE_GENERIC;
        credential.TargetName = target_w.as_ptr() as *mut _;
        credential.UserName = user_w.as_ptr() as *mut _;
        credential.CredentialBlobSize = secret.len() as u32;
        credential.CredentialBlob = secret.as_ptr() as *mut _;
        credential.Persist = CRED_PERSIST_LOCAL_MACHINE;

        let ok = unsafe { CredWriteW(&credential, 0) };
        if ok == 0 {
            return Err(P1Error::Platform(format!(
                "CredWriteW last_error={}",
                unsafe { GetLastError() }
            )));
        }
        Ok(())
    }

    pub fn read(target: &str) -> Result<Vec<u8>, P1Error> {
        let target_w =
            U16CString::from_str(target).map_err(|_| P1Error::Platform("invalid_target".into()))?;
        let mut cred_ptr: *mut CREDENTIALW = std::ptr::null_mut();
        let ok = unsafe { CredReadW(target_w.as_ptr(), CRED_TYPE_GENERIC, 0, &mut cred_ptr) };
        if ok == 0 {
            let code = unsafe { GetLastError() };
            if code == ERROR_NOT_FOUND {
                return Err(P1Error::Missing);
            }
            return Err(P1Error::Platform(format!("CredReadW last_error={code}")));
        }
        if cred_ptr.is_null() {
            return Err(P1Error::Platform("CredReadW null".into()));
        }
        let result = unsafe {
            let cred = &*cred_ptr;
            let len = cred.CredentialBlobSize as usize;
            let mut out = vec![0u8; len];
            if len > 0 && !cred.CredentialBlob.is_null() {
                std::ptr::copy_nonoverlapping(cred.CredentialBlob, out.as_mut_ptr(), len);
            }
            CredFree(cred_ptr as *const _);
            out
        };
        Ok(result)
    }

    pub fn delete(target: &str) -> Result<(), P1Error> {
        let target_w =
            U16CString::from_str(target).map_err(|_| P1Error::Platform("invalid_target".into()))?;
        let ok = unsafe { CredDeleteW(target_w.as_ptr(), CRED_TYPE_GENERIC, 0) };
        if ok == 0 {
            let code = unsafe { GetLastError() };
            if code == ERROR_NOT_FOUND {
                return Err(P1Error::Missing);
            }
            return Err(P1Error::Platform(format!("CredDeleteW last_error={code}")));
        }
        Ok(())
    }
}

#[cfg(windows)]
fn run_windows() -> Result<MatrixReport, String> {
    let item = SyntheticItem::new(CandidateId::P1);
    let target = win::target_name(&item.service, &item.account);
    let mut cells = Vec::new();

    let _ = win::delete(&target);

    match win::store(&target, &item.account, &item.secret) {
        Ok(()) => cells.push(MatrixCell {
            name: "store",
            status: CellStatus::Pass,
            note: format!("CredWriteW ok ({} bytes)", item.secret_len()),
        }),
        Err(e) => {
            cells.push(MatrixCell {
                name: "store",
                status: CellStatus::Fail,
                note: e.to_string(),
            });
            let _ = win::delete(&target);
            return Ok(report_windows(cells));
        }
    }

    match win::read(&target) {
        Ok(got) => {
            let ok = secrets_match(&got, &item.secret);
            cells.push(MatrixCell {
                name: "read",
                status: if ok {
                    CellStatus::Pass
                } else {
                    CellStatus::Fail
                },
                note: if ok {
                    format!("CredReadW round-trip ok ({} bytes)", got.len())
                } else {
                    "CredReadW round-trip mismatch".to_string()
                },
            });
        }
        Err(e) => cells.push(MatrixCell {
            name: "read",
            status: CellStatus::Fail,
            note: e.to_string(),
        }),
    }

    match win::delete(&target) {
        Ok(()) => cells.push(MatrixCell {
            name: "delete",
            status: CellStatus::Pass,
            note: "CredDeleteW ok".to_string(),
        }),
        Err(e) => cells.push(MatrixCell {
            name: "delete",
            status: CellStatus::Fail,
            note: e.to_string(),
        }),
    }

    match win::read(&target) {
        Err(P1Error::Missing) => cells.push(MatrixCell {
            name: "missing-item",
            status: CellStatus::Pass,
            note: "ERROR_NOT_FOUND after delete".to_string(),
        }),
        Ok(_) => cells.push(MatrixCell {
            name: "missing-item",
            status: CellStatus::Fail,
            note: "unexpected success reading deleted item".to_string(),
        }),
        Err(e) => cells.push(MatrixCell {
            name: "missing-item",
            status: CellStatus::Fail,
            note: format!("expected Missing, got: {e}"),
        }),
    }

    let err = win::read(&target).err().unwrap_or(P1Error::Missing);
    let display = format!("{err}");
    let debug = format!("{err:?}");
    let leaked = item.appears_in(&display) || item.appears_in(&debug);
    cells.push(MatrixCell {
        name: "error-redaction",
        status: if leaked {
            CellStatus::Fail
        } else {
            CellStatus::Pass
        },
        note: if leaked {
            "P1 error Display/Debug contained synthetic secret".to_string()
        } else {
            "typed P1 errors omit secret payloads".to_string()
        },
    });

    let _ = win::delete(&target);
    Ok(report_windows(cells))
}

#[cfg(windows)]
fn report_windows(cells: Vec<MatrixCell>) -> MatrixReport {
    MatrixReport {
        candidate: CandidateId::P1,
        platform: Platform::Windows,
        provider_note: "direct CredWriteW/CredReadW/CredDeleteW via windows-sys=0.61.2; target={service}/{account}; CRED_PERSIST_LOCAL_MACHINE".to_string(),
        cells,
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use super::P1Error;
    use security_framework::passwords::{
        delete_generic_password, find_generic_password, set_generic_password,
    };

    pub fn store(service: &str, account: &str, secret: &[u8]) -> Result<(), P1Error> {
        set_generic_password(service, account, secret)
            .map_err(|_| P1Error::Platform("set_generic_password_failed".into()))
    }

    pub fn read(service: &str, account: &str) -> Result<Vec<u8>, P1Error> {
        match find_generic_password(None, service, account) {
            Ok((data, _)) => Ok(data.to_vec()),
            Err(_) => Err(P1Error::Missing),
        }
    }

    pub fn delete(service: &str, account: &str) -> Result<(), P1Error> {
        match delete_generic_password(service, account) {
            Ok(()) => Ok(()),
            Err(_) => Err(P1Error::Missing),
        }
    }
}

#[cfg(target_os = "macos")]
fn run_macos() -> Result<MatrixReport, String> {
    let item = SyntheticItem::new(CandidateId::P1);
    let mut cells = Vec::new();
    let _ = mac::delete(&item.service, &item.account);

    match mac::store(&item.service, &item.account, &item.secret) {
        Ok(()) => cells.push(MatrixCell {
            name: "store",
            status: CellStatus::Pass,
            note: format!("set_generic_password ok ({} bytes)", item.secret_len()),
        }),
        Err(e) => {
            cells.push(MatrixCell {
                name: "store",
                status: CellStatus::Fail,
                note: e.to_string(),
            });
            let _ = mac::delete(&item.service, &item.account);
            return Ok(report_macos(cells));
        }
    }

    match mac::read(&item.service, &item.account) {
        Ok(got) => {
            let ok = secrets_match(&got, &item.secret);
            cells.push(MatrixCell {
                name: "read",
                status: if ok {
                    CellStatus::Pass
                } else {
                    CellStatus::Fail
                },
                note: if ok {
                    format!("find_generic_password round-trip ok ({} bytes)", got.len())
                } else {
                    "find_generic_password round-trip mismatch".to_string()
                },
            });
        }
        Err(e) => cells.push(MatrixCell {
            name: "read",
            status: CellStatus::Fail,
            note: e.to_string(),
        }),
    }

    match mac::delete(&item.service, &item.account) {
        Ok(()) => cells.push(MatrixCell {
            name: "delete",
            status: CellStatus::Pass,
            note: "delete_generic_password ok".to_string(),
        }),
        Err(e) => cells.push(MatrixCell {
            name: "delete",
            status: CellStatus::Fail,
            note: e.to_string(),
        }),
    }

    match mac::read(&item.service, &item.account) {
        Err(P1Error::Missing) => cells.push(MatrixCell {
            name: "missing-item",
            status: CellStatus::Pass,
            note: "missing after delete".to_string(),
        }),
        Ok(_) => cells.push(MatrixCell {
            name: "missing-item",
            status: CellStatus::Fail,
            note: "unexpected success reading deleted item".to_string(),
        }),
        Err(e) => cells.push(MatrixCell {
            name: "missing-item",
            status: CellStatus::Fail,
            note: format!("expected Missing, got: {e}"),
        }),
    }

    let err = mac::read(&item.service, &item.account)
        .err()
        .unwrap_or(P1Error::Missing);
    let leaked = item.appears_in(&format!("{err}")) || item.appears_in(&format!("{err:?}"));
    cells.push(MatrixCell {
        name: "error-redaction",
        status: if leaked {
            CellStatus::Fail
        } else {
            CellStatus::Pass
        },
        note: if leaked {
            "P1 error Display/Debug contained synthetic secret".to_string()
        } else {
            "typed P1 errors omit secret payloads".to_string()
        },
    });

    let _ = mac::delete(&item.service, &item.account);
    Ok(report_macos(cells))
}

#[cfg(target_os = "macos")]
fn report_macos(cells: Vec<MatrixCell>) -> MatrixReport {
    MatrixReport {
        candidate: CandidateId::P1,
        platform: Platform::Macos,
        provider_note: "direct security-framework generic-password APIs".to_string(),
        cells,
    }
}

#[cfg(not(windows))]
fn run_windows() -> Result<MatrixReport, String> {
    Ok(MatrixReport {
        candidate: CandidateId::P1,
        platform: Platform::Windows,
        provider_note: "direct Cred* APIs — not compiled on this host".to_string(),
        cells: waiting_all("Windows host required"),
    })
}

#[cfg(not(target_os = "macos"))]
fn run_macos() -> Result<MatrixReport, String> {
    Ok(MatrixReport {
        candidate: CandidateId::P1,
        platform: Platform::Macos,
        provider_note: "direct security-framework Keychain APIs — not compiled on this host"
            .to_string(),
        cells: waiting_all("Mac host required; see tools/m1-secure-store-spike/README.md"),
    })
}
