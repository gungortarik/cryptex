//! Platform adapters for background registration.
//!
//! These adapters intentionally do **not** install LaunchAgents or Windows
//! startup entries. Registration remains NotConfigured until P0-2 evidence
//! selects a supervision architecture for ADR-0012.

use crate::supervision::RegistrationStatus;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod other;

pub fn current_registration_status() -> RegistrationStatus {
    #[cfg(target_os = "macos")]
    {
        macos::registration_status()
    }
    #[cfg(target_os = "windows")]
    {
        windows::registration_status()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        other::registration_status()
    }
}
