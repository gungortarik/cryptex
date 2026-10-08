//! Library search over disposable projection rows (M3 Slice 6).
//!
//! Optional FTS5 text query plus conjunction filters. All bound values use
//! parameterized SQL. `created_at` ranges compare exact UTC instants per M2
//! RFC3339 rules (including offsets and fractional seconds); lexical string
//! ordering is never used for ranges.

use cryptex_core::validate::rfc3339_utc_instant;
use rusqlite::{Connection, params_from_iter};

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
    /// Optional maximum hits after ordering.
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

/// Search projected events.
///
/// Result order is deterministic:
/// 1. `created_at` ascending by exact UTC instant, including fractional seconds
/// 2. `event_id` ascending (lexicographic UUID string) only when instants are equal
///
/// Invalid/unsupported FTS queries and invalid RFC3339 bounds return
/// [`ProjectionError::InvalidQuery`] rather than an empty hit list.
pub fn search_events(conn: &Connection, query: &SearchQuery) -> ProjectionResult<Vec<SearchHit>> {
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

    let mut hits = Vec::new();
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
        hits.push((instant, hit));
    }

    hits.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.event_id.cmp(&b.1.event_id)));

    let mut out: Vec<SearchHit> = hits.into_iter().map(|(_, h)| h).collect();
    if let Some(limit) = query.limit {
        out.truncate(limit as usize);
    }
    Ok(out)
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
