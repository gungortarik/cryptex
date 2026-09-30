use crate::supervision::RegistrationStatus;

/// macOS adapter placeholder. Does not write LaunchAgent plists.
pub fn registration_status() -> RegistrationStatus {
    RegistrationStatus::NotConfigured
}
