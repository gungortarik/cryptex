//! K2: keyring-core + explicitly selected platform providers.

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
        candidate: CandidateId::K2,
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

#[cfg(windows)]
fn run_windows() -> Result<MatrixReport, String> {
    use keyring_core::{Entry, Error, set_default_store, unset_default_store};
    use windows_native_keyring_store::Store;

    let item = SyntheticItem::new(CandidateId::K2);
    let mut cells = Vec::new();

    let store = Store::new().map_err(|e| format!("Store::new failed: {e}"))?;
    set_default_store(store);

    let entry =
        Entry::new(&item.service, &item.account).map_err(|e| format!("Entry::new failed: {e}"))?;

    let _ = entry.delete_credential();

    match entry.set_secret(&item.secret) {
        Ok(()) => cells.push(MatrixCell {
            name: "store",
            status: CellStatus::Pass,
            note: format!("wrote {} synthetic bytes", item.secret_len()),
        }),
        Err(e) => {
            cells.push(MatrixCell {
                name: "store",
                status: CellStatus::Fail,
                note: format!("set_secret failed: {e}"),
            });
            let _ = entry.delete_credential();
            unset_default_store();
            return Ok(report_windows(cells));
        }
    }

    match entry.get_secret() {
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
                    format!("round-trip ok ({} bytes)", got.len())
                } else {
                    "round-trip byte mismatch".to_string()
                },
            });
        }
        Err(e) => cells.push(MatrixCell {
            name: "read",
            status: CellStatus::Fail,
            note: format!("get_secret failed: {e}"),
        }),
    }

    match entry.delete_credential() {
        Ok(()) => cells.push(MatrixCell {
            name: "delete",
            status: CellStatus::Pass,
            note: "delete_credential ok".to_string(),
        }),
        Err(e) => cells.push(MatrixCell {
            name: "delete",
            status: CellStatus::Fail,
            note: format!("delete_credential failed: {e}"),
        }),
    }

    match entry.get_secret() {
        Err(Error::NoEntry) => cells.push(MatrixCell {
            name: "missing-item",
            status: CellStatus::Pass,
            note: "NoEntry after delete".to_string(),
        }),
        Ok(_) => cells.push(MatrixCell {
            name: "missing-item",
            status: CellStatus::Fail,
            note: "unexpected success reading deleted item".to_string(),
        }),
        Err(e) => cells.push(MatrixCell {
            name: "missing-item",
            status: CellStatus::Fail,
            note: format!("expected NoEntry, got: {e}"),
        }),
    }

    let missing_err = entry.get_secret().err().unwrap_or(Error::NoEntry);
    let payload_err = Error::BadEncoding(item.secret.clone());
    let assessment = crate::redaction::assess_k2_error_surfaces(
        &item.secret,
        &format!("{missing_err}"),
        &format!("{missing_err:?}"),
        &format!("{payload_err}"),
        &format!("{payload_err:?}"),
    );
    cells.push(MatrixCell {
        name: "error-redaction",
        status: if assessment.overall_pass() {
            CellStatus::Pass
        } else {
            CellStatus::Fail
        },
        note: assessment.matrix_note(),
    });

    let _ = entry.delete_credential();
    unset_default_store();
    Ok(report_windows(cells))
}

#[cfg(windows)]
fn report_windows(cells: Vec<MatrixCell>) -> MatrixReport {
    MatrixReport {
        candidate: CandidateId::K2,
        platform: Platform::Windows,
        provider_note: "keyring-core=1.0.0 + windows-native-keyring-store=1.1.0 (default-features=false); default target_name={user}.{service}".to_string(),
        cells,
    }
}

#[cfg(target_os = "macos")]
fn run_macos() -> Result<MatrixReport, String> {
    use apple_native_keyring_store::keychain::Store;
    use keyring_core::{Entry, Error, set_default_store, unset_default_store};

    let item = SyntheticItem::new(CandidateId::K2);
    let mut cells = Vec::new();

    let store = Store::new().map_err(|e| format!("Store::new failed: {e}"))?;
    set_default_store(store);

    let entry =
        Entry::new(&item.service, &item.account).map_err(|e| format!("Entry::new failed: {e}"))?;
    let _ = entry.delete_credential();

    match entry.set_secret(&item.secret) {
        Ok(()) => cells.push(MatrixCell {
            name: "store",
            status: CellStatus::Pass,
            note: format!("wrote {} synthetic bytes", item.secret_len()),
        }),
        Err(e) => {
            cells.push(MatrixCell {
                name: "store",
                status: CellStatus::Fail,
                note: format!("set_secret failed: {e}"),
            });
            let _ = entry.delete_credential();
            unset_default_store();
            return Ok(report_macos(cells));
        }
    }

    match entry.get_secret() {
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
                    format!("round-trip ok ({} bytes)", got.len())
                } else {
                    "round-trip byte mismatch".to_string()
                },
            });
        }
        Err(e) => cells.push(MatrixCell {
            name: "read",
            status: CellStatus::Fail,
            note: format!("get_secret failed: {e}"),
        }),
    }

    match entry.delete_credential() {
        Ok(()) => cells.push(MatrixCell {
            name: "delete",
            status: CellStatus::Pass,
            note: "delete_credential ok".to_string(),
        }),
        Err(e) => cells.push(MatrixCell {
            name: "delete",
            status: CellStatus::Fail,
            note: format!("delete_credential failed: {e}"),
        }),
    }

    match entry.get_secret() {
        Err(Error::NoEntry) => cells.push(MatrixCell {
            name: "missing-item",
            status: CellStatus::Pass,
            note: "NoEntry after delete".to_string(),
        }),
        Ok(_) => cells.push(MatrixCell {
            name: "missing-item",
            status: CellStatus::Fail,
            note: "unexpected success reading deleted item".to_string(),
        }),
        Err(e) => cells.push(MatrixCell {
            name: "missing-item",
            status: CellStatus::Fail,
            note: format!("expected NoEntry, got: {e}"),
        }),
    }

    let missing_err = entry.get_secret().err().unwrap_or(Error::NoEntry);
    let payload_err = Error::BadEncoding(item.secret.clone());
    let assessment = crate::redaction::assess_k2_error_surfaces(
        &item.secret,
        &format!("{missing_err}"),
        &format!("{missing_err:?}"),
        &format!("{payload_err}"),
        &format!("{payload_err:?}"),
    );
    cells.push(MatrixCell {
        name: "error-redaction",
        status: if assessment.overall_pass() {
            CellStatus::Pass
        } else {
            CellStatus::Fail
        },
        note: assessment.matrix_note(),
    });

    let _ = entry.delete_credential();
    unset_default_store();
    Ok(report_macos(cells))
}

#[cfg(target_os = "macos")]
fn report_macos(cells: Vec<MatrixCell>) -> MatrixReport {
    MatrixReport {
        candidate: CandidateId::K2,
        platform: Platform::Macos,
        provider_note: "keyring-core=1.0.0 + apple-native-keyring-store=1.0.2 (keychain feature)"
            .to_string(),
        cells,
    }
}

#[cfg(not(windows))]
fn run_windows() -> Result<MatrixReport, String> {
    Ok(MatrixReport {
        candidate: CandidateId::K2,
        platform: Platform::Windows,
        provider_note: "windows-native-keyring-store=1.1.0 — not compiled on this host".to_string(),
        cells: waiting_all("Windows host required"),
    })
}

#[cfg(not(target_os = "macos"))]
fn run_macos() -> Result<MatrixReport, String> {
    Ok(MatrixReport {
        candidate: CandidateId::K2,
        platform: Platform::Macos,
        provider_note: "apple-native-keyring-store=1.0.2 (keychain) — not compiled on this host"
            .to_string(),
        cells: waiting_all("Mac host required; see tools/m1-secure-store-spike/README.md"),
    })
}
