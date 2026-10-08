use std::fmt;

use cryptex_core::CoreError;

/// Typed failures for the event domain (ADR-0016).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventError {
    /// Frame magic/version/length rules failed.
    Framing(String),
    /// A configured resource limit would be exceeded.
    LimitExceeded(String),
    /// Unsupported or incompatible schema version.
    UnsupportedSchema { found: u32, expected: u32 },
    /// Hash, signature, JCS, or integrity-chain failure.
    Integrity(String),
    /// Catch-all for later validation wiring; must remain typed and visible.
    Invalid(String),
}

pub type EventResult<T> = Result<T, EventError>;

impl fmt::Display for EventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Framing(message) => write!(f, "event framing error: {message}"),
            Self::LimitExceeded(message) => write!(f, "event limit exceeded: {message}"),
            Self::UnsupportedSchema { found, expected } => write!(
                f,
                "unsupported event schema_version {found} (expected {expected})"
            ),
            Self::Integrity(message) => write!(f, "event integrity error: {message}"),
            Self::Invalid(message) => write!(f, "invalid event: {message}"),
        }
    }
}

impl std::error::Error for EventError {}

impl From<CoreError> for EventError {
    fn from(value: CoreError) -> Self {
        match value {
            CoreError::UnsupportedSchema {
                kind: _,
                found,
                expected,
            } => Self::UnsupportedSchema { found, expected },
            other => Self::Invalid(other.to_string()),
        }
    }
}
