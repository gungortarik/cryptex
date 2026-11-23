//! Reconcile the owner's separate selection with the verified disposable batch.
use super::{Approval, LifecycleError, fault, provenance::Source};
use cryptex_core::validate::parse_uuid_str;
use cryptex_cortex_inventory::mapped_note_name;
use cryptex_events::ContentHashHex;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    io::Read,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Selection {
    schema: String,
    brain_id: String,
    selected: Vec<Approval>,
}

impl Selection {
    pub(super) fn read(input: impl Read, brain_id: &str) -> Result<Self, LifecycleError> {
        const MAX_BYTES: u64 = 8 * 1024 * 1024;
        let mut bytes = Vec::new();
        input.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(fault("selection_too_large"));
        }
        let mut selection: Self =
            serde_json::from_slice(&bytes).map_err(|_| fault("invalid_selection"))?;
        if selection.schema != "cryptex.cortex.selection.v1" || selection.brain_id != brain_id {
            return Err(fault("selection_scope_mismatch"));
        }
        let mut ids = HashSet::new();
        let mut paths = HashSet::new();
        let mut digests = HashSet::new();
        for item in &selection.selected {
            if parse_uuid_str(&item.capture_id).is_err()
                || mapped_note_name(&item.source_path).is_err()
                || ContentHashHex::parse(&item.source_sha256).is_err()
                || item
                    .review_sha256
                    .as_ref()
                    .is_some_and(|hash| ContentHashHex::parse(hash).is_err())
            {
                return Err(fault("invalid_selection"));
            }
            if !ids.insert(item.capture_id.to_ascii_lowercase())
                || !paths.insert(&item.source_path)
                || !digests.insert(&item.source_sha256)
            {
                return Err(fault("ambiguous_selection"));
            }
        }
        selection
            .selected
            .sort_by(|a, b| a.capture_id.cmp(&b.capture_id));
        Ok(selection)
    }

    pub(super) fn compare(self, sources: Vec<Source>) -> Result<Comparison, LifecycleError> {
        let mut receipts = BTreeMap::new();
        let selected_count = self.selected.len();
        let receipt_count = sources.len();
        for source in sources {
            if receipts
                .insert(source.approval.capture_id.clone(), source)
                .is_some()
            {
                return Err(fault("ambiguous_source_receipt"));
            }
        }
        let mut matched = Vec::new();
        let mut missing = Vec::new();
        let mut changed = Vec::new();
        for decision in self.selected {
            match receipts.remove(&decision.capture_id) {
                None => missing.push(decision),
                Some(source) if source.approval == decision => matched.push(source),
                Some(source) => changed.push(json!({"selected":decision, "verified":source})),
            }
        }
        let extra: Vec<_> = receipts.into_values().collect();
        let ready =
            selected_count > 0 && missing.is_empty() && extra.is_empty() && changed.is_empty();
        let report = json!({
            "schema":"cryptex.cortex.selection-comparison.v1", "brain_id":self.brain_id,
            "ready_for_owner_review":ready, "selected_count":selected_count, "verified_count":receipt_count,
            "matched":matched, "missing":missing, "extra":extra, "changed":changed
        });
        let review_sha256 = if ready {
            let bytes =
                serde_json_canonicalizer::to_vec(&report).map_err(|_| fault("review_encoding"))?;
            Some(format!("{:x}", Sha256::digest(bytes)))
        } else {
            None
        };
        Ok(Comparison {
            report,
            review_sha256,
        })
    }
}

pub(super) struct Comparison {
    pub report: Value,
    pub review_sha256: Option<String>,
}
