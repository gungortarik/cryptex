//! Typed projection failures (M3).
//!
//! Projection/SQLite state is disposable derived data. Errors here must not be
//! treated as canonical corruption, and must not silently swallow FTS/schema
//! unavailability.

use std::fmt;
use std::io;

use rusqlite::Error as SqliteError;

/// Projection helper failures.
#[derive(Debug)]
pub enum ProjectionError {
    /// Bundled SQLite build lacks FTS5 or FTS5 cannot be exercised.
    Fts5Unavailable { detail: String },
    /// On-disk projection schema_version is unsupported by this binary.
    UnsupportedSchema { found: u32, expected: u32 },
    /// Required projection tables/FTS objects are missing or misconfigured.
    SchemaInconsistent { detail: String },
    /// Filesystem failure while creating disposable projection paths.
    Io(io::Error),
    /// Underlying SQLite API failure while probing disposable state.
    Sqlite(SqliteError),
}

pub type ProjectionResult<T> = Result<T, ProjectionError>;

impl fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fts5Unavailable { detail } => {
                write!(f, "SQLite FTS5 unavailable for projection: {detail}")
            }
            Self::UnsupportedSchema { found, expected } => write!(
                f,
                "unsupported projection schema_version {found} (expected {expected})"
            ),
            Self::SchemaInconsistent { detail } => {
                write!(f, "projection schema inconsistent: {detail}")
            }
            Self::Io(err) => write!(f, "projection path io error: {err}"),
            Self::Sqlite(err) => write!(f, "projection sqlite error: {err}"),
        }
    }
}

impl std::error::Error for ProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(err) => Some(err),
            Self::Io(err) => Some(err),
            Self::Fts5Unavailable { .. }
            | Self::UnsupportedSchema { .. }
            | Self::SchemaInconsistent { .. } => None,
        }
    }
}

impl From<SqliteError> for ProjectionError {
    fn from(value: SqliteError) -> Self {
        Self::Sqlite(value)
    }
}

impl From<io::Error> for ProjectionError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl PartialEq for ProjectionError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Fts5Unavailable { detail: a }, Self::Fts5Unavailable { detail: b }) => a == b,
            (
                Self::UnsupportedSchema {
                    found: af,
                    expected: ae,
                },
                Self::UnsupportedSchema {
                    found: bf,
                    expected: be,
                },
            ) => af == bf && ae == be,
            (Self::SchemaInconsistent { detail: a }, Self::SchemaInconsistent { detail: b }) => {
                a == b
            }
            (Self::Io(a), Self::Io(b)) => a.kind() == b.kind() && a.to_string() == b.to_string(),
            (Self::Sqlite(a), Self::Sqlite(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for ProjectionError {}
