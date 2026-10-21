//! Library search over disposable projection rows (M3 Slice 6).
//!
//! Optional FTS5 text query plus conjunction filters. All bound values use
//! parameterized SQL. `created_at` ranges compare exact UTC instants per M2
//! RFC3339 rules (including offsets and fractional seconds); lexical string
//! ordering is never used for ranges.

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::time::Instant;

use cryptex_core::validate::{Rfc3339UtcInstant, rfc3339_utc_instant};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};

use crate::{ProjectionError, ProjectionResult};

/// Conjunction search over projected `memory.note` rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchQuery {
    /// Optional FTS5 `MATCH` query over note text. `None` means no text constraint.
    pub text: Option<String>,
    pub classification: Option<String>,
    pub event_type: Option<String>,
    pub device_id: Option<String>,
    pub brain_id: Option<String>,
    /// Inclusive lower bound (RFC3339). Compared as UTC instant.
    pub created_at_from: Option<String>,
    /// Inclusive upper bound (RFC3339). Compared as UTC instant.
    pub created_at_to: Option<String>,
    /// Optional maximum hits after exact-instant ordering.
    ///
    /// When set, matching rows are selected with O(limit) retained memory.
    /// When unset, all matches are collected (callers should bound large stores).
    pub limit: Option<u32>,
}

/// One projected row returned from search (M2 fields only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub event_id: String,
    pub brain_id: String,
    pub device_id: String,
    pub device_seq: String,
    pub created_at: String,
    pub event_type: String,
    pub classification: String,
    pub content_hash: String,
    pub signing_key_id: String,
    pub text: String,
}

/// Keyset cursor for the newest-first timeline. The cursor is a continuation
/// hint only; callers must re-run the daemon's retrieval-safety check per page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineCursor {
    pub created_at: String,
    pub event_id: String,
}

/// Filters and bounded page size for the local note timeline.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TimelineQuery {
    pub classification: Option<String>,
    pub event_type: Option<String>,
    pub device_id: Option<String>,
    pub brain_id: Option<String>,
    pub created_at_from: Option<String>,
    pub created_at_to: Option<String>,
    pub cursor: Option<TimelineCursor>,
    /// Required page size, validated to `1..=50`.
    pub limit: u32,
}

/// One bounded newest-first timeline page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelinePage {
    pub hits: Vec<SearchHit>,
    pub next_cursor: Option<TimelineCursor>,
}

/// Read one projected note by exact event ID. The daemon must run the shared
/// retrieval-safety gate before exposing this row.
pub fn event_detail(
    conn: &Connection,
    brain_id: &str,
    event_id: &str,
) -> ProjectionResult<Option<SearchHit>> {
    if event_id.is_empty() || event_id.len() > 128 {
        return Err(ProjectionError::InvalidQuery {
            detail: "event id is invalid".into(),
        });
    }
    let mut statement = conn.prepare(
        "SELECT event_id, brain_id, device_id, device_seq, created_at, event_type, \
         classification, content_hash, signing_key_id, text FROM events \
         WHERE brain_id = ?1 AND event_id = ?2",
    )?;
    statement
        .query_row(params![brain_id, event_id], |row| {
            Ok(SearchHit {
                event_id: row.get(0)?,
                brain_id: row.get(1)?,
                device_id: row.get(2)?,
                device_seq: row.get(3)?,
                created_at: row.get(4)?,
                event_type: row.get(5)?,
                classification: row.get(6)?,
                content_hash: row.get(7)?,
                signing_key_id: row.get(8)?,
                text: row.get(9)?,
            })
        })
        .optional()
        .map_err(ProjectionError::from)
}

/// Search projected events.
///
/// Result order is deterministic:
/// 1. `created_at` ascending by exact UTC instant, including fractional seconds
/// 2. `event_id` ascending (lexicographic UUID string) only when instants are equal
///
/// Invalid/unsupported FTS queries and invalid RFC3339 bounds return
/// [`ProjectionError::InvalidQuery`] rather than an empty hit list.
pub fn search_events(conn: &Connection, query: &SearchQuery) -> ProjectionResult<Vec<SearchHit>> {
    search_events_internal(conn, query, None, false)
}

/// Return one bounded newest-first page. The `limit + 1` probe determines
/// whether another page exists without retaining every matching row.
pub fn timeline_events(conn: &Connection, query: &TimelineQuery) -> ProjectionResult<TimelinePage> {
    if !(1..=50).contains(&query.limit) {
        return Err(ProjectionError::InvalidQuery {
            detail: "timeline limit must be between 1 and 50".into(),
        });
    }
    let search_query = SearchQuery {
        text: None,
        classification: query.classification.clone(),
        event_type: query.event_type.clone(),
        device_id: query.device_id.clone(),
        brain_id: query.brain_id.clone(),
        created_at_from: query.created_at_from.clone(),
        created_at_to: query.created_at_to.clone(),
        limit: Some(query.limit + 1),
    };
    let hits = search_events_internal(conn, &search_query, query.cursor.as_ref(), true)?;
    let mut hits = hits;
    let has_more = hits.len() > query.limit as usize;
    hits.truncate(query.limit as usize);
    let next_cursor = if has_more {
        hits.last().map(|hit| TimelineCursor {
            created_at: hit.created_at.clone(),
            event_id: hit.event_id.clone(),
        })
    } else {
        None
    };
    Ok(TimelinePage { hits, next_cursor })
}

fn search_events_internal(
    conn: &Connection,
    query: &SearchQuery,
    cursor: Option<&TimelineCursor>,
    descending: bool,
) -> ProjectionResult<Vec<SearchHit>> {
    if let Some(text) = &query.text
        && text.is_empty()
    {
        return Err(ProjectionError::InvalidQuery {
            detail: "text query must not be empty when provided".into(),
        });
    }

    let from_instant = match &query.created_at_from {
        Some(raw) => {
            Some(
                rfc3339_utc_instant(raw).map_err(|err| ProjectionError::InvalidQuery {
                    detail: format!("created_at_from is not valid M2 RFC3339: {err}"),
                })?,
            )
        }
        None => None,
    };
    let to_instant = match &query.created_at_to {
        Some(raw) => {
            Some(
                rfc3339_utc_instant(raw).map_err(|err| ProjectionError::InvalidQuery {
                    detail: format!("created_at_to is not valid M2 RFC3339: {err}"),
                })?,
            )
        }
        None => None,
    };
    if let (Some(from), Some(to)) = (&from_instant, &to_instant)
        && from > to
    {
        return Err(ProjectionError::InvalidQuery {
            detail: "created_at_from must be <= created_at_to".into(),
        });
    }
    let cursor_instant = match cursor {
        Some(cursor) if cursor.event_id.is_empty() || cursor.event_id.len() > 128 => {
            return Err(ProjectionError::InvalidQuery {
                detail: "timeline cursor event id is invalid".into(),
            });
        }
        Some(cursor) => Some(rfc3339_utc_instant(&cursor.created_at).map_err(|err| {
            ProjectionError::InvalidQuery {
                detail: format!("timeline cursor timestamp is invalid: {err}"),
            }
        })?),
        None => None,
    };

    let mut sql = String::from(
        "SELECT event_id, brain_id, device_id, device_seq, created_at, event_type, \
         classification, content_hash, signing_key_id, text \
         FROM events WHERE 1=1",
    );
    let mut binds: Vec<String> = Vec::new();

    if let Some(v) = &query.classification {
        binds.push(v.clone());
        sql.push_str(&format!(" AND classification = ?{}", binds.len()));
    }
    if let Some(v) = &query.event_type {
        binds.push(v.clone());
        sql.push_str(&format!(" AND event_type = ?{}", binds.len()));
    }
    if let Some(v) = &query.device_id {
        binds.push(v.clone());
        sql.push_str(&format!(" AND device_id = ?{}", binds.len()));
    }
    if let Some(v) = &query.brain_id {
        binds.push(v.clone());
        sql.push_str(&format!(" AND brain_id = ?{}", binds.len()));
    }
    if let Some(v) = &query.text {
        binds.push(v.clone());
        sql.push_str(&format!(
            " AND rowid IN (SELECT rowid FROM events_fts WHERE events_fts MATCH ?{})",
            binds.len()
        ));
    }

    let fts_active = query.text.is_some();
    let map_err = |err| map_search_sqlite_error(err, fts_active);
    let mut stmt = conn.prepare(&sql).map_err(map_err)?;
    let rows = stmt
        .query_map(params_from_iter(binds.iter()), |row| {
            Ok(SearchHit {
                event_id: row.get(0)?,
                brain_id: row.get(1)?,
                device_id: row.get(2)?,
                device_seq: row.get(3)?,
                created_at: row.get(4)?,
                event_type: row.get(5)?,
                classification: row.get(6)?,
                content_hash: row.get(7)?,
                signing_key_id: row.get(8)?,
                text: row.get(9)?,
            })
        })
        .map_err(map_err)?;

    // When `limit` is set, keep only the best `limit` hits in a bounded max-heap
    // (exact UTC instant, then event_id). Unlimited queries still collect all
    // matches — callers should pass `limit` for large local stores.
    let limit = query.limit.map(|n| n as usize);
    if limit == Some(0) {
        return Ok(Vec::new());
    }

    let mut unlimited: Vec<(Rfc3339UtcInstant, SearchHit)> = Vec::new();
    let mut bounded: BinaryHeap<RankedHit> = BinaryHeap::new();

    for row in rows {
        let hit = row.map_err(map_err)?;
        let instant = rfc3339_utc_instant(&hit.created_at).map_err(|err| {
            ProjectionError::SchemaInconsistent {
                detail: format!(
                    "projected created_at failed M2 RFC3339 parse for event_id {}: {err}",
                    hit.event_id
                ),
            }
        })?;
        if let Some(from) = &from_instant
            && &instant < from
        {
            continue;
        }
        if let Some(to) = &to_instant
            && &instant > to
        {
            continue;
        }

        if let (Some(cursor), Some(cursor_instant)) = (cursor, cursor_instant.as_ref()) {
            let cursor_order = instant
                .cmp(cursor_instant)
                .then_with(|| hit.event_id.cmp(&cursor.event_id));
            // Timeline is newest-first; subsequent pages continue strictly
            // before the previous page's final (instant, event_id) tuple.
            if cursor_order != Ordering::Less {
                continue;
            }
        }

        if let Some(max_keep) = limit {
            let ranked = RankedHit {
                instant,
                hit,
                descending,
            };
            if bounded.len() < max_keep {
                bounded.push(ranked);
            } else if let Some(worst) = bounded.peek() {
                // Max-heap peek is the largest (worst) among kept smallest hits.
                if ranked.cmp(worst) == Ordering::Less {
                    bounded.pop();
                    bounded.push(ranked);
                }
            }
        } else {
            unlimited.push((instant, hit));
        }
    }

    let out = if limit.is_some() {
        // into_sorted_vec yields ascending order for BinaryHeap.
        bounded
            .into_sorted_vec()
            .into_iter()
            .map(|ranked| ranked.hit)
            .collect()
    } else {
        unlimited.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.event_id.cmp(&b.1.event_id)));
        unlimited.into_iter().map(|(_, h)| h).collect()
    };
    Ok(out)
}

/// Run a bounded search with SQLite VM cancellation. The progress hook is
/// always removed before returning so a timed-out guarded query cannot affect
/// a later M3 projection operation using the same connection.
pub fn search_events_until(
    conn: &Connection,
    query: &SearchQuery,
    deadline: Instant,
) -> ProjectionResult<Vec<SearchHit>> {
    if Instant::now() >= deadline {
        return Err(ProjectionError::OperationDeadlineExceeded);
    }
    conn.progress_handler(1_000, Some(move || Instant::now() >= deadline))?;
    let result = search_events(conn, query);
    let deadline_expired = Instant::now() >= deadline;
    let clear_result = conn.progress_handler(0, None::<fn() -> bool>);
    if deadline_expired {
        clear_result?;
        return Err(ProjectionError::OperationDeadlineExceeded);
    }
    match (result, clear_result) {
        (Ok(hits), Ok(())) => Ok(hits),
        (Err(error), _) => Err(error),
        (_, Err(error)) => Err(error.into()),
    }
}

/// Run one bounded timeline page with SQLite VM cancellation.
pub fn timeline_events_until(
    conn: &Connection,
    query: &TimelineQuery,
    deadline: Instant,
) -> ProjectionResult<TimelinePage> {
    if Instant::now() >= deadline {
        return Err(ProjectionError::OperationDeadlineExceeded);
    }
    conn.progress_handler(1_000, Some(move || Instant::now() >= deadline))?;
    let result = timeline_events(conn, query);
    let deadline_expired = Instant::now() >= deadline;
    let clear_result = conn.progress_handler(0, None::<fn() -> bool>);
    if deadline_expired {
        clear_result?;
        return Err(ProjectionError::OperationDeadlineExceeded);
    }
    match (result, clear_result) {
        (Ok(page), Ok(())) => Ok(page),
        (Err(error), _) => Err(error),
        (_, Err(error)) => Err(error.into()),
    }
}

pub fn event_detail_until(
    conn: &Connection,
    brain_id: &str,
    event_id: &str,
    deadline: Instant,
) -> ProjectionResult<Option<SearchHit>> {
    if Instant::now() >= deadline {
        return Err(ProjectionError::OperationDeadlineExceeded);
    }
    conn.progress_handler(1_000, Some(move || Instant::now() >= deadline))?;
    let result = event_detail(conn, brain_id, event_id);
    let deadline_expired = Instant::now() >= deadline;
    let clear_result = conn.progress_handler(0, None::<fn() -> bool>);
    if deadline_expired {
        clear_result?;
        return Err(ProjectionError::OperationDeadlineExceeded);
    }
    match (result, clear_result) {
        (Ok(detail), Ok(())) => Ok(detail),
        (Err(error), _) => Err(error),
        (_, Err(error)) => Err(error.into()),
    }
}

/// Max-heap element: larger (instant, event_id) sorts greater so the worst kept
/// hit sits at the heap peak when bounding by `limit`.
#[derive(Debug)]
struct RankedHit {
    instant: Rfc3339UtcInstant,
    hit: SearchHit,
    descending: bool,
}

impl PartialEq for RankedHit {
    fn eq(&self, other: &Self) -> bool {
        self.instant == other.instant
            && self.hit.event_id == other.hit.event_id
            && self.descending == other.descending
    }
}

impl Eq for RankedHit {}

impl PartialOrd for RankedHit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RankedHit {
    fn cmp(&self, other: &Self) -> Ordering {
        let direction = self.descending.cmp(&other.descending);
        if direction != Ordering::Equal {
            return direction;
        }
        let order = self
            .instant
            .cmp(&other.instant)
            .then_with(|| self.hit.event_id.cmp(&other.hit.event_id));
        if self.descending {
            order.reverse()
        } else {
            order
        }
    }
}

fn map_search_sqlite_error(err: rusqlite::Error, fts_active: bool) -> ProjectionError {
    let message = err.to_string();
    let lower = message.to_ascii_lowercase();
    let looks_like_query_problem = lower.contains("fts5")
        || lower.contains("fts")
        || lower.contains("match")
        || lower.contains("syntax")
        || lower.contains("malformed")
        || lower.contains("unterminated")
        || lower.contains("no such column");
    if fts_active && looks_like_query_problem {
        return ProjectionError::InvalidQuery {
            detail: format!("unsupported or invalid FTS query: {message}"),
        };
    }
    ProjectionError::Sqlite(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectionDb;
    use crate::ingest::project_memory_note;
    use cryptex_events::device_seq::DeviceSeq;
    use cryptex_events::envelope::{
        AcceptedEvent, Classification, EVENT_SCHEMA_VERSION, EventPayload, EventType,
        MemoryNotePayload,
    };
    use cryptex_events::hex::{ContentHashHex, SignatureHex, SigningKeyId};

    #[test]
    fn expired_search_deadline_returns_no_hits_and_does_not_poison_connection() {
        let db = ProjectionDb::open_in_memory().unwrap();
        let expired = Instant::now() - std::time::Duration::from_millis(1);
        assert!(matches!(
            search_events_until(db.connection(), &SearchQuery::default(), expired),
            Err(ProjectionError::OperationDeadlineExceeded)
        ));
        assert!(
            search_events(db.connection(), &SearchQuery::default())
                .unwrap()
                .is_empty()
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn note(
        event_id: &str,
        device_id: &str,
        seq: u64,
        created_at: &str,
        classification: Classification,
        text: &str,
        content_hash: &str,
        brain_id: &str,
    ) -> AcceptedEvent {
        AcceptedEvent {
            schema_version: EVENT_SCHEMA_VERSION,
            event_id: event_id.into(),
            brain_id: brain_id.into(),
            device_id: device_id.into(),
            device_seq: DeviceSeq::from_u64(seq).unwrap(),
            created_at: created_at.into(),
            event_type: EventType::MemoryNote,
            classification,
            signing_key_id: SigningKeyId::parse(&"ab".repeat(16)).unwrap(),
            payload: EventPayload::MemoryNote(MemoryNotePayload::new(text).unwrap()),
            prev_content_hash: ContentHashHex::zero(),
            content_hash: ContentHashHex::parse(content_hash).unwrap(),
            signature: SignatureHex::parse(&"ef".repeat(64)).unwrap(),
        }
    }

    fn seed_db() -> ProjectionDb {
        let db = ProjectionDb::open_in_memory().unwrap();
        let brain = "22222222-2222-4222-8222-222222222222";
        let rows = [
            note(
                "11111111-1111-4111-8111-111111111111",
                "device-a",
                1,
                "2026-10-06T12:00:00Z",
                Classification::Public,
                "alpha searchable café",
                &"11".repeat(32),
                brain,
            ),
            note(
                "22222222-2222-4222-8222-222222222222",
                "device-a",
                2,
                "2026-10-06T13:00:00Z",
                Classification::Internal,
                "beta other notes",
                &"22".repeat(32),
                brain,
            ),
            note(
                "33333333-3333-4333-8333-333333333333",
                "device-b",
                1,
                "2026-10-06T08:00:00-05:00", // == 13:00Z
                Classification::Private,
                "gamma café unicode",
                &"33".repeat(32),
                "44444444-4444-4444-8444-444444444444",
            ),
            note(
                "44444444-4444-4444-8444-444444444444",
                "device-b",
                2,
                "2026-10-07T00:00:00Z",
                Classification::Restricted,
                "delta later day",
                &"44".repeat(32),
                brain,
            ),
        ];
        for row in &rows {
            project_memory_note(db.connection(), row).unwrap();
        }
        db
    }

    #[test]
    fn fts_match_and_non_match() {
        let db = seed_db();
        let hits = search_events(
            db.connection(),
            &SearchQuery {
                text: Some("searchable".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].event_id, "11111111-1111-4111-8111-111111111111");

        let none = search_events(
            db.connection(),
            &SearchQuery {
                text: Some("zzznomatchtokenqqq".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap();
        assert!(none.is_empty());
    }

    #[test]
    fn fts_unicode_match() {
        let db = seed_db();
        let hits = search_events(
            db.connection(),
            &SearchQuery {
                text: Some("café".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].event_id, "11111111-1111-4111-8111-111111111111");
        assert_eq!(hits[1].event_id, "33333333-3333-4333-8333-333333333333");
    }

    #[test]
    fn metadata_filters_alone_and_combined() {
        let db = seed_db();
        let by_class = search_events(
            db.connection(),
            &SearchQuery {
                classification: Some("private".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap();
        assert_eq!(by_class.len(), 1);
        assert_eq!(by_class[0].device_id, "device-b");

        let by_type = search_events(
            db.connection(),
            &SearchQuery {
                event_type: Some("memory.note".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap();
        assert_eq!(by_type.len(), 4);

        let by_device = search_events(
            db.connection(),
            &SearchQuery {
                device_id: Some("device-a".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap();
        assert_eq!(by_device.len(), 2);

        let by_brain = search_events(
            db.connection(),
            &SearchQuery {
                brain_id: Some("44444444-4444-4444-8444-444444444444".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap();
        assert_eq!(by_brain.len(), 1);

        let combined = search_events(
            db.connection(),
            &SearchQuery {
                text: Some("café".into()),
                classification: Some("public".into()),
                device_id: Some("device-a".into()),
                event_type: Some("memory.note".into()),
                brain_id: Some("22222222-2222-4222-8222-222222222222".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap();
        assert_eq!(combined.len(), 1);
        assert_eq!(combined[0].event_id, "11111111-1111-4111-8111-111111111111");
    }

    #[test]
    fn created_at_range_uses_utc_instants_not_lexical_order() {
        let db = seed_db();
        // 12:30Z .. 13:30Z should include device-b's 08:00-05:00 (== 13:00Z)
        // and device-a's 13:00Z, but not 12:00Z or next-day.
        let hits = search_events(
            db.connection(),
            &SearchQuery {
                created_at_from: Some("2026-10-06T12:30:00Z".into()),
                created_at_to: Some("2026-10-06T13:30:00Z".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap();
        let ids: Vec<_> = hits.iter().map(|h| h.event_id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "22222222-2222-4222-8222-222222222222",
                "33333333-3333-4333-8333-333333333333",
            ]
        );

        // Offset-form bound equivalent to 13:00Z inclusive lower edge.
        let from_offset = search_events(
            db.connection(),
            &SearchQuery {
                created_at_from: Some("2026-10-06T08:00:00-05:00".into()),
                created_at_to: Some("2026-10-06T08:00:00-05:00".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap();
        assert_eq!(from_offset.len(), 2); // 13:00Z note and 08:00-05:00 note share instant
    }

    #[test]
    fn ordering_is_created_at_then_event_id() {
        let db = seed_db();
        let hits = search_events(db.connection(), &SearchQuery::default()).unwrap();
        let ids: Vec<_> = hits.iter().map(|h| h.event_id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "11111111-1111-4111-8111-111111111111", // 12:00Z
                "22222222-2222-4222-8222-222222222222", // 13:00Z
                "33333333-3333-4333-8333-333333333333", // 13:00Z, higher event_id
                "44444444-4444-4444-8444-444444444444", // next day
            ]
        );
    }

    #[test]
    fn malformed_fts_query_returns_typed_error_not_empty() {
        let db = seed_db();
        let err = search_events(
            db.connection(),
            &SearchQuery {
                text: Some("\"unterminated".into()),
                ..SearchQuery::default()
            },
        )
        .expect_err("must fail closed");
        match err {
            ProjectionError::InvalidQuery { detail } => {
                assert!(detail.to_ascii_lowercase().contains("fts") || detail.contains("invalid"));
            }
            other => panic!("expected InvalidQuery, got {other}"),
        }
    }

    #[test]
    fn empty_text_and_bad_created_at_bounds_are_invalid_query() {
        let db = seed_db();
        let err = search_events(
            db.connection(),
            &SearchQuery {
                text: Some(String::new()),
                ..SearchQuery::default()
            },
        )
        .unwrap_err();
        assert!(matches!(err, ProjectionError::InvalidQuery { .. }));

        let err = search_events(
            db.connection(),
            &SearchQuery {
                created_at_from: Some("yesterday".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap_err();
        assert!(matches!(err, ProjectionError::InvalidQuery { .. }));

        let err = search_events(
            db.connection(),
            &SearchQuery {
                created_at_from: Some("2026-10-07T00:00:00Z".into()),
                created_at_to: Some("2026-10-06T00:00:00Z".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap_err();
        assert!(matches!(err, ProjectionError::InvalidQuery { .. }));
    }

    #[test]
    fn limit_applies_after_deterministic_ordering() {
        let db = seed_db();
        let hits = search_events(
            db.connection(),
            &SearchQuery {
                limit: Some(2),
                ..SearchQuery::default()
            },
        )
        .unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].event_id, "11111111-1111-4111-8111-111111111111");
        assert_eq!(hits[1].event_id, "22222222-2222-4222-8222-222222222222");
    }

    fn seed_fractional_db() -> ProjectionDb {
        let db = ProjectionDb::open_in_memory().unwrap();
        let brain = "22222222-2222-4222-8222-222222222222";
        let rows = [
            note(
                "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                "device-a",
                1,
                "2026-10-06T12:00:00.100Z",
                Classification::Public,
                "frac early",
                &"aa".repeat(32),
                brain,
            ),
            note(
                "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
                "device-a",
                2,
                "2026-10-06T12:00:00.200Z",
                Classification::Public,
                "frac mid",
                &"bb".repeat(32),
                brain,
            ),
            note(
                "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
                "device-a",
                3,
                "2026-10-06T07:00:00.200-05:00", // == 12:00:00.200Z
                Classification::Public,
                "frac mid offset twin",
                &"cc".repeat(32),
                brain,
            ),
            note(
                "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
                "device-a",
                4,
                "2026-10-06T12:00:00.250Z",
                Classification::Public,
                "frac late",
                &"dd".repeat(32),
                brain,
            ),
            note(
                "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee",
                "device-a",
                5,
                "2026-10-06T12:00:00Z",
                Classification::Public,
                "frac whole second",
                &"ee".repeat(32),
                brain,
            ),
        ];
        for row in &rows {
            project_memory_note(db.connection(), row).unwrap();
        }
        db
    }

    #[test]
    fn fractional_inclusive_bounds_and_same_second_ordering() {
        let db = seed_fractional_db();

        // Inclusive lower/upper within the same second.
        let hits = search_events(
            db.connection(),
            &SearchQuery {
                created_at_from: Some("2026-10-06T12:00:00.100Z".into()),
                created_at_to: Some("2026-10-06T12:00:00.200Z".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap();
        let ids: Vec<_> = hits.iter().map(|h| h.event_id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa", // .100Z
                "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb", // .200Z
                "cccccccc-cccc-4ccc-8ccc-cccccccccccc", // offset twin of .200Z
            ]
        );

        // Strictly inside: exclude inclusive edges by using tighter bounds.
        let interior = search_events(
            db.connection(),
            &SearchQuery {
                created_at_from: Some("2026-10-06T12:00:00.101Z".into()),
                created_at_to: Some("2026-10-06T12:00:00.199Z".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap();
        assert!(interior.is_empty());

        // Offset-form fractional bound equivalent to .200Z.
        let offset_bound = search_events(
            db.connection(),
            &SearchQuery {
                created_at_from: Some("2026-10-06T07:00:00.200-05:00".into()),
                created_at_to: Some("2026-10-06T07:00:00.200-05:00".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap();
        assert_eq!(offset_bound.len(), 2);
        assert_eq!(
            offset_bound[0].event_id,
            "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"
        );
        assert_eq!(
            offset_bound[1].event_id,
            "cccccccc-cccc-4ccc-8ccc-cccccccccccc"
        );
    }

    #[test]
    fn timeline_pages_are_newest_first_with_exact_instant_ties_and_no_duplicates() {
        let db = seed_fractional_db();
        let first = timeline_events(
            db.connection(),
            &TimelineQuery {
                limit: 2,
                ..TimelineQuery::default()
            },
        )
        .unwrap();
        let first_ids: Vec<_> = first.hits.iter().map(|hit| hit.event_id.as_str()).collect();
        assert_eq!(
            first_ids,
            [
                "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
                "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
            ]
        );
        let second = timeline_events(
            db.connection(),
            &TimelineQuery {
                cursor: first.next_cursor,
                limit: 2,
                ..TimelineQuery::default()
            },
        )
        .unwrap();
        let second_ids: Vec<_> = second
            .hits
            .iter()
            .map(|hit| hit.event_id.as_str())
            .collect();
        assert_eq!(
            second_ids,
            [
                "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
                "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ]
        );
        let third = timeline_events(
            db.connection(),
            &TimelineQuery {
                cursor: second.next_cursor,
                limit: 2,
                ..TimelineQuery::default()
            },
        )
        .unwrap();
        assert_eq!(third.hits.len(), 1);
        assert_eq!(
            third.hits[0].event_id,
            "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee"
        );
        assert!(third.next_cursor.is_none());
    }

    #[test]
    fn timeline_rejects_invalid_bounds_and_cursor() {
        let db = seed_db();
        assert!(matches!(
            timeline_events(
                db.connection(),
                &TimelineQuery {
                    limit: 0,
                    ..TimelineQuery::default()
                }
            ),
            Err(ProjectionError::InvalidQuery { .. })
        ));
        assert!(matches!(
            timeline_events(
                db.connection(),
                &TimelineQuery {
                    created_at_from: Some("yesterday".into()),
                    limit: 10,
                    ..TimelineQuery::default()
                }
            ),
            Err(ProjectionError::InvalidQuery { .. })
        ));
        assert!(matches!(
            timeline_events(
                db.connection(),
                &TimelineQuery {
                    cursor: Some(TimelineCursor {
                        created_at: "not-a-time".into(),
                        event_id: "event".into(),
                    }),
                    limit: 10,
                    ..TimelineQuery::default()
                }
            ),
            Err(ProjectionError::InvalidQuery { .. })
        ));
    }

    #[test]
    fn fractional_ordering_then_event_id_tiebreak_and_limit() {
        let db = seed_fractional_db();
        let hits = search_events(db.connection(), &SearchQuery::default()).unwrap();
        let ids: Vec<_> = hits.iter().map(|h| h.event_id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee", // 12:00:00Z
                "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa", // .100Z
                "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb", // .200Z
                "cccccccc-cccc-4ccc-8ccc-cccccccccccc", // .200Z offset twin, higher event_id
                "dddddddd-dddd-4ddd-8ddd-dddddddddddd", // .250Z
            ]
        );

        let limited = search_events(
            db.connection(),
            &SearchQuery {
                limit: Some(3),
                ..SearchQuery::default()
            },
        )
        .unwrap();
        assert_eq!(
            limited
                .iter()
                .map(|h| h.event_id.as_str())
                .collect::<Vec<_>>(),
            [
                "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee",
                "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
            ]
        );
    }
}
