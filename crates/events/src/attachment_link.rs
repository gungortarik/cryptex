//! Read-first M9 codec. Resolving a signed reference proves its binding to a
//! note in the supplied authenticated history. It grants no retrieval rights.
use crate::{
    AcceptedEvent, Classification, ContentHashHex, EventResult, VerifiedV2History, VersionedRecord,
};
use serde::{Deserialize, Serialize};

pub const EVENT_TYPE: &str = "attachment.link";

#[derive(Debug, PartialEq, Eq)]
pub enum LinkError {
    MissingLink,
    UnsupportedTypeOrVersion,
    InvalidPayload,
    MissingNote,
    NoteHashMismatch,
    BrainMismatch,
    ClassificationMismatch,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ResolvedAttachmentLink<'a> {
    pub note_event_id: &'a str,
    pub classification: Classification,
    pub blob_sha256: [u8; 32],
}

// The versioned reader has already authenticated and strictly decoded the
// common envelope. Only the fields needed by this extension are projected.
#[derive(Deserialize)]
struct EnvelopeFields {
    event_type: String,
    payload_schema_version: u32,
    brain_id: String,
    classification: Classification,
    payload: serde_json::Value,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LinkPayload {
    note_event_id: String,
    note_content_hash: ContentHashHex,
    blob_sha256: ContentHashHex,
}

impl LinkPayload {
    /// Prepare only the reference fields. Signing and canonical append belong
    /// to the daemon, after it verifies the target note and blob.
    pub fn for_note(note: &AcceptedEvent, blob_sha256: &[u8; 32]) -> EventResult<Self> {
        Ok(Self {
            note_event_id: note.event_id.clone(),
            note_content_hash: note.content_hash.clone(),
            blob_sha256: ContentHashHex::from_bytes(blob_sha256)?,
        })
    }
}

/// Look up both records in the same verified snapshot. UUIDs have the same
/// case-insensitive identity semantics as the history verifier.
pub fn resolve<'a>(
    history: &'a VerifiedV2History,
    link_event_id: &str,
) -> Result<ResolvedAttachmentLink<'a>, LinkError> {
    let record = history
        .records()
        .iter()
        .find_map(|record| match record {
            VersionedRecord::VerifiedUnsupported(record)
                if record.event_id.eq_ignore_ascii_case(link_event_id) =>
            {
                Some(record)
            }
            _ => None,
        })
        .ok_or(LinkError::MissingLink)?;
    let envelope: EnvelopeFields =
        serde_json::from_slice(&record.raw_record).map_err(|_| LinkError::InvalidPayload)?;
    if envelope.event_type != EVENT_TYPE || envelope.payload_schema_version != 1 {
        return Err(LinkError::UnsupportedTypeOrVersion);
    }
    let payload: LinkPayload =
        serde_json::from_value(envelope.payload).map_err(|_| LinkError::InvalidPayload)?;
    cryptex_core::validate::parse_uuid_str(&payload.note_event_id)
        .map_err(|_| LinkError::InvalidPayload)?;
    let note = history
        .records()
        .iter()
        .find_map(|record| match record {
            VersionedRecord::VerifiedV1(note)
                if note.event_id.eq_ignore_ascii_case(&payload.note_event_id) =>
            {
                Some(note)
            }
            _ => None,
        })
        .ok_or(LinkError::MissingNote)?;
    if !envelope.brain_id.eq_ignore_ascii_case(&note.brain_id) {
        return Err(LinkError::BrainMismatch);
    }
    if payload.note_content_hash != note.content_hash {
        return Err(LinkError::NoteHashMismatch);
    }
    if envelope.classification != note.classification {
        return Err(LinkError::ClassificationMismatch);
    }
    Ok(ResolvedAttachmentLink {
        note_event_id: &note.event_id,
        classification: note.classification,
        blob_sha256: payload
            .blob_sha256
            .to_bytes()
            .map_err(|_| LinkError::InvalidPayload)?,
    })
}
