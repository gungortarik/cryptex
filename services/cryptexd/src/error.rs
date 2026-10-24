use std::fmt;

/// Typed lifecycle errors. Failures are explicit and never silently swallowed.
#[derive(Debug)]
pub enum LifecycleError {
    Usage(String),
    Io(String),
    AlreadyRunning { holder_pid: Option<u32> },
    Incompatible(String),
    UnsafeRuntimePath(String),
    ImportRecoveryConflict(String),
    Attention(String),
}

impl fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) => write!(f, "{message}"),
            Self::Io(message) => write!(f, "io error: {message}"),
            Self::AlreadyRunning { holder_pid } => match holder_pid {
                Some(pid) => write!(f, "daemon instance already running (pid {pid})"),
                None => write!(f, "daemon instance already running"),
            },
            Self::Incompatible(message) => write!(f, "incompatible: {message}"),
            Self::UnsafeRuntimePath(message) => write!(f, "unsafe runtime path: {message}"),
            Self::ImportRecoveryConflict(message) => {
                write!(f, "import recovery conflict: {message}")
            }
            Self::Attention(message) => write!(f, "attention: {message}"),
        }
    }
}

impl std::error::Error for LifecycleError {}

impl From<std::io::Error> for LifecycleError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

pub type LifecycleResult<T> = Result<T, LifecycleError>;
