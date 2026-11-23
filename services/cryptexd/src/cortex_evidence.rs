//! Wire types shared by the import decision and the encrypted recovery reader.
use crate::capture::CaptureCommit;
use cryptex_events::{Classification, SignatureHex};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
pub const DIRECTORY: &str = "cortex-source-receipts";
pub const SCHEMA: &str = "cryptex.m15.disposable-source.v1";
pub const DECISION_FILE: &str = "cortex-batch-decision.json";
pub const DECISION_SCHEMA: &str = "cryptex.m15.disposable-batch-decision.v1";
pub const MAX_RECEIPT: u64 = 32 * 1024;
pub(crate) fn backup_member(name: &str) -> bool {
    name == DECISION_FILE
        || name.split_once('/').is_some_and(|(directory, file)| {
            directory == DIRECTORY
                && file.strip_suffix(".json").is_some_and(|id| {
                    cryptex_core::validate::parse_uuid_str(id).is_ok()
                        && id == id.to_ascii_lowercase()
                })
        })
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    pub capture_id: String,
    pub source_path: String,
    pub source_sha256: String,
    pub classification: Classification,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_sha256: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub schema: String,
    pub brain_id: String,
    pub device_id: String,
    pub event_id: String,
    pub content_hash: String,
    pub approval: Approval,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt<T> {
    pub source: T,
    pub signature: SignatureHex,
}

impl Source {
    pub fn from_commit(approval: &Approval, commit: &CaptureCommit) -> Self {
        Self {
            schema: SCHEMA.into(),
            brain_id: commit.brain_id.clone(),
            device_id: commit.device_id.clone(),
            event_id: commit.event_id.clone(),
            content_hash: commit.content_hash.clone(),
            approval: approval.clone(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Choice {
    Accepted,
    Cancelled,
}

impl Choice {
    pub fn from_flag(flag: &str) -> Option<Self> {
        match flag {
            "--accept-selection" => Some(Self::Accepted),
            "--cancel-selection" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchDecision {
    pub schema: String,
    pub brain_id: String,
    pub device_id: String,
    pub review_sha256: String,
    pub choice: Choice,
}

pub fn comparison_report(
    brain: &str,
    matched: &[Source],
    missing: &[Approval],
    extra: &[Source],
    changed: &[Value],
) -> Value {
    json!({
        "schema":"cryptex.cortex.selection-comparison.v1", "brain_id":brain,
        "ready_for_owner_review":!matched.is_empty() && missing.is_empty() && extra.is_empty() && changed.is_empty(),
        "selected_count":matched.len() + missing.len() + changed.len(),
        "verified_count":matched.len() + extra.len() + changed.len(),
        "matched":matched, "missing":missing, "extra":extra, "changed":changed
    })
}

pub fn review_digest(report: &Value) -> Result<String, serde_json::Error> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json_canonicalizer::to_vec(report)?)
    ))
}
