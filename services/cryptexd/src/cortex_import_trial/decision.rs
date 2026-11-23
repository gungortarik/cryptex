//! Owner decision for one exact review in a retained disposable Brain.
use super::{LifecycleError, fault, provenance, selection::Comparison};
use cryptex_core::DeviceId;
pub(super) use cryptexd::cortex_evidence::Choice;
use cryptexd::cortex_evidence::{BatchDecision, DECISION_FILE as FILE, DECISION_SCHEMA as SCHEMA};
use ed25519_dalek::SigningKey;
use serde_json::{Value, json};
use std::path::Path;

fn load_decision(
    root: &Path,
    key: &ed25519_dalek::VerifyingKey,
    device: &DeviceId,
    brain_id: &str,
    comparison: &Comparison,
) -> Result<Option<BatchDecision>, LifecycleError> {
    let path = root.join(FILE);
    if provenance::exists(&path)? {
        let saved: BatchDecision = provenance::read_signed(&path, key)?;
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
        Ok(Some(saved))
    } else {
        Ok(None)
    }
}

pub(super) fn require_accepted(
    root: &Path,
    key: &ed25519_dalek::VerifyingKey,
    device: &DeviceId,
    brain_id: &str,
    comparison: &Comparison,
) -> Result<(), LifecycleError> {
    let digest = comparison
        .review_sha256
        .as_ref()
        .ok_or_else(|| fault("selection_not_ready"))?;
    let saved = load_decision(root, key, device, brain_id, comparison)?
        .ok_or_else(|| fault("owner_decision_pending"))?;
    if saved.choice != Choice::Accepted || &saved.review_sha256 != digest {
        return Err(fault("batch_not_accepted"));
    }
    Ok(())
}

pub(super) fn review(
    root: &Path,
    key: &SigningKey,
    device: &DeviceId,
    brain_id: &str,
    comparison: Comparison,
    requested: Option<(Choice, &str)>,
) -> Result<Value, LifecycleError> {
    let mut stored = load_decision(root, &key.verifying_key(), device, brain_id, &comparison)?;
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
