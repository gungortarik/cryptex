use crate::supervision::RegistrationStatus;

/// Windows adapter placeholder. Does not register per-user startup entries.
pub fn registration_status() -> RegistrationStatus {
    RegistrationStatus::NotConfigured
}
