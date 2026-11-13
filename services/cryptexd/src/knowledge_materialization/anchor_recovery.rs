//! One canonical scan and one journal commit for startup anchor recovery.
//! Persisted witnesses remain hints. Locks cover the complete current history
//! until every selected receipt has been rechecked and the journal reconciled.

use super::*;
use crate::bootstrap_journal::{
    BootstrapJournalStore, BootstrapOperation, BootstrapState, PendingBootstrapTransaction,
};
use std::collections::BTreeMap;

fn held(reason: &str) -> KnowledgeMaterializationError {
    KnowledgeMaterializationError::Verification(reason.into())
}

pub(super) fn reconcile_before_startup<S: SecureStore + ?Sized>(
    runtime_root: &Path,
    store: &S,
    env: SecureStoreEnvironment,
) -> KnowledgeMaterializationResult<()> {
    let journal =
        BootstrapJournalStore::new(runtime_root).map_err(|error| held(&error.to_string()))?;
    // Drop the journal lock before acquiring any canonical locks.
    let pending = journal.load().map_err(|error| held(&error.to_string()))?;
    let transactions = pending
        .transactions
        .iter()
        .filter(|transaction| {
            matches!(
                transaction.state,
                BootstrapState::AppendUnknown | BootstrapState::Anchored
            )
        })
        .collect::<Vec<_>>();
    if transactions.is_empty() {
        return Ok(());
    }
    let brain = crate::identity_runtime::load_brain_identity(runtime_root)
        .map_err(|error| held(&error.to_string()))?;
    let identity = load_local_device_identity(runtime_root, &brain.brain_id)
        .map_err(|error| held(&error.to_string()))?;
    let local_device =
        DeviceId::new(identity.device_id).map_err(|error| held(&error.to_string()))?;
    let ready = load_existing_event_signing_key(store, env, runtime_root, &local_device)
        .map_err(|error| held(&error.to_string()))?;
    let local_key_id = SigningKeyId::parse(&ready.public.signing_key_id)
        .map_err(|error| held(&error.to_string()))?;
    let devices = read_canonical_device_inventory(runtime_root, None)?;
    let policy = scan_verified_trust_policy(
        runtime_root,
        &brain.brain_id,
        &local_device,
        &local_key_id,
        &ready.signing_key.verifying_key().to_bytes(),
        &devices,
    )?;
    let crate::trust_reducer::TrustReduction::Ready(trust) = &policy.reduction else {
        return Err(held("startup recovery trust policy is held"));
    };
    if policy
        .devices
        .iter()
        .any(|device| !trust.trusted_devices.contains(device))
    {
        return Err(held("startup recovery found an untrusted canonical stream"));
    }

    // Event IDs are global. Security transaction IDs span security types
    // within one author stream; a bilateral counterpart has the same ID.
    let mut event_ids: BTreeMap<&str, Vec<&AuthenticatedSecurityRecord>> = BTreeMap::new();
    let mut local_transactions: BTreeMap<&str, Vec<&cryptex_events::SecurityEventV3>> =
        BTreeMap::new();
    for record in policy.history.records() {
        event_ids.entry(record.event_id()).or_default().push(record);
        if let AuthenticatedSecurityRecord::SecurityV3(event) = record
            && event.device_id() == &local_device
        {
            let id = match event.payload() {
                SecurityPayloadV3::PeerApproved { transaction_id, .. }
                | SecurityPayloadV3::DeviceRestricted { transaction_id, .. }
                | SecurityPayloadV3::DeviceSelfRetired { transaction_id, .. } => transaction_id,
            };
            local_transactions.entry(id).or_default().push(event);
        }
    }

    let mut anchors = Vec::with_capacity(transactions.len());
    for transaction in transactions {
        let snapshot = &transaction.snapshot;
        if snapshot.brain_id != brain.brain_id
            || snapshot.local_identity.device_id != local_device.as_str()
            || snapshot.local_identity.signing_key_id != local_key_id.as_str()
        {
            return Err(held(
                "startup recovery identity differs from the enrolled device",
            ));
        }
        let records = event_ids.get(transaction.stable_event_id.as_str());
        let matching = local_transactions.get(snapshot.transaction_id.as_str());
        let (Some(records), Some(matching)) = (records, matching) else {
            ensure_anchor_append_retry_is_safe(
                transaction.state,
                AnchorAppendMode::StartupReconciliation,
            )?;
            return Err(held(
                "journaled anchor absent from canonical history; startup will not append",
            ));
        };
        if records.len() != 1 || matching.len() != 1 {
            return Err(held(
                "canonical event or local security transaction ID duplicated; startup held",
            ));
        }
        let AuthenticatedSecurityRecord::SecurityV3(event) = records[0] else {
            return Err(held("stable event ID occupied by a non-security record"));
        };
        if !std::ptr::eq(event, matching[0])
            || event.device_id() != &local_device
            || event.brain_id() != brain.brain_id
            || event.signing_key_id() != &local_key_id
        {
            return Err(held(
                "canonical anchor identity differs from the approved operation",
            ));
        }
        match snapshot.operation {
            BootstrapOperation::PeerAdmission => verify_peer(transaction, event)?,
            BootstrapOperation::DeviceRestriction | BootstrapOperation::SelfRetirement => {
                verify_owner(transaction, event, &policy)?
            }
        }
        anchors.push((transaction, event));
    }
    // Keep canonical locks throughout this compare-and-batch commit. The
    // store reloads under its own lock and refuses concurrent journal changes.
    journal
        .reconcile_verified_anchor_events(&pending, &anchors)
        .map_err(|error| held(&error.to_string()))
}

fn verify_peer(
    transaction: &PendingBootstrapTransaction,
    event: &cryptex_events::SecurityEventV3,
) -> KnowledgeMaterializationResult<()> {
    let raw = transaction
        .transcript_messages
        .iter()
        .map(|s| s.as_bytes().to_vec())
        .collect::<Vec<_>>();
    if raw.len() != 4 || !transaction.local_approval_signature_released {
        return Err(held(
            "peer recovery lacks the released complete owner-approved transcript",
        ));
    }
    let mut checked = transaction.clone();
    checked
        .record_verified_complete_transcript(&raw)
        .map_err(|error| held(&error.to_string()))?;
    if &checked != transaction {
        return Err(held(
            "peer recovery transcript changes its frozen journal entry",
        ));
    }
    let refs = raw.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let transcript = cryptex_events::verify_bootstrap_transcript(&refs)
        .map_err(|error| held(&error.to_string()))?;
    let snapshot = &transaction.snapshot;
    let key = SigningKeyId::parse(&snapshot.local_identity.signing_key_id)
        .map_err(|error| held(&error.to_string()))?;
    let digest = cryptex_events::peer_approval_operation_digest(
        &snapshot.brain_id,
        &snapshot.local_identity.device_id,
        &key,
        &snapshot.transaction_id,
        transcript.transcript(),
    )
    .map_err(|error| held(&error.to_string()))?;
    if event.event_type() != SecurityEventTypeV3::PeerApproved
        || event.operation_digest() != digest
        || !matches!(event.payload(), SecurityPayloadV3::PeerApproved { transaction_id, transcript: recorded }
            if transaction_id == &snapshot.transaction_id && recorded == transcript.transcript())
    {
        return Err(held(
            "canonical peer anchor differs from exact owner-approved transcript",
        ));
    }
    Ok(())
}

fn verify_owner(
    transaction: &PendingBootstrapTransaction,
    event: &cryptex_events::SecurityEventV3,
    policy: &VerifiedTrustPolicySnapshot,
) -> KnowledgeMaterializationResult<()> {
    if transaction.append_attempts.len() != 1 {
        return Err(held(
            "owner-operation recovery append evidence is ambiguous",
        ));
    }
    let attempt = &transaction.append_attempts[0];
    if event.device_seq().to_u64().ok() != Some(attempt.reserved_sequence)
        || event.prev_content_hash().as_str() != attempt.predecessor_hash
        || event.content_hash().as_str()
            != attempt
                .candidate_content_hash
                .as_deref()
                .unwrap_or_default()
    {
        return Err(held(
            "canonical owner-operation event differs from pinned append attempt",
        ));
    }
    let snapshot = &transaction.snapshot;
    let payload_matches = match (snapshot.operation, event.payload()) {
        (
            BootstrapOperation::DeviceRestriction,
            SecurityPayloadV3::DeviceRestricted {
                transaction_id,
                target_device_id,
                target_signing_key_id,
                target_public_key,
                reason,
                cutoff_seq,
                cutoff_content_hash,
            },
        ) => {
            let target = snapshot.restriction_target.as_deref().unwrap_or_default();
            let peer = snapshot.peer_identity.as_ref();
            let binding = DeviceId::new(target.to_owned())
                .ok()
                .and_then(|device| policy.verified_trust_history.approved_peer_binding(&device));
            let (expected_reason, expected_public_key) =
                match (snapshot.restriction_reason, binding) {
                    (Some(crate::bootstrap_journal::RestrictionReason::Retired), Some(binding)) => {
                        (
                            cryptex_events::RestrictionReasonV3::Retired,
                            lower_hex(&binding.public_key),
                        )
                    }
                    (
                        Some(crate::bootstrap_journal::RestrictionReason::Compromised),
                        Some(binding),
                    ) => (
                        cryptex_events::RestrictionReasonV3::Compromised,
                        lower_hex(&binding.public_key),
                    ),
                    _ => {
                        return Err(KnowledgeMaterializationError::Verification(
                            "owner-operation recovery lacks its approved target binding".into(),
                        ));
                    }
                };
            let expected_cutoff = snapshot
                .cutoff_seq
                .as_deref()
                .and_then(|value| value.parse::<u64>().ok());
            transaction_id == &snapshot.transaction_id
                && peer.is_some_and(|peer| {
                    peer.device_id == target
                        && peer.signing_key_id == target_signing_key_id.as_str()
                })
                && target_device_id.as_str() == target
                && target_public_key == &expected_public_key
                && *reason == expected_reason
                && Some(*cutoff_seq) == expected_cutoff
                && snapshot.cutoff_content_hash.as_deref() == Some(cutoff_content_hash.as_str())
        }
        (
            BootstrapOperation::SelfRetirement,
            SecurityPayloadV3::DeviceSelfRetired {
                transaction_id,
                last_authorized_seq,
                last_authorized_content_hash,
            },
        ) => {
            transaction_id == &snapshot.transaction_id
                && snapshot
                    .last_authorized_seq
                    .as_deref()
                    .and_then(|value| value.parse::<u64>().ok())
                    == Some(*last_authorized_seq)
                && snapshot.last_authorized_content_hash.as_deref()
                    == Some(last_authorized_content_hash.as_str())
        }
        _ => false,
    };
    if !payload_matches {
        return Err(KnowledgeMaterializationError::Verification(
            "canonical owner-operation payload differs from the frozen owner snapshot".into(),
        ));
    }

    Ok(())
}
