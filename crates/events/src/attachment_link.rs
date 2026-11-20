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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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

/// Strictly parsed metadata from an authenticated v2 envelope. The history
/// verifier separately checks the stream, Brain and locally trusted signer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedLinkFields {
    pub(crate) brain_id: String,
    classification: Classification,
    payload: LinkPayload,
}

impl SignedLinkFields {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, LinkError> {
        let envelope: EnvelopeFields =
            serde_json::from_slice(bytes).map_err(|_| LinkError::InvalidPayload)?;
        if envelope.event_type != EVENT_TYPE || envelope.payload_schema_version != 1 {
            return Err(LinkError::UnsupportedTypeOrVersion);
        }
        let payload: LinkPayload =
            serde_json::from_value(envelope.payload).map_err(|_| LinkError::InvalidPayload)?;
        cryptex_core::validate::parse_uuid_str(&payload.note_event_id)
            .map_err(|_| LinkError::InvalidPayload)?;
        Ok(Self {
            brain_id: envelope.brain_id,
            classification: envelope.classification,
            payload,
        })
    }

    pub fn note_event_id(&self) -> &str {
        &self.payload.note_event_id
    }

    pub fn bind<'a>(
        &self,
        note: &'a AcceptedEvent,
    ) -> Result<ResolvedAttachmentLink<'a>, LinkError> {
        self.bind_identity(
            &note.event_id,
            &note.brain_id,
            &note.content_hash,
            note.classification,
        )
    }

    /// A verified purge bridge can supply the same content-free binding fields.
    pub fn bind_identity<'a>(
        &self,
        event_id: &'a str,
        brain_id: &str,
        hash: &ContentHashHex,
        classification: Classification,
    ) -> Result<ResolvedAttachmentLink<'a>, LinkError> {
        if !self.note_event_id().eq_ignore_ascii_case(event_id) {
            return Err(LinkError::MissingNote);
        }
        if !self.brain_id.eq_ignore_ascii_case(brain_id) {
            return Err(LinkError::BrainMismatch);
        }
        if &self.payload.note_content_hash != hash {
            return Err(LinkError::NoteHashMismatch);
        }
        if self.classification != classification {
            return Err(LinkError::ClassificationMismatch);
        }
        Ok(ResolvedAttachmentLink {
            note_event_id: event_id,
            classification,
            blob_sha256: self
                .payload
                .blob_sha256
                .to_bytes()
                .map_err(|_| LinkError::InvalidPayload)?,
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
    let link = SignedLinkFields::parse(&record.raw_record)?;
    let note = history
        .records()
        .iter()
        .find_map(|record| match record {
            VersionedRecord::VerifiedV1(note)
                if note.event_id.eq_ignore_ascii_case(link.note_event_id()) =>
            {
                Some(note)
            }
            _ => None,
        })
        .ok_or(LinkError::MissingNote)?;
    link.bind(note)
}
