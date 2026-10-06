//! Platform adapters for background registration / supervision proofs.
//!
//! Production registration remains NotConfigured until ADR-0012 evidence exists.
//! macOS exposes a reversible test-only LaunchAgent harness for Mode A.

use crate::supervision::RegistrationStatus;

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

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
