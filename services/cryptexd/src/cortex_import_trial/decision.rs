//! Owner decision for one exact review in a retained disposable Brain.
use super::{LifecycleError, fault, provenance, selection::Comparison};
use cryptex_core::DeviceId;
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

const FILE: &str = "cortex-batch-decision.json";
const SCHEMA: &str = "cryptex.m15.disposable-batch-decision.v1";

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Choice {
    Accepted,
    Cancelled,
}

impl Choice {
    pub(super) fn from_flag(flag: &str) -> Option<Self> {
        match flag {
            "--accept-selection" => Some(Self::Accepted),
            "--cancel-selection" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchDecision {
    schema: String,
    brain_id: String,
    device_id: String,
    review_sha256: String,
    choice: Choice,
}

pub(super) fn review(
    root: &Path,
    key: &SigningKey,
    device: &DeviceId,
    brain_id: &str,
    comparison: Comparison,
    requested: Option<(Choice, &str)>,
) -> Result<Value, LifecycleError> {
    let path = root.join(FILE);
    let mut stored = if provenance::exists(&path)? {
        let saved: BatchDecision = provenance::read_signed(&path, &key.verifying_key())?;
        if saved.schema != SCHEMA
            || saved.brain_id != brain_id
            || saved.device_id != device.as_str()
        {
            return Err(fault("batch_decision_scope_mismatch"));
        }
        if comparison
            .review_sha256
            .as_ref()
            .is_some_and(|current| current != &saved.review_sha256)
        {
            return Err(fault("batch_decision_review_mismatch"));
        }
        Some(saved)
    } else {
        None
    };
    if let Some((choice, reviewed)) = requested {
        let current = comparison
            .review_sha256
            .as_deref()
            .ok_or_else(|| fault("selection_not_ready"))?;
        if current != reviewed {
            return Err(fault("stale_selection_review"));
        }
        let decision = BatchDecision {
            schema: SCHEMA.into(),
            brain_id: brain_id.into(),
            device_id: device.as_str().into(),
            review_sha256: current.into(),
            choice,
        };
        if stored.as_ref().is_some_and(|saved| saved != &decision) {
            return Err(fault("batch_decision_conflict"));
        }
        provenance::publish(root, root, FILE, &decision, key)?;
        stored = Some(decision);
    }
    let mut report = comparison.report;
    report["review_sha256"] = json!(comparison.review_sha256);
    report["ready_for_owner_review"] =
        json!(comparison.review_sha256.is_some() && stored.is_none());
    // A historical decision applies only to the exact comparison it names.
    report["batch_decision"] = json!(
        stored.filter(|saved| comparison.review_sha256.as_ref() == Some(&saved.review_sha256))
    );
    Ok(report)
}
