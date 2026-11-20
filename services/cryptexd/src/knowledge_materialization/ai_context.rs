//! Read-only daemon support for public note context. Client authorization and
//! adapter transport belong to M12. This reader is not an IPC endpoint.

use super::{GuardedSearchError, search_events_with_retrieval_safety_deadline};
use cryptex_core::DeviceId;
use cryptex_events::TrustedDaemonKeyResolver;
use cryptex_projection::{ProjectionDb, SearchQuery};
use serde::Serialize;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Serialize)]
pub struct PublicNoteExcerpt {
    pub event_id: String,
    pub text: String,
}

/// Build fresh context from verified, visible public notes in one Brain.
/// A verification failure propagates without returning any excerpts.
pub fn read_public_note_context(
    runtime_root: &Path,
    brain_id: &str,
    devices: &[DeviceId],
    resolver: &mut impl TrustedDaemonKeyResolver,
    db: &ProjectionDb,
    phrase: &str,
    deadline: Instant,
) -> Result<Vec<PublicNoteExcerpt>, GuardedSearchError> {
    let query = SearchQuery {
        text: Some(phrase.to_owned()),
        classification: Some("public".into()),
        event_type: Some("memory.note".into()),
        limit: Some(10),
        ..Default::default()
    };
    let hits = search_events_with_retrieval_safety_deadline(
        runtime_root,
        brain_id,
        devices,
        resolver,
        db,
        &query,
        Some(deadline),
    )?;
    Ok(hits
        .into_iter()
        .map(|hit| PublicNoteExcerpt {
            event_id: hit.event_id,
            text: hit.text,
        })
        .collect())
}
