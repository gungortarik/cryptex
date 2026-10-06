//! Shared evaluation matrix types. Secret bytes never appear in Display output.

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Platform {
    Windows,
    Macos,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateId {
    K2,
    P1,
}

impl fmt::Display for CandidateId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CandidateId::K2 => write!(f, "K2"),
            CandidateId::P1 => write!(f, "P1"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CellStatus {
    Pass,
    Fail,
    Waiting,
}

impl fmt::Display for CellStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CellStatus::Pass => write!(f, "PASS"),
            CellStatus::Fail => write!(f, "FAIL"),
            CellStatus::Waiting => write!(f, "WAITING"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct MatrixCell {
    pub name: &'static str,
    pub status: CellStatus,
    pub note: String,
}

#[derive(Clone, Debug)]
pub struct MatrixReport {
    pub candidate: CandidateId,
    pub platform: Platform,
    pub provider_note: String,
    pub cells: Vec<MatrixCell>,
}

impl MatrixReport {
    pub fn setup_failed(candidate: CandidateId, platform: Platform, err: String) -> Self {
        Self {
            candidate,
            platform,
            provider_note: "setup-failed".to_string(),
            cells: vec![MatrixCell {
                name: "setup",
                status: CellStatus::Fail,
                note: err,
            }],
        }
    }

    pub fn has_failure(&self) -> bool {
        self.cells
            .iter()
            .any(|c| matches!(c.status, CellStatus::Fail))
    }

    pub fn print(&self) {
        println!(
            "--- candidate={} platform={:?} ---",
            self.candidate, self.platform
        );
        println!("provider={}", self.provider_note);
        for cell in &self.cells {
            println!("  {:<16} {:<8} {}", cell.name, cell.status, cell.note);
        }
        println!();
    }
}

/// Synthetic throwaway values for one evaluation run.
/// The secret is never logged; only length and equality results are reported.
pub struct SyntheticItem {
    pub service: String,
    pub account: String,
    pub secret: Vec<u8>,
}

impl SyntheticItem {
    pub fn new(candidate: CandidateId) -> Self {
        let run_id = std::process::id();
        let account = format!(
            "synthetic-{}-{}-{run_id}",
            candidate.to_string().to_lowercase(),
            now_millis()
        );
        // Distinctive throwaway pattern that must never appear in logs/evidence.
        let secret = format!("SYNTHETIC_SPIKE_SECRET_NOT_REAL::{account}::v1").into_bytes();
        Self {
            service: "Cryptex-Dev-M1-Spike".to_string(),
            account,
            secret,
        }
    }

    pub fn secret_len(&self) -> usize {
        self.secret.len()
    }

    /// Returns true if `text` exposes secret payload bytes (UTF-8 and Debug/hex forms).
    pub fn appears_in(&self, text: &str) -> bool {
        crate::redaction::payload_bytes_recoverable(&self.secret, text)
    }
}

impl Drop for SyntheticItem {
    fn drop(&mut self) {
        for b in &mut self.secret {
            *b = 0;
        }
    }
}

fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

pub fn secrets_match(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}
