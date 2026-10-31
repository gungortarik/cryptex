//! Supervision architecture remains OPEN until P0-2 evidence justifies ADR-0012.
//!
//! Candidates to compare (docs/23_REQUIRED_TECHNICAL_SPIKES.md):
//! - native per-user background registration (LaunchAgent + Windows per-user startup)
//! - Tauri/tray supervisor spawning cryptexd sidecar
//! - hybrid only if evidence justifies it
//!
//! This module encodes the open decision; it does not select an approach.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisionStrategy {
    /// No ADR-0012 decision has been accepted.
    Unresolved,
}

impl SupervisionStrategy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unresolved => "unresolved",
        }
    }
}

impl fmt::Display for SupervisionStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationStatus {
    /// Platform registration is intentionally not configured during the foundation slice.
    NotConfigured,
}

impl RegistrationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotConfigured => "not_configured",
        }
    }
}

impl fmt::Display for RegistrationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

pub fn selected_supervision() -> SupervisionStrategy {
    SupervisionStrategy::Unresolved
}
