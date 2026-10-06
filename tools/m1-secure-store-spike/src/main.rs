//! Product-separated ADR-0014 secure-store evaluation spike.
//!
//! Prints only PASS/FAIL/WAITING and structural diagnostics.
//! Never prints secret payload bytes.

mod k2;
mod matrix;
mod p1;

use matrix::{CandidateId, CellStatus, MatrixCell, MatrixReport, Platform};

fn main() {
    let platform = current_platform();
    println!("ADR-0014 secure-store spike");
    println!("platform={platform:?}");
    println!("secret_policy=synthetic_throwaway_never_logged");
    println!();

    let mut reports: Vec<MatrixReport> = Vec::new();

    match k2::run(platform) {
        Ok(report) => reports.push(report),
        Err(err) => {
            eprintln!("K2 setup failure (no secret in message): {err}");
            reports.push(MatrixReport::setup_failed(CandidateId::K2, platform, err));
        }
    }

    match p1::run(platform) {
        Ok(report) => reports.push(report),
        Err(err) => {
            eprintln!("P1 setup failure (no secret in message): {err}");
            reports.push(MatrixReport::setup_failed(CandidateId::P1, platform, err));
        }
    }

    // Cross-platform cells that cannot run on this host stay WAITING (never claimed verified).
    if matches!(platform, Platform::Windows) {
        reports.push(waiting_remote(
            CandidateId::K2,
            Platform::Macos,
            "keyring-core=1.0.0 + apple-native-keyring-store=1.0.2 (keychain) — Mac host required",
        ));
        reports.push(waiting_remote(
            CandidateId::P1,
            Platform::Macos,
            "direct security-framework Keychain APIs — Mac host required",
        ));
    } else if matches!(platform, Platform::Macos) {
        reports.push(waiting_remote(
            CandidateId::K2,
            Platform::Windows,
            "keyring-core=1.0.0 + windows-native-keyring-store=1.1.0 — Windows host required",
        ));
        reports.push(waiting_remote(
            CandidateId::P1,
            Platform::Windows,
            "direct Cred* APIs — Windows host required",
        ));
    }

    for report in &reports {
        report.print();
    }

    let failed = reports.iter().any(|r| r.has_failure());
    if failed {
        std::process::exit(1);
    }
}

fn waiting_remote(candidate: CandidateId, platform: Platform, provider_note: &str) -> MatrixReport {
    MatrixReport {
        candidate,
        platform,
        provider_note: provider_note.to_string(),
        cells: ["store", "read", "delete", "missing-item", "error-redaction"]
            .into_iter()
            .map(|name| MatrixCell {
                name,
                status: CellStatus::Waiting,
                note: "see tools/m1-secure-store-spike/README.md".to_string(),
            })
            .collect(),
    }
}

fn current_platform() -> Platform {
    if cfg!(target_os = "windows") {
        Platform::Windows
    } else if cfg!(target_os = "macos") {
        Platform::Macos
    } else {
        Platform::Unsupported
    }
}
