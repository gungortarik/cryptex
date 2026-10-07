use std::fmt;

use cryptex_core::CoreError;

/// Typed storage-layout failures (ADR-0016).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageError {
    InvalidDeviceId(String),
    InvalidLayout(String),
}

pub type StorageResult<T> = Result<T, StorageError>;

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDeviceId(message) => write!(f, "invalid device id: {message}"),
            Self::InvalidLayout(message) => write!(f, "invalid storage layout: {message}"),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<CoreError> for StorageError {
    fn from(value: CoreError) -> Self {
        match value {
            CoreError::InvalidId(message) => Self::InvalidDeviceId(message),
            other => Self::InvalidLayout(other.to_string()),
        }
    }
}
