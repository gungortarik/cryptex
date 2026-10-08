//! Typed projection failures (M3).
//!
//! Projection/SQLite state is disposable derived data. Errors here must not be
//! treated as canonical corruption, and must not silently swallow FTS/schema
//! unavailability.

use std::fmt;

use rusqlite::Error as SqliteError;

/// Projection helper failures.
#[derive(Debug)]
pub enum ProjectionError {
    /// Bundled SQLite build lacks FTS5 or FTS5 cannot be exercised.
    Fts5Unavailable { detail: String },
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
            Self::Sqlite(err) => write!(f, "projection sqlite error: {err}"),
        }
    }
}

impl std::error::Error for ProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(err) => Some(err),
            Self::Fts5Unavailable { .. } => None,
        }
    }
}

impl From<SqliteError> for ProjectionError {
    fn from(value: SqliteError) -> Self {
        Self::Sqlite(value)
    }
}

impl PartialEq for ProjectionError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Fts5Unavailable { detail: a }, Self::Fts5Unavailable { detail: b }) => a == b,
            (Self::Sqlite(a), Self::Sqlite(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for ProjectionError {}
