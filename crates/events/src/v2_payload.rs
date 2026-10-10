//! Strict ADR-0017 v2 `knowledge.assertion` payload model and validation.
//!
//! This is a read/validation model only. It does not expose a canonical writer.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

use cryptex_core::validate::parse_uuid_str;

use crate::{EventError, EventResult};

pub const MAX_LABEL_BYTES: usize = 256;
pub const MAX_ALIAS_BYTES: usize = 256;
pub const MAX_STRUCTURED_TEXT_BYTES: usize = 524_288;
pub const MAX_ENTITY_MERGE_IDS: usize = 256;
pub const MAX_CONFLICT_DECISION_IDS: usize = 4096;
pub const MAX_GENERAL_STRING_BYTES: usize = 1_048_576;
pub const UUID_PATTERN: &str =
    r"^[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}$";
pub const PREDICATE_PATTERN: &str = r"^[a-z][a-z0-9._-]{0,63}$";
pub const DECIMAL_PATTERN: &str = r"^-?(0|[1-9][0-9]*)(\.[0-9]*[1-9])?$";
pub const DATE_PATTERN: &str = r"^[0-9]{4}-[0-9]{2}-[0-9]{2}$";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Person,
    Project,
    Course,
    Device,
    Organization,
    Concept,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Confirmed,
    Inferred,
    Uncertain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationBasis {
    DirectUser,
    UserApprovedAiCapture,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PublicationAuthorization {
    pub basis: AuthorizationBasis,
    #[schemars(regex(pattern = UUID_PATTERN), length(min = 36, max = 36))]
    pub receipt_id: String,
}

impl PublicationAuthorization {
    fn validate(&self) -> EventResult<()> {
        validate_uuid("publication_authorization.receipt_id", &self.receipt_id)
    }

    fn permits(&self, confidence: Confidence) -> bool {
        self.basis == AuthorizationBasis::DirectUser || confidence != Confidence::Confirmed
    }
}

/// Fact values are tagged, exact-shape unions; no floating-point values exist.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value", deny_unknown_fields)]
pub enum FactValue {
    #[serde(rename = "text")]
    Text(#[schemars(length(max = MAX_STRUCTURED_TEXT_BYTES))] String),
    #[serde(rename = "decimal")]
    Decimal(
        #[schemars(regex(pattern = DECIMAL_PATTERN), length(max = MAX_GENERAL_STRING_BYTES))]
        String,
    ),
    #[serde(rename = "boolean")]
    Boolean(bool),
    #[serde(rename = "date")]
    Date(#[schemars(regex(pattern = DATE_PATTERN), length(equal = 10))] String),
    #[serde(rename = "entity_ref")]
    EntityRef(#[schemars(regex(pattern = UUID_PATTERN), length(equal = 36))] String),
}

impl FactValue {
    fn validate(&self) -> EventResult<()> {
        match self {
            Self::Text(value) => validate_nfc_text("fact value", value, MAX_STRUCTURED_TEXT_BYTES),
            Self::Decimal(value) if !is_canonical_decimal(value) => {
                invalid("decimal must use the canonical non-exponent string form")
            }
            Self::Decimal(_) => Ok(()),
            Self::Boolean(_) => Ok(()),
            Self::Date(value) if !is_valid_gregorian_date(value) => {
                invalid("date must be a valid Gregorian YYYY-MM-DD value")
            }
            Self::Date(_) => Ok(()),
            Self::EntityRef(value) => validate_uuid("fact entity_ref", value),
        }
    }
}

/// Semantic replacement data for correction and conflict-resolution events.
/// `kind` distinguishes the assertion's semantic shape, not a new event type.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum SemanticAssertion {
    #[serde(rename = "fact")]
    Fact {
        #[schemars(regex(pattern = UUID_PATTERN), length(min = 36, max = 36))]
        subject_entity_id: String,
        #[schemars(regex(pattern = PREDICATE_PATTERN), length(min = 1, max = 64))]
        predicate: String,
        value: FactValue,
        confidence: Confidence,
    },
    #[serde(rename = "relation")]
    Relation {
        #[schemars(regex(pattern = UUID_PATTERN), length(min = 36, max = 36))]
        subject_entity_id: String,
        #[schemars(regex(pattern = PREDICATE_PATTERN), length(min = 1, max = 64))]
        predicate: String,
        #[schemars(regex(pattern = UUID_PATTERN), length(min = 36, max = 36))]
        object_entity_id: String,
        confidence: Confidence,
    },
}

impl SemanticAssertion {
    fn validate(&self, authorization: &PublicationAuthorization) -> EventResult<()> {
        match self {
            Self::Fact {
                subject_entity_id,
                predicate,
                value,
                confidence,
            } => {
                validate_uuid("subject_entity_id", subject_entity_id)?;
                validate_predicate(predicate)?;
                value.validate()?;
                validate_authorized_confidence(authorization, *confidence)
            }
            Self::Relation {
                subject_entity_id,
                predicate,
                object_entity_id,
                confidence,
            } => {
                validate_uuid("subject_entity_id", subject_entity_id)?;
                validate_predicate(predicate)?;
                validate_uuid("object_entity_id", object_entity_id)?;
                validate_authorized_confidence(authorization, *confidence)
            }
        }
    }
}

/// Required JSON property whose value is either `null` or a typed assertion.
/// Unlike `Option<T>`, this wrapper remains a required struct property.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
#[schemars(transparent)]
pub struct RequiredNullable<T>(pub Option<T>);

impl<'de, T> Deserialize<'de> for RequiredNullable<T>
where
    T: serde::de::DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        if value.is_null() {
            Ok(Self(None))
        } else {
            T::deserialize(value)
                .map(Some)
                .map(Self)
                .map_err(serde::de::Error::custom)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum ConflictOutcome {
    #[serde(rename = "select")]
    Select {
        #[schemars(regex(pattern = UUID_PATTERN), length(equal = 36))]
        decision_event_id: String,
    },
    #[serde(rename = "replace")]
    Replace { value: SemanticAssertion },
    #[serde(rename = "withdraw")]
    Withdraw,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum KnowledgeAssertionPayload {
    #[serde(rename = "entity.declare")]
    EntityDeclare {
        #[schemars(regex(pattern = UUID_PATTERN), length(min = 36, max = 36))]
        entity_id: String,
        entity_kind: EntityKind,
        #[schemars(length(min = 1, max = MAX_LABEL_BYTES))]
        label: String,
        publication_authorization: PublicationAuthorization,
    },
    #[serde(rename = "entity.alias")]
    EntityAlias {
        #[schemars(regex(pattern = UUID_PATTERN), length(min = 36, max = 36))]
        entity_id: String,
        #[schemars(length(min = 1, max = MAX_ALIAS_BYTES))]
        alias: String,
        publication_authorization: PublicationAuthorization,
    },
    #[serde(rename = "fact.assert")]
    FactAssert {
        #[schemars(regex(pattern = UUID_PATTERN), length(min = 36, max = 36))]
        subject_entity_id: String,
        #[schemars(regex(pattern = PREDICATE_PATTERN), length(min = 1, max = 64))]
        predicate: String,
        value: FactValue,
        confidence: Confidence,
        publication_authorization: PublicationAuthorization,
    },
    #[serde(rename = "relation.assert")]
    RelationAssert {
        #[schemars(regex(pattern = UUID_PATTERN), length(min = 36, max = 36))]
        subject_entity_id: String,
        #[schemars(regex(pattern = PREDICATE_PATTERN), length(min = 1, max = 64))]
        predicate: String,
        #[schemars(regex(pattern = UUID_PATTERN), length(min = 36, max = 36))]
        object_entity_id: String,
        confidence: Confidence,
        publication_authorization: PublicationAuthorization,
    },
    #[serde(rename = "assertion.correct")]
    AssertionCorrect {
        #[schemars(regex(pattern = UUID_PATTERN), length(min = 36, max = 36))]
        target_event_id: String,
        replacement: RequiredNullable<SemanticAssertion>,
        publication_authorization: PublicationAuthorization,
    },
    #[serde(rename = "entity.merge")]
    EntityMerge {
        #[schemars(
            length(min = 2, max = MAX_ENTITY_MERGE_IDS),
            inner(regex(pattern = UUID_PATTERN), length(equal = 36))
        )]
        entity_ids: Vec<String>,
        publication_authorization: PublicationAuthorization,
    },
    #[serde(rename = "entity.merge_revoke")]
    EntityMergeRevoke {
        #[schemars(regex(pattern = UUID_PATTERN), length(min = 36, max = 36))]
        target_merge_event_id: String,
        publication_authorization: PublicationAuthorization,
    },
    #[serde(rename = "assertion.resolve_conflict")]
    AssertionResolveConflict {
        #[schemars(regex(pattern = UUID_PATTERN), length(min = 36, max = 36))]
        target_event_id: String,
        #[schemars(
            length(min = 2, max = MAX_CONFLICT_DECISION_IDS),
            inner(regex(pattern = UUID_PATTERN), length(equal = 36))
        )]
        decision_event_ids: Vec<String>,
        outcome: ConflictOutcome,
        publication_authorization: PublicationAuthorization,
    },
}

impl KnowledgeAssertionPayload {
    pub fn parse_and_validate(value: Value) -> EventResult<Self> {
        let payload: Self = serde_json::from_value(value)
            .map_err(|_| EventError::Invalid("known v2 payload has invalid shape".into()))?;
        payload.validate()?;
        Ok(payload)
    }

    pub fn validate(&self) -> EventResult<()> {
        match self {
            Self::EntityDeclare {
                entity_id,
                label,
                publication_authorization,
                ..
            } => {
                validate_uuid("entity_id", entity_id)?;
                validate_label("entity label", label, MAX_LABEL_BYTES)?;
                publication_authorization.validate()
            }
            Self::EntityAlias {
                entity_id,
                alias,
                publication_authorization,
            } => {
                validate_uuid("entity_id", entity_id)?;
                validate_label("entity alias", alias, MAX_ALIAS_BYTES)?;
                publication_authorization.validate()
            }
            Self::FactAssert {
                subject_entity_id,
                predicate,
                value,
                confidence,
                publication_authorization,
            } => {
                validate_uuid("subject_entity_id", subject_entity_id)?;
                validate_predicate(predicate)?;
                value.validate()?;
                validate_authorized_confidence(publication_authorization, *confidence)
            }
            Self::RelationAssert {
                subject_entity_id,
                predicate,
                object_entity_id,
                confidence,
                publication_authorization,
            } => {
                validate_uuid("subject_entity_id", subject_entity_id)?;
                validate_predicate(predicate)?;
                validate_uuid("object_entity_id", object_entity_id)?;
                validate_authorized_confidence(publication_authorization, *confidence)
            }
            Self::AssertionCorrect {
                target_event_id,
                replacement,
                publication_authorization,
            } => {
                validate_uuid("target_event_id", target_event_id)?;
                publication_authorization.validate()?;
                if let Some(value) = &replacement.0 {
                    value.validate(publication_authorization)?;
                }
                Ok(())
            }
            Self::EntityMerge {
                entity_ids,
                publication_authorization,
            } => {
                validate_sorted_uuid_list("entity_ids", entity_ids, 2, MAX_ENTITY_MERGE_IDS)?;
                publication_authorization.validate()
            }
            Self::EntityMergeRevoke {
                target_merge_event_id,
                publication_authorization,
            } => {
                validate_uuid("target_merge_event_id", target_merge_event_id)?;
                publication_authorization.validate()
            }
            Self::AssertionResolveConflict {
                target_event_id,
                decision_event_ids,
                outcome,
                publication_authorization,
            } => {
                validate_uuid("target_event_id", target_event_id)?;
                validate_sorted_uuid_list(
                    "decision_event_ids",
                    decision_event_ids,
                    2,
                    MAX_CONFLICT_DECISION_IDS,
                )?;
                publication_authorization.validate()?;
                match outcome {
                    ConflictOutcome::Select { decision_event_id } => {
                        validate_uuid("outcome.decision_event_id", decision_event_id)?;
                        if !decision_event_ids
                            .iter()
                            .any(|candidate| candidate.eq_ignore_ascii_case(decision_event_id))
                        {
                            return invalid(
                                "selected decision_event_id must appear in decision_event_ids",
                            );
                        }
                        Ok(())
                    }
                    ConflictOutcome::Replace { value } => value.validate(publication_authorization),
                    ConflictOutcome::Withdraw => Ok(()),
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
pub enum KnowledgeAssertionEventType {
    #[serde(rename = "knowledge.assertion")]
    #[schemars(rename = "knowledge.assertion")]
    KnowledgeAssertion,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct V2KnowledgeAssertionEvent {
    #[schemars(range(min = 2, max = 2))]
    pub schema_version: u32,
    #[schemars(regex(pattern = UUID_PATTERN), length(min = 36, max = 36))]
    pub event_id: String,
    #[schemars(regex(pattern = UUID_PATTERN), length(min = 36, max = 36))]
    pub brain_id: String,
    #[schemars(regex(pattern = r"^[A-Za-z0-9_-]{1,64}$"), length(min = 1, max = 64))]
    pub device_id: String,
    pub device_seq: crate::device_seq::DeviceSeq,
    #[schemars(length(max = MAX_GENERAL_STRING_BYTES))]
    pub created_at: String,
    pub event_type: KnowledgeAssertionEventType,
    #[schemars(range(min = 1, max = 1))]
    pub payload_schema_version: u32,
    pub classification: crate::envelope::Classification,
    pub signing_key_id: crate::hex::SigningKeyId,
    pub payload: KnowledgeAssertionPayload,
    pub prev_content_hash: crate::hex::ContentHashHex,
    pub content_hash: crate::hex::ContentHashHex,
    pub signature: crate::hex::SignatureHex,
}

impl V2KnowledgeAssertionEvent {
    pub fn validate(&self) -> EventResult<()> {
        if self.schema_version != 2 || self.payload_schema_version != 1 {
            return invalid("known v2 assertion requires schema_version 2 and payload schema 1");
        }
        validate_uuid("event_id", &self.event_id)?;
        validate_uuid("brain_id", &self.brain_id)?;
        cryptex_core::ids::DeviceId::new(self.device_id.clone())
            .map_err(|_| EventError::Invalid("v2 device_id is invalid".into()))?;
        self.device_seq
            .to_u64()
            .map_err(|_| EventError::Invalid("v2 device_seq is invalid".into()))?;
        cryptex_core::validate::parse_rfc3339_str(&self.created_at)
            .map_err(|_| EventError::Invalid("v2 created_at is invalid".into()))?;
        self.payload.validate()
    }
}

fn validate_authorized_confidence(
    authorization: &PublicationAuthorization,
    confidence: Confidence,
) -> EventResult<()> {
    authorization.validate()?;
    if !authorization.permits(confidence) {
        return invalid("confirmed confidence requires direct_user authorization");
    }
    Ok(())
}

fn validate_label(name: &str, value: &str, max_bytes: usize) -> EventResult<()> {
    validate_nfc_text(name, value, max_bytes)?;
    if value.trim().is_empty() {
        return invalid("label or alias is empty after Unicode whitespace trimming");
    }
    Ok(())
}

fn validate_nfc_text(name: &str, value: &str, max_bytes: usize) -> EventResult<()> {
    if value.len() > max_bytes || value.len() > MAX_GENERAL_STRING_BYTES {
        return invalid("text exceeds the accepted UTF-8 byte limit");
    }
    if !value.nfc().eq(value.chars()) {
        return invalid("string is not NFC-normalized under Unicode 16.0.0");
    }
    let _ = name;
    Ok(())
}

fn validate_uuid(name: &str, value: &str) -> EventResult<()> {
    parse_uuid_str(value)
        .map_err(|_| EventError::Invalid(format!("{name} is not a valid UUID")))?;
    Ok(())
}

fn validate_predicate(value: &str) -> EventResult<()> {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 64
        || !bytes[0].is_ascii_lowercase()
        || !bytes.iter().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(*b, b'.' | b'_' | b'-')
        })
    {
        return invalid("predicate must match [a-z][a-z0-9._-]{0,63}");
    }
    Ok(())
}

fn validate_sorted_uuid_list(
    name: &str,
    values: &[String],
    min: usize,
    max: usize,
) -> EventResult<()> {
    if values.len() < min || values.len() > max {
        return invalid("UUID list count is outside the accepted bounds");
    }
    let mut previous: Option<String> = None;
    for value in values {
        validate_uuid(name, value)?;
        let normalized = value.to_ascii_lowercase();
        if previous.as_ref().is_some_and(|prior| prior >= &normalized) {
            return invalid("UUID lists must be sorted and unique");
        }
        previous = Some(normalized);
    }
    Ok(())
}

pub fn is_canonical_decimal(value: &str) -> bool {
    if value.is_empty() || value == "-0" || value.starts_with('+') {
        return false;
    }
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    let mut parts = unsigned.split('.');
    let integer = parts.next().unwrap_or_default();
    let valid_integer = integer == "0"
        || (!integer.is_empty()
            && integer.as_bytes()[0].is_ascii_digit()
            && integer.as_bytes()[0] != b'0'
            && integer.bytes().all(|b| b.is_ascii_digit()));
    if !valid_integer {
        return false;
    }
    match (parts.next(), parts.next()) {
        (None, None) => true,
        (Some(fraction), None) => {
            !fraction.is_empty()
                && !fraction.ends_with('0')
                && fraction.bytes().all(|b| b.is_ascii_digit())
        }
        _ => false,
    }
}

pub fn is_valid_gregorian_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(i, b)| matches!(i, 4 | 7) || b.is_ascii_digit())
    {
        return false;
    }
    let parse = |start, end| value[start..end].parse::<u32>().ok();
    let (Some(year), Some(month), Some(day)) = (parse(0, 4), parse(5, 7), parse(8, 10)) else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (year > 0) && (1..=days).contains(&day)
}

fn invalid<T>(message: &str) -> EventResult<T> {
    Err(EventError::Invalid(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID_A: &str = "11111111-1111-4111-8111-111111111111";
    const ID_B: &str = "22222222-2222-4222-8222-222222222222";
    const ID_C: &str = "33333333-3333-4333-8333-333333333333";

    fn auth(basis: &str) -> Value {
        serde_json::json!({"basis":basis,"receipt_id":ID_C})
    }

    fn parse(payload: Value) -> EventResult<KnowledgeAssertionPayload> {
        KnowledgeAssertionPayload::parse_and_validate(payload)
    }

    #[test]
    fn all_initial_payload_kinds_validate_with_synthetic_values() {
        let direct = auth("direct_user");
        let valid = [
            serde_json::json!({"kind":"entity.declare","entity_id":ID_A,"entity_kind":"person","label":"Ada","publication_authorization":direct}),
            serde_json::json!({"kind":"entity.alias","entity_id":ID_A,"alias":"Ada Lovelace","publication_authorization":direct}),
            serde_json::json!({"kind":"fact.assert","subject_entity_id":ID_A,"predicate":"person.birth_date","value":{"kind":"date","value":"1815-12-10"},"confidence":"confirmed","publication_authorization":direct}),
            serde_json::json!({"kind":"relation.assert","subject_entity_id":ID_A,"predicate":"works_with","object_entity_id":ID_B,"confidence":"inferred","publication_authorization":direct}),
            serde_json::json!({"kind":"assertion.correct","target_event_id":ID_B,"replacement":null,"publication_authorization":direct}),
            serde_json::json!({"kind":"entity.merge","entity_ids":[ID_A,ID_B],"publication_authorization":direct}),
            serde_json::json!({"kind":"entity.merge_revoke","target_merge_event_id":ID_B,"publication_authorization":direct}),
            serde_json::json!({"kind":"assertion.resolve_conflict","target_event_id":ID_A,"decision_event_ids":[ID_B,ID_C],"outcome":{"kind":"withdraw"},"publication_authorization":direct}),
        ];
        assert_eq!(valid.len(), 8);
        for payload in valid {
            parse(payload).expect("synthetic accepted payload");
        }
    }

    #[test]
    fn confidence_authorization_and_unknown_fields_fail_closed() {
        let ai = auth("user_approved_ai_capture");
        assert!(parse(serde_json::json!({"kind":"fact.assert","subject_entity_id":ID_A,"predicate":"x","value":{"kind":"boolean","value":true},"confidence":"confirmed","publication_authorization":ai})).is_err());
        assert!(parse(serde_json::json!({"kind":"entity.declare","entity_id":ID_A,"entity_kind":"person","label":"Ada","extra":true,"publication_authorization":auth("direct_user")})).is_err());
        assert!(parse(serde_json::json!({"kind":"entity.declare","entity_id":ID_A,"entity_kind":"brain","label":"Ada","publication_authorization":auth("direct_user")})).is_err());
    }

    #[test]
    fn decimal_date_nfc_limits_and_predicate_contract_are_enforced() {
        for accepted in [
            "0",
            "-1",
            "1.01",
            "-0.01",
            "100000000000000000000000000000000000",
        ] {
            assert!(is_canonical_decimal(accepted), "{accepted}");
        }
        for rejected in ["-0", "00", "01", "+1", "1.0", "1.10", "1e3", ".1", "1."] {
            assert!(!is_canonical_decimal(rejected), "{rejected}");
        }
        for accepted in ["2000-02-29", "2024-02-29"] {
            assert!(is_valid_gregorian_date(accepted), "{accepted}");
        }
        for rejected in [
            "0000-02-29",
            "1900-02-29",
            "2023-02-29",
            "2024-13-01",
            "2024-1-01",
        ] {
            assert!(!is_valid_gregorian_date(rejected), "{rejected}");
        }
        assert!(parse(serde_json::json!({"kind":"fact.assert","subject_entity_id":ID_A,"predicate":"x","value":{"kind":"text","value":"Cafe\u{301}"},"confidence":"inferred","publication_authorization":auth("direct_user")})).is_err());
        assert!(parse(serde_json::json!({"kind":"entity.alias","entity_id":ID_A,"alias":"  \t ","publication_authorization":auth("direct_user")})).is_err());
        assert!(parse(serde_json::json!({"kind":"fact.assert","subject_entity_id":ID_A,"predicate":"Upper","value":{"kind":"boolean","value":true},"confidence":"inferred","publication_authorization":auth("direct_user")})).is_err());
    }

    #[test]
    fn merge_and_resolution_lists_must_be_sorted_unique_and_bounded() {
        assert!(parse(serde_json::json!({"kind":"entity.merge","entity_ids":[ID_B,ID_A],"publication_authorization":auth("direct_user")})).is_err());
        assert!(parse(serde_json::json!({"kind":"entity.merge","entity_ids":[ID_A,ID_A],"publication_authorization":auth("direct_user")})).is_err());
        assert!(parse(serde_json::json!({"kind":"assertion.resolve_conflict","target_event_id":ID_A,"decision_event_ids":[ID_B],"outcome":{"kind":"withdraw"},"publication_authorization":auth("direct_user")})).is_err());
        assert!(parse(serde_json::json!({"kind":"assertion.resolve_conflict","target_event_id":ID_A,"decision_event_ids":[ID_B,ID_C],"outcome":{"kind":"select","decision_event_id":ID_A},"publication_authorization":auth("direct_user")})).is_err());
        assert!(parse(serde_json::json!({"kind":"assertion.resolve_conflict","target_event_id":ID_A,"decision_event_ids":[ID_B,ID_C],"outcome":{"kind":"select","decision_event_id":ID_B},"publication_authorization":auth("direct_user")})).is_ok());
    }

    #[test]
    fn correction_replacement_is_required_and_validated_as_fact_or_relation() {
        assert!(
            parse(serde_json::json!({
                "kind":"assertion.correct",
                "target_event_id":ID_A,
                "publication_authorization":auth("direct_user")
            }))
            .is_err()
        );
        assert!(parse(serde_json::json!({
            "kind":"assertion.correct",
            "target_event_id":ID_A,
            "replacement":{"kind":"fact","subject_entity_id":ID_B,"predicate":"x","value":{"kind":"text","value":"replacement"},"confidence":"confirmed"},
            "publication_authorization":auth("direct_user")
        })).is_ok());
        assert!(parse(serde_json::json!({
            "kind":"assertion.correct",
            "target_event_id":ID_A,
            "replacement":{"kind":"fact","subject_entity_id":ID_B,"predicate":"x","value":{"kind":"text","value":"replacement"},"confidence":"likely"},
            "publication_authorization":auth("direct_user")
        })).is_err());
    }

    #[test]
    fn every_fact_value_variant_is_typed_and_bounded() {
        for value in [
            serde_json::json!({"kind":"text","value":"valid NFC text"}),
            serde_json::json!({"kind":"decimal","value":"-12.03"}),
            serde_json::json!({"kind":"boolean","value":false}),
            serde_json::json!({"kind":"date","value":"2024-02-29"}),
            serde_json::json!({"kind":"entity_ref","value":ID_B}),
        ] {
            assert!(
                parse(serde_json::json!({
                    "kind":"fact.assert",
                    "subject_entity_id":ID_A,
                    "predicate":"x",
                    "value":value,
                    "confidence":"inferred",
                    "publication_authorization":auth("direct_user")
                }))
                .is_ok()
            );
        }
        assert!(
            parse(serde_json::json!({
                "kind":"fact.assert",
                "subject_entity_id":ID_A,
                "predicate":"x",
                "value":{"kind":"boolean","value":"false"},
                "confidence":"inferred",
                "publication_authorization":auth("direct_user")
            }))
            .is_err()
        );
        assert!(
            parse(serde_json::json!({
                "kind":"fact.assert",
                "subject_entity_id":ID_A,
                "predicate":"x",
                "value":{"kind":"date","value":"2023-02-29"},
                "confidence":"inferred",
                "publication_authorization":auth("direct_user")
            }))
            .is_err()
        );
    }

    #[test]
    fn label_and_structured_text_limits_are_utf8_byte_limits() {
        let accepted_label = "é".repeat(MAX_LABEL_BYTES / 2);
        assert!(
            parse(serde_json::json!({
                "kind":"entity.declare",
                "entity_id":ID_A,
                "entity_kind":"person",
                "label":accepted_label,
                "publication_authorization":auth("direct_user")
            }))
            .is_ok()
        );
        let rejected_label = "é".repeat((MAX_LABEL_BYTES / 2) + 1);
        assert!(
            parse(serde_json::json!({
                "kind":"entity.declare",
                "entity_id":ID_A,
                "entity_kind":"person",
                "label":rejected_label,
                "publication_authorization":auth("direct_user")
            }))
            .is_err()
        );

        let max_text = "x".repeat(MAX_STRUCTURED_TEXT_BYTES);
        assert!(
            parse(serde_json::json!({
                "kind":"fact.assert",
                "subject_entity_id":ID_A,
                "predicate":"x",
                "value":{"kind":"text","value":max_text},
                "confidence":"inferred",
                "publication_authorization":auth("direct_user")
            }))
            .is_ok()
        );
        let oversized_text = "x".repeat(MAX_STRUCTURED_TEXT_BYTES + 1);
        assert!(
            parse(serde_json::json!({
                "kind":"fact.assert",
                "subject_entity_id":ID_A,
                "predicate":"x",
                "value":{"kind":"text","value":oversized_text},
                "confidence":"inferred",
                "publication_authorization":auth("direct_user")
            }))
            .is_err()
        );
    }
}
