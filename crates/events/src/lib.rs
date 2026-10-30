//! Cryptex immutable event foundations (M2).
//!
//! Authority: accepted ADR-0016. Canonical writes remain `cryptexd`-only.

pub mod bootstrap;
pub mod device_seq;
pub mod envelope;
pub mod error;
pub mod framing;
pub mod hex;
pub mod integrity;
pub mod limits;
pub mod schema;
pub mod security_history;
pub mod security_v3;
pub mod v2_payload;
pub mod validate_record;
pub mod versioned_reader;

pub use bootstrap::{
    BootstrapApprovalRole, BootstrapError, BootstrapIdentity, VerifiedBootstrapApprovalRequest,
    VerifiedBootstrapOffer, VerifiedBootstrapTranscript, create_bootstrap_offer,
    create_bootstrap_proof, parse_bootstrap_message, sign_bootstrap_approval,
    verify_bootstrap_approval_request, verify_bootstrap_offer, verify_bootstrap_transcript,
    verify_bootstrap_transcript_values,
};
pub use device_seq::{DEVICE_SEQ_PATTERN, DeviceSeq};
pub use envelope::{
    AcceptedEvent, Classification, EVENT_SCHEMA_VERSION, EventPayload, EventType,
    MemoryNotePayload, SignableEvent,
};
pub use error::{EventError, EventResult};
pub use framing::{
    FRAME_HEADER_BYTES, FRAME_MAGIC, FRAME_VERSION_V1, decode_frame_header, encode_frame_header,
};
pub use hex::{ContentHashHex, SignatureHex, SigningKeyId, ZERO_CONTENT_HASH_HEX};
pub use integrity::{
    CONTENT_HASH_DOMAIN, SIGNATURE_DOMAIN, accept_event, canonicalize_signable, check_chain_link,
    check_first_event_prev_hash, content_hash, content_hash_raw, reject_duplicate_json_keys,
    sign_content_hash, signing_key_id_from_verifying_key, verify_accepted_event,
    verify_content_hash_signature,
};
pub use limits::{
    DEFAULT_SEAL_EVENT_THRESHOLD_HINT, MAX_ACCEPTED_RECORD_BYTES, MAX_JSON_CONTAINER_ELEMENTS,
    MAX_JSON_NESTING_DEPTH, MAX_JSON_STRING_BYTES_PER_RECORD, MAX_MEMORY_NOTE_TEXT_BYTES,
    check_body_len,
};
pub use security_history::{
    AuthenticatedSecurityRecord, CancellableSecurityHistoryError, VerifiedSecurityHistory,
    verify_security_history, verify_security_history_cancellable,
};
pub use security_v3::{
    RestrictionReasonV3, SECURITY_V3_CONTENT_HASH_DOMAIN, SECURITY_V3_SCHEMA_VERSION,
    SECURITY_V3_SIGNATURE_DOMAIN, SecurityEventTypeV3, SecurityEventV3, SecurityPayloadV3,
    peer_approval_operation_digest, read_security_v3_event, sign_peer_approved_event,
};
pub use v2_payload::{
    AuthorizationBasis, Confidence, ConflictOutcome, EntityKind, FactValue,
    KnowledgeAssertionEventType, KnowledgeAssertionPayload, PublicationAuthorization,
    RequiredNullable, SemanticAssertion, V2KnowledgeAssertionEvent,
};
pub use validate_record::{
    ValidationFailure, ValidationReasonCode, validate_accepted_record_bytes,
};
pub use versioned_reader::{
    CancellableHistoryVerificationError, EVENT_SCHEMA_VERSION_V2, LocalTrustAnchor,
    TrustedDaemonKeyResolver, UnverifiableEnvelope, V2_CONTENT_HASH_DOMAIN, V2_SIGNATURE_DOMAIN,
    VerifiedUnsupportedV2, VerifiedV2History, VerifiedV2KnowledgeAssertion, VersionedChainVerifier,
    VersionedRecord, read_versioned_record, verify_v2_history, verify_v2_history_cancellable,
};
