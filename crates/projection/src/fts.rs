//! FTS5 availability probe for disposable projection databases (M3 Slice 2).
//!
//! This module does not create product projection schema. It only verifies that
//! the pinned bundled SQLite build exposes FTS5 on the current host.

use rusqlite::Connection;

use crate::{ProjectionError, ProjectionResult};

/// Fail closed unless this connection's SQLite build can use FTS5.
///
/// Checks `PRAGMA compile_options` for `ENABLE_FTS5` and exercises
/// `CREATE VIRTUAL TABLE ... USING fts5` on a temporary in-memory table.
pub fn ensure_fts5_available(conn: &Connection) -> ProjectionResult<()> {
    let mut has_enable_fts5 = false;
    let mut stmt = conn.prepare("PRAGMA compile_options")?;
    let options = stmt.query_map([], |row| row.get::<_, String>(0))?;
    for opt in options {
        let opt = opt?;
        if opt == "ENABLE_FTS5" {
            has_enable_fts5 = true;
            break;
        }
    }
    if !has_enable_fts5 {
        return Err(ProjectionError::Fts5Unavailable {
            detail: "PRAGMA compile_options does not list ENABLE_FTS5".into(),
        });
    }

    conn.execute_batch(
        "CREATE VIRTUAL TABLE temp.__cryptex_fts5_smoke USING fts5(body);
         INSERT INTO temp.__cryptex_fts5_smoke(body) VALUES ('cryptex fts5 smoke');
         DROP TABLE temp.__cryptex_fts5_smoke;",
    )
    .map_err(|err| ProjectionError::Fts5Unavailable {
        detail: format!("FTS5 virtual table smoke failed: {err}"),
    })?;

    Ok(())
}

/// Open an in-memory disposable SQLite connection and require FTS5.
pub fn probe_bundled_fts5() -> ProjectionResult<()> {
    let conn = Connection::open_in_memory()?;
    ensure_fts5_available(&conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_sqlite_exposes_fts5() {
        probe_bundled_fts5().expect("bundled SQLite must enable FTS5 on first-class hosts");
    }

    #[test]
    fn ensure_fts5_available_is_idempotent_on_same_connection() {
        let conn = Connection::open_in_memory().expect("memory db");
        ensure_fts5_available(&conn).expect("first probe");
        ensure_fts5_available(&conn).expect("second probe");
    }
}
