use std::fmt;
use std::io;

use cryptex_core::CoreError;
use cryptex_events::EventError;

/// Typed storage failures (ADR-0016).
#[derive(Debug)]
pub enum StorageError {
    InvalidDeviceId(String),
    InvalidLayout(String),
    Framing(String),
    Integrity(String),
    Recovery(String),
    Quarantine(String),
    Io(io::Error),
    Event(EventError),
}

pub type StorageResult<T> = Result<T, StorageError>;

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDeviceId(message) => write!(f, "invalid device id: {message}"),
            Self::InvalidLayout(message) => write!(f, "invalid storage layout: {message}"),
            Self::Framing(message) => write!(f, "segment framing error: {message}"),
            Self::Integrity(message) => write!(f, "segment integrity error: {message}"),
            Self::Recovery(message) => write!(f, "segment recovery error: {message}"),
            Self::Quarantine(message) => write!(f, "quarantine error: {message}"),
            Self::Io(error) => write!(f, "storage io: {error}"),
            Self::Event(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Event(error) => Some(error),
            _ => None,
        }
    }
}

impl PartialEq for StorageError {
    fn eq(&self, other: &Self) -> bool {
        use StorageError::*;
        match (self, other) {
            (InvalidDeviceId(a), InvalidDeviceId(b))
            | (InvalidLayout(a), InvalidLayout(b))
            | (Framing(a), Framing(b))
            | (Integrity(a), Integrity(b))
            | (Recovery(a), Recovery(b))
            | (Quarantine(a), Quarantine(b)) => a == b,
            (Event(a), Event(b)) => a == b,
            (Io(a), Io(b)) => a.kind() == b.kind() && a.to_string() == b.to_string(),
            _ => false,
        }
    }
}

impl Eq for StorageError {}

impl From<CoreError> for StorageError {
    fn from(value: CoreError) -> Self {
        match value {
            CoreError::InvalidId(message) => Self::InvalidDeviceId(message),
            other => Self::InvalidLayout(other.to_string()),
        }
    }
}

impl From<EventError> for StorageError {
    fn from(value: EventError) -> Self {
        match value {
            EventError::Framing(message) => Self::Framing(message),
            EventError::Integrity(message) => Self::Integrity(message),
            other => Self::Event(other),
        }
    }
}

impl From<io::Error> for StorageError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
