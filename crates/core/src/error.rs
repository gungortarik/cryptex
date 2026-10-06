use std::fmt;

/// Typed domain errors for identity and sequence foundations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    InvalidId(String),
    SequenceStateMissing { device_id: String },
    SequenceStateAlreadyExists { device_id: String },
    SequenceStateCorrupt { device_id: String, detail: String },
    SequenceOverflow { device_id: String },
    SequenceNonMonotonic { device_id: String },
    Durability(String),
    Io(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId(message) => write!(f, "invalid id: {message}"),
            Self::SequenceStateMissing { device_id } => {
                write!(f, "sequence state missing for device {device_id}")
            }
            Self::SequenceStateAlreadyExists { device_id } => {
                write!(f, "sequence state already exists for device {device_id}")
            }
            Self::SequenceStateCorrupt { device_id, detail } => {
                write!(f, "sequence state corrupt for device {device_id}: {detail}")
            }
            Self::SequenceOverflow { device_id } => {
                write!(f, "sequence overflow for device {device_id}")
            }
            Self::SequenceNonMonotonic { device_id } => {
                write!(f, "sequence non-monotonic for device {device_id}")
            }
            Self::Durability(message) => write!(f, "durability error: {message}"),
            Self::Io(message) => write!(f, "io error: {message}"),
        }
    }
}

impl std::error::Error for CoreError {}

impl From<std::io::Error> for CoreError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

impl From<serde_json::Error> for CoreError {
    fn from(value: serde_json::Error) -> Self {
        Self::SequenceStateCorrupt {
            device_id: String::new(),
            detail: value.to_string(),
        }
    }
}

pub type CoreResult<T> = Result<T, CoreError>;
