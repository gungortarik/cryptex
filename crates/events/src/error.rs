use std::fmt;

/// Typed failures for the event domain (ADR-0016).
///
/// Variants are intentionally broad in slice 1 so later integrity/store slices
/// can refine without silent failure swallowing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventError {
    /// Frame magic/version/length rules failed.
    Framing(String),
    /// A configured resource limit would be exceeded.
    LimitExceeded(String),
    /// Unsupported or incompatible schema version.
    UnsupportedSchema { found: u32, expected: u32 },
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
            Self::Invalid(message) => write!(f, "invalid event: {message}"),
        }
    }
}

impl std::error::Error for EventError {}
