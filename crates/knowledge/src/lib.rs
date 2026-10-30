//! Deterministic M4 knowledge reduction (ADR-0017).
//!
//! This crate is pure: it reads already verified v2 records and returns a
//! rebuildable semantic result. It does not append canonical events, access
//! SQLite, infer facts from free text, or treat signatures as proof of truth.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use cryptex_events::{
    Classification, Confidence, ConflictOutcome, FactValue, KnowledgeAssertionPayload,
    SemanticAssertion, VerifiedV2History, VerifiedV2KnowledgeAssertion, VersionedRecord,
};
use serde_json::json;
use unicode_normalization::UnicodeNormalization;

pub mod sqlite;
mod unicode16_casefold;

pub const CARDINALITY_POLICY_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReducerError {
    InvalidEvent(String),
    ConflictingEventId(String),
    CanonicalizationFailed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    pub event_id: String,
    pub device_id: String,
    pub device_seq: u64,
    pub signing_key_id: String,
    pub confidence: Option<Confidence>,
    pub authorization_receipt_id: String,
    pub classification: Classification,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entity {
    pub entity_id: String,
    pub representative_id: String,
    pub kind: cryptex_events::EntityKind,
    pub label: String,
    pub provenance: Vec<Provenance>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alias {
    pub semantic_id: String,
    pub entity_id: String,
    pub spelling: String,
    pub normalized_key: String,
    pub provenance: Vec<Provenance>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fact {
    pub semantic_id: String,
    pub subject_entity_id: String,
    pub predicate: String,
    pub value: FactValue,
    pub provenance: Vec<Provenance>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relation {
    pub semantic_id: String,
    pub subject_entity_id: String,
    pub predicate: String,
    pub object_entity_id: String,
    pub provenance: Vec<Provenance>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub kind: &'static str,
    pub key: String,
    pub event_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingDependency {
    pub event_id: String,
    pub target_id: String,
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Withdrawal {
    pub target_event_id: String,
    pub provenance: Vec<Provenance>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrectionRecord {
    pub correction_event_id: String,
    pub target_event_id: String,
    pub replacement: Option<SemanticAssertion>,
    pub resolution_status: &'static str,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityMergeRecord {
    pub event_id: String,
    pub entity_ids: Vec<String>,
    pub representative_id: Option<String>,
    pub status: EntityMergeStatus,
    pub provenance: Vec<Provenance>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityMergeStatus {
    Active,
    Revoked,
    Pending,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KnowledgeState {
    pub entities: Vec<Entity>,
    pub aliases: Vec<Alias>,
    pub facts: Vec<Fact>,
    pub relations: Vec<Relation>,
    pub conflicts: Vec<Conflict>,
    pub pending: Vec<PendingDependency>,
    pub withdrawals: Vec<Withdrawal>,
    pub corrections: Vec<CorrectionRecord>,
    pub entity_merges: Vec<EntityMergeRecord>,
    /// Events whose semantics were applied or safely retained as a no-op.
    pub materialized_event_ids: Vec<String>,
}

/// Reduce one complete, chain-verified event set. The supplied history must
/// have been verified with a resolver that returns only keys trusted as
/// Cryptex daemon attestations for enrolled devices. The signed receipt UUID
/// is retained as provenance; it is not independently verified human consent.
/// Input order has no semantic effect.
pub fn reduce_verified_events(
    expected_brain_id: &str,
    input: &VerifiedV2History,
) -> Result<KnowledgeState, ReducerError> {
    reduce_verified_events_cancellable(expected_brain_id, input, || false)
}

/// Reduce only content records admitted by effective cutoffs from the local
/// trust reducer. V3 policy records never enter the knowledge reducer.
///
/// This function assumes the supplied security history is complete and
/// authenticated and that `effective_cutoffs` was derived from locally
/// anchored trust policy. It does not establish either property itself.
pub fn reduce_verified_security_history(
    expected_brain_id: &str,
    input: &cryptex_events::VerifiedSecurityHistory,
    effective_cutoffs: &std::collections::BTreeMap<String, u64>,
) -> Result<KnowledgeState, ReducerError> {
    reduce_verified_security_history_cancellable(
        expected_brain_id,
        input,
        effective_cutoffs,
        || false,
    )
}

/// Cancellable cutoff-aware reduction. Cancellation never returns a partial
/// knowledge state.
pub fn reduce_verified_security_history_cancellable<C>(
    expected_brain_id: &str,
    input: &cryptex_events::VerifiedSecurityHistory,
    effective_cutoffs: &std::collections::BTreeMap<String, u64>,
    mut cancelled: C,
) -> Result<KnowledgeState, ReducerError>
where
    C: FnMut() -> bool,
{
    let content = input
        .content_history_through_cutoffs_cancellable(effective_cutoffs, &mut cancelled)
        .map_err(|error| match error {
            cryptex_events::CancellableSecurityHistoryError::Cancelled => ReducerError::Cancelled,
            cryptex_events::CancellableSecurityHistoryError::InvalidRecord(_) => {
                ReducerError::InvalidEvent(expected_brain_id.to_owned())
            }
        })?;
    reduce_verified_events_cancellable(expected_brain_id, &content, cancelled)
}

/// Reduce a complete verified history with cooperative cancellation points.
/// A cancelled reduction returns no partial state.
pub fn reduce_verified_events_cancellable<C>(
    expected_brain_id: &str,
    input: &VerifiedV2History,
    mut cancelled: C,
) -> Result<KnowledgeState, ReducerError>
where
    C: FnMut() -> bool,
{
    let check_cancelled = |cancelled: &mut C| {
        if cancelled() {
            Err(ReducerError::Cancelled)
        } else {
            Ok(())
        }
    };
    check_cancelled(&mut cancelled)?;
    cryptex_core::validate::parse_uuid_str(expected_brain_id)
        .map_err(|_| ReducerError::InvalidEvent(expected_brain_id.to_owned()))?;
    let mut known_event_ids = BTreeSet::new();
    for record in input.records() {
        check_cancelled(&mut cancelled)?;
        if let Some(event_id) = verified_record_event_id(record) {
            known_event_ids.insert(canonical_uuid(event_id));
        }
    }
    let mut by_id = BTreeMap::<String, VerifiedV2KnowledgeAssertion>::new();
    let mut known_assertion_event_ids = BTreeSet::new();
    for record in input.assertions() {
        check_cancelled(&mut cancelled)?;
        let event = record.event();
        event
            .validate()
            .map_err(|_| ReducerError::InvalidEvent(event.event_id.clone()))?;
        if canonical_uuid(&event.brain_id) != canonical_uuid(expected_brain_id) {
            return Err(ReducerError::InvalidEvent(event.event_id.clone()));
        }
        let canonical_id = canonical_uuid(&event.event_id);
        if matches!(
            event.payload,
            KnowledgeAssertionPayload::FactAssert { .. }
                | KnowledgeAssertionPayload::RelationAssert { .. }
        ) {
            known_assertion_event_ids.insert(canonical_id.clone());
        }
        match by_id.get(&canonical_id) {
            Some(prior) if prior.raw_record() != record.raw_record() => {
                return Err(ReducerError::ConflictingEventId(canonical_id));
            }
            Some(_) => {}
            None => {
                by_id.insert(canonical_id, record.clone());
            }
        }
    }

    let mut records = Vec::with_capacity(by_id.len());
    for record in by_id.into_values() {
        check_cancelled(&mut cancelled)?;
        records.push(record);
    }
    sort_by_cancellable(
        &mut records,
        |left, right| stable_event_cmp(left.event(), right.event()),
        &mut cancelled,
    )?;
    let mut state = KnowledgeState::default();
    let mut invalid_event_ids = BTreeSet::<String>::new();
    let mut entities = BTreeMap::<String, Entity>::new();
    let mut conflicted_entities = BTreeSet::<String>::new();
    let mut aliases = Vec::<(String, String, Provenance)>::new();
    let mut merges = Vec::<(String, Vec<String>, Provenance)>::new();
    let mut revoked_merges = Vec::<(String, String, Provenance)>::new();
    let mut assertions = BTreeMap::<String, (SemanticAssertion, Provenance)>::new();
    let mut corrections =
        BTreeMap::<String, Vec<(String, Option<SemanticAssertion>, Provenance)>>::new();
    let mut resolutions = Vec::new();
    let mut handled_resolution_ids = BTreeSet::new();

    for record in &records {
        check_cancelled(&mut cancelled)?;
        let event = record.event();
        let payload = &event.payload;
        let provenance = provenance(event);
        match payload {
            KnowledgeAssertionPayload::EntityDeclare {
                entity_id,
                entity_kind,
                label,
                ..
            } => {
                let entity_id = canonical_uuid(entity_id);
                if conflicted_entities.contains(&entity_id) {
                    state.conflicts.push(Conflict {
                        kind: "entity_declaration",
                        key: entity_id,
                        event_ids: vec![event.event_id.clone()],
                    });
                    continue;
                }
                match entities.get_mut(&entity_id) {
                    Some(existing) if existing.kind == *entity_kind && existing.label == *label => {
                        existing.provenance.push(provenance);
                    }
                    Some(existing) => {
                        let prior = existing.provenance[0].event_id.clone();
                        entities.remove(&entity_id);
                        conflicted_entities.insert(entity_id.clone());
                        state.conflicts.push(Conflict {
                            kind: "entity_declaration",
                            key: entity_id.clone(),
                            event_ids: vec![prior, event.event_id.clone()],
                        });
                    }
                    None => {
                        entities.insert(
                            entity_id.clone(),
                            Entity {
                                entity_id: entity_id.clone(),
                                representative_id: entity_id,
                                kind: *entity_kind,
                                label: label.clone(),
                                provenance: vec![provenance],
                            },
                        );
                    }
                }
            }
            KnowledgeAssertionPayload::EntityAlias {
                entity_id, alias, ..
            } => {
                aliases.push((canonical_uuid(entity_id), alias.clone(), provenance));
            }
            KnowledgeAssertionPayload::EntityMerge { entity_ids, .. } => {
                merges.push((
                    canonical_uuid(&event.event_id),
                    entity_ids.iter().map(|id| canonical_uuid(id)).collect(),
                    provenance,
                ));
            }
            KnowledgeAssertionPayload::EntityMergeRevoke {
                target_merge_event_id,
                ..
            } => {
                revoked_merges.push((
                    canonical_uuid(&event.event_id),
                    canonical_uuid(target_merge_event_id),
                    provenance,
                ));
            }
            KnowledgeAssertionPayload::FactAssert {
                subject_entity_id,
                predicate,
                value,
                confidence,
                ..
            } => {
                assertions.insert(
                    canonical_uuid(&event.event_id),
                    (
                        SemanticAssertion::Fact {
                            subject_entity_id: canonical_uuid(subject_entity_id),
                            predicate: predicate.clone(),
                            value: value.clone(),
                            confidence: *confidence,
                        },
                        provenance,
                    ),
                );
            }
            KnowledgeAssertionPayload::RelationAssert {
                subject_entity_id,
                predicate,
                object_entity_id,
                confidence,
                ..
            } => {
                assertions.insert(
                    canonical_uuid(&event.event_id),
                    (
                        SemanticAssertion::Relation {
                            subject_entity_id: canonical_uuid(subject_entity_id),
                            predicate: predicate.clone(),
                            object_entity_id: canonical_uuid(object_entity_id),
                            confidence: *confidence,
                        },
                        provenance,
                    ),
                );
            }
            KnowledgeAssertionPayload::AssertionCorrect {
                target_event_id,
                replacement,
                ..
            } => {
                let mut provenance = provenance;
                if let Some(replacement) = &replacement.0 {
                    provenance.confidence = Some(match replacement {
                        SemanticAssertion::Fact { confidence, .. }
                        | SemanticAssertion::Relation { confidence, .. } => *confidence,
                    });
                }
                corrections
                    .entry(canonical_uuid(target_event_id))
                    .or_default()
                    .push((
                        canonical_uuid(&event.event_id),
                        replacement.0.clone(),
                        provenance,
                    ));
            }
            KnowledgeAssertionPayload::AssertionResolveConflict {
                target_event_id,
                decision_event_ids,
                outcome,
                ..
            } => {
                resolutions.push((
                    canonical_uuid(&event.event_id),
                    canonical_uuid(target_event_id),
                    decision_event_ids
                        .iter()
                        .map(|id| canonical_uuid(id))
                        .collect::<Vec<_>>(),
                    outcome.clone(),
                    provenance,
                ));
            }
        }
    }

    // Revoke is effective only for a known merge edge. Unknown references stay pending.
    let mut revoked_targets = BTreeSet::new();
    let mut revoked_provenance_by_target = BTreeMap::<String, Vec<Provenance>>::new();
    for (_, target, _) in &revoked_merges {
        check_cancelled(&mut cancelled)?;
        revoked_targets.insert(target.clone());
    }
    for (_, target, source) in &revoked_merges {
        check_cancelled(&mut cancelled)?;
        revoked_provenance_by_target
            .entry(target.clone())
            .or_default()
            .push(source.clone());
    }
    let mut merge_ids = BTreeSet::new();
    for (merge_id, _, _) in &merges {
        check_cancelled(&mut cancelled)?;
        merge_ids.insert(merge_id.clone());
    }
    let mut union_find = EntityUnionFind::new(entities.keys(), &mut cancelled)?;
    let mut active_merges = BTreeSet::new();
    for (merge_id, ids, _provenance) in &merges {
        check_cancelled(&mut cancelled)?;
        if revoked_targets.contains(merge_id) {
            continue;
        }
        let mut missing_entity = None;
        for id in ids {
            check_cancelled(&mut cancelled)?;
            if !entities.contains_key(id) {
                missing_entity = Some(id.clone());
                break;
            }
        }
        if let Some(missing_entity) = missing_entity {
            state.pending.push(PendingDependency {
                event_id: merge_id.clone(),
                target_id: missing_entity,
                reason: "missing_entity",
            });
            continue;
        }
        union_find.union_all(ids, &mut cancelled)?;
        active_merges.insert(merge_id.clone());
    }
    for (entity_id, entity) in &mut entities {
        check_cancelled(&mut cancelled)?;
        entity.representative_id = union_find.representative(entity_id, &mut cancelled)?;
    }
    for (revoke_event_id, target_merge_id, _) in &revoked_merges {
        check_cancelled(&mut cancelled)?;
        if !merge_ids.contains(target_merge_id) {
            if known_event_ids.contains(target_merge_id) {
                invalid_event_ids.insert(revoke_event_id.clone());
                state.conflicts.push(Conflict {
                    kind: "invalid_merge_revoke_reference",
                    key: target_merge_id.clone(),
                    event_ids: vec![revoke_event_id.clone(), target_merge_id.clone()],
                });
                continue;
            }
            state.pending.push(PendingDependency {
                event_id: revoke_event_id.clone(),
                target_id: target_merge_id.clone(),
                reason: "missing_merge",
            });
        }
    }
    let mut pending_event_ids = BTreeSet::new();
    for pending in &state.pending {
        check_cancelled(&mut cancelled)?;
        pending_event_ids.insert(pending.event_id.clone());
    }
    for (merge_id, ids, merge_provenance) in &merges {
        check_cancelled(&mut cancelled)?;
        let status = if pending_event_ids.contains(merge_id) {
            EntityMergeStatus::Pending
        } else if active_merges.contains(merge_id) {
            EntityMergeStatus::Active
        } else {
            EntityMergeStatus::Revoked
        };
        let representative_id = (status == EntityMergeStatus::Active)
            .then(|| {
                ids.iter()
                    .filter_map(|id| {
                        entities
                            .get(id)
                            .map(|entity| entity.representative_id.clone())
                    })
                    .min()
            })
            .flatten();
        let mut provenance = vec![merge_provenance.clone()];
        if let Some(revocations) = revoked_provenance_by_target.get(merge_id) {
            for source in revocations {
                check_cancelled(&mut cancelled)?;
                provenance.push(source.clone());
            }
        }
        sort_by_cancellable(
            &mut provenance,
            |a, b| a.event_id.cmp(&b.event_id),
            &mut cancelled,
        )?;
        state.entity_merges.push(EntityMergeRecord {
            event_id: merge_id.clone(),
            entity_ids: ids.clone(),
            representative_id,
            status,
            provenance,
        });
    }
    sort_by_cancellable(
        &mut state.entity_merges,
        |a, b| a.event_id.cmp(&b.event_id),
        &mut cancelled,
    )?;

    sort_by_cancellable(
        &mut aliases,
        |a, b| {
            normalize_alias(&a.1)
                .cmp(&normalize_alias(&b.1))
                .then(a.1.cmp(&b.1))
                .then(a.0.cmp(&b.0))
        },
        &mut cancelled,
    )?;
    let mut alias_classes: Vec<Vec<(String, String, Provenance)>> = Vec::new();
    let mut current_alias_key: Option<String> = None;
    for alias in aliases {
        check_cancelled(&mut cancelled)?;
        let key = normalize_alias(&alias.1);
        if current_alias_key.as_deref() == Some(key.as_str()) {
            alias_classes
                .last_mut()
                .expect("current alias key has a group")
                .push(alias);
        } else {
            current_alias_key = Some(key);
            alias_classes.push(vec![alias]);
        }
    }
    for group in alias_classes {
        let mut normalized_key: Option<String> = None;
        let mut targets = BTreeSet::new();
        let mut provenance = Vec::new();
        let mut spelling = String::new();
        let mut has_unresolved_target = false;
        for (entity_id, alias_spelling, source) in group {
            check_cancelled(&mut cancelled)?;
            let folded = normalize_alias(&alias_spelling);
            if normalized_key.as_ref().is_none_or(|prior| folded < *prior) {
                normalized_key = Some(folded);
            }
            if let Some(entity) = entities.get(&entity_id) {
                targets.insert(entity.representative_id.clone());
                if spelling.is_empty() || alias_spelling < spelling {
                    spelling = alias_spelling;
                }
                provenance.push(source);
            } else {
                has_unresolved_target = true;
                state.pending.push(PendingDependency {
                    event_id: source.event_id.clone(),
                    target_id: entity_id,
                    reason: "missing_entity",
                });
            }
        }
        let normalized_key = normalized_key.unwrap_or_default();
        if targets.len() > 1 {
            let mut event_ids = Vec::with_capacity(provenance.len());
            for source in &provenance {
                check_cancelled(&mut cancelled)?;
                event_ids.push(source.event_id.clone());
            }
            state.conflicts.push(Conflict {
                kind: "ambiguous_alias",
                key: normalized_key.clone(),
                event_ids,
            });
        }
        if targets.len() == 1 && !has_unresolved_target {
            let entity_id = targets.iter().next().cloned().unwrap_or_default();
            sort_by_cancellable(
                &mut provenance,
                |a, b| a.event_id.cmp(&b.event_id),
                &mut cancelled,
            )?;
            state.aliases.push(Alias {
                semantic_id: derived_id(
                    "alias",
                    &json!([
                        canonical_uuid(expected_brain_id),
                        &entity_id,
                        &normalized_key
                    ]),
                )?,
                entity_id,
                spelling,
                normalized_key,
                provenance,
            });
        }
    }

    let mut resolutions_by_target = BTreeMap::<String, Vec<_>>::new();
    for resolution in resolutions {
        check_cancelled(&mut cancelled)?;
        resolutions_by_target
            .entry(resolution.1.clone())
            .or_default()
            .push(resolution);
    }

    let mut active = BTreeMap::<String, (SemanticAssertion, Vec<Provenance>)>::new();
    for (target, mut decisions) in corrections {
        check_cancelled(&mut cancelled)?;
        sort_by_cancellable(&mut decisions, |a, b| a.0.cmp(&b.0), &mut cancelled)?;
        let Some((base, base_provenance)) = assertions.remove(&target) else {
            let mut correction_ids = Vec::with_capacity(decisions.len());
            for decision in &decisions {
                check_cancelled(&mut cancelled)?;
                correction_ids.push(decision.0.clone());
            }
            if known_event_ids.contains(&target) {
                for correction_id in &correction_ids {
                    check_cancelled(&mut cancelled)?;
                    invalid_event_ids.insert(correction_id.clone());
                }
                state.conflicts.push(Conflict {
                    kind: "invalid_correction_target",
                    key: target,
                    event_ids: correction_ids,
                });
            } else {
                for correction_id in correction_ids {
                    state.pending.push(PendingDependency {
                        event_id: correction_id,
                        target_id: target.clone(),
                        reason: "missing_assertion",
                    });
                }
            }
            continue;
        };
        let mut decision_ids = BTreeSet::new();
        let mut correction_history = Vec::with_capacity(decisions.len());
        for decision in &decisions {
            check_cancelled(&mut cancelled)?;
            decision_ids.insert(decision.0.clone());
            correction_history.push(decision.2.clone());
        }
        let target_resolutions = resolutions_by_target
            .get(&target)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let mut resolution_history = Vec::with_capacity(target_resolutions.len());
        for resolution in target_resolutions {
            check_cancelled(&mut cancelled)?;
            handled_resolution_ids.insert(resolution.0.clone());
            resolution_history.push(resolution.4.clone());
        }
        let selected: Vec<(String, Option<SemanticAssertion>, Provenance)> = if !target_resolutions
            .is_empty()
        {
            let mut resolution_ids = BTreeSet::new();
            let mut resolution_by_id = BTreeMap::new();
            for resolution in target_resolutions {
                check_cancelled(&mut cancelled)?;
                resolution_ids.insert(resolution.0.clone());
                resolution_by_id.insert(resolution.0.as_str(), resolution);
            }
            let mut resolution_edges = BTreeMap::new();
            for resolution in target_resolutions {
                check_cancelled(&mut cancelled)?;
                let mut dependencies = BTreeSet::new();
                for id in &resolution.2 {
                    check_cancelled(&mut cancelled)?;
                    if resolution_ids.contains(id) {
                        dependencies.insert(id.clone());
                    }
                }
                resolution_edges.insert(resolution.0.clone(), dependencies);
            }
            if graph_has_cycle(&resolution_edges, &mut cancelled)? {
                let mut event_ids: Vec<_> = decision_ids.iter().cloned().collect();
                event_ids.extend(resolution_ids.iter().cloned());
                sort_by_cancellable(&mut event_ids, Ord::cmp, &mut cancelled)?;
                let mut unique_event_ids = Vec::with_capacity(event_ids.len());
                for event_id in event_ids {
                    check_cancelled(&mut cancelled)?;
                    if unique_event_ids.last() != Some(&event_id) {
                        unique_event_ids.push(event_id);
                    }
                }
                state.conflicts.push(Conflict {
                    kind: "decision_cycle",
                    key: target.clone(),
                    event_ids: unique_event_ids,
                });
                for resolution_id in resolution_ids {
                    state.pending.push(PendingDependency {
                        event_id: resolution_id,
                        target_id: target.clone(),
                        reason: "decision_cycle",
                    });
                }
                continue;
            }
            let mut depended_on = BTreeSet::new();
            for dependencies in resolution_edges.values() {
                check_cancelled(&mut cancelled)?;
                for dependency in dependencies {
                    check_cancelled(&mut cancelled)?;
                    depended_on.insert(dependency.clone());
                }
            }
            let mut terminal_resolutions = BTreeSet::new();
            for resolution_id in &resolution_ids {
                check_cancelled(&mut cancelled)?;
                if !depended_on.contains(resolution_id) {
                    terminal_resolutions.insert(resolution_id.clone());
                }
            }
            let mut resolved =
                BTreeMap::<String, (String, Option<SemanticAssertion>, Provenance)>::new();
            for decision in &decisions {
                resolved.insert(decision.0.clone(), decision.clone());
            }
            let mut invalid_resolution = false;
            let mut choices =
                BTreeMap::<String, (String, Option<SemanticAssertion>, Provenance)>::new();
            for resolution_id in topological_nodes_cancellable(&resolution_edges, &mut cancelled)? {
                let Some(resolution) = resolution_by_id.get(resolution_id.as_str()) else {
                    continue;
                };
                let (_, _, supplied_ids, outcome, provenance) = *resolution;
                let mut supplied = BTreeSet::new();
                for supplied_id in supplied_ids {
                    check_cancelled(&mut cancelled)?;
                    supplied.insert(supplied_id.clone());
                }
                let mut missing_reference = false;
                let mut invalid_reference = false;
                for supplied_id in &supplied {
                    check_cancelled(&mut cancelled)?;
                    if decision_ids.contains(supplied_id) || resolution_ids.contains(supplied_id) {
                        continue;
                    }
                    if known_event_ids.contains(supplied_id) {
                        invalid_reference = true;
                    } else {
                        missing_reference = true;
                    }
                }
                if invalid_reference {
                    invalid_event_ids.insert(resolution_id.clone());
                    state.conflicts.push(Conflict {
                        kind: "invalid_resolution_reference",
                        key: target.clone(),
                        event_ids: vec![resolution_id.clone()],
                    });
                    invalid_resolution = true;
                    continue;
                }
                if missing_reference {
                    state.pending.push(PendingDependency {
                        event_id: resolution_id.clone(),
                        target_id: target.clone(),
                        reason: "missing_decision",
                    });
                    invalid_resolution = true;
                    continue;
                }
                let expected_terminal_count = decision_ids.len() + resolution_ids.len() - 1;
                let mut complete_terminal_set = supplied.len() == expected_terminal_count;
                if complete_terminal_set {
                    for decision_id in &decision_ids {
                        check_cancelled(&mut cancelled)?;
                        if !supplied.contains(decision_id) {
                            complete_terminal_set = false;
                            break;
                        }
                    }
                }
                if complete_terminal_set {
                    for other_resolution in &resolution_ids {
                        check_cancelled(&mut cancelled)?;
                        if other_resolution != &resolution_id
                            && !supplied.contains(other_resolution)
                        {
                            complete_terminal_set = false;
                            break;
                        }
                    }
                }
                if terminal_resolutions.contains(&resolution_id) && !complete_terminal_set {
                    state.pending.push(PendingDependency {
                        event_id: resolution_id.clone(),
                        target_id: target.clone(),
                        reason: "incomplete_decision_set",
                    });
                    invalid_resolution = true;
                    continue;
                }
                let chosen = match outcome {
                    ConflictOutcome::Select { decision_event_id } => {
                        let decision_id = canonical_uuid(decision_event_id);
                        supplied
                            .contains(&decision_id)
                            .then(|| resolved.get(&decision_id).cloned())
                            .flatten()
                    }
                    ConflictOutcome::Replace { value } => {
                        let mut source = provenance.clone();
                        source.confidence = Some(match value {
                            SemanticAssertion::Fact { confidence, .. }
                            | SemanticAssertion::Relation { confidence, .. } => *confidence,
                        });
                        Some((resolution_id.clone(), Some(value.clone()), source))
                    }
                    ConflictOutcome::Withdraw => {
                        Some((resolution_id.clone(), None, provenance.clone()))
                    }
                };
                let Some((_, value, _)) = chosen else {
                    state.pending.push(PendingDependency {
                        event_id: resolution_id.clone(),
                        target_id: target.clone(),
                        reason: "missing_decision",
                    });
                    invalid_resolution = true;
                    continue;
                };
                let mut source = provenance.clone();
                if source.confidence.is_none() {
                    source.confidence = value.as_ref().map(|assertion| match assertion {
                        SemanticAssertion::Fact { confidence, .. }
                        | SemanticAssertion::Relation { confidence, .. } => *confidence,
                    });
                }
                let resolved_choice = (resolution_id.clone(), value, source);
                resolved.insert(resolution_id.clone(), resolved_choice.clone());
                if terminal_resolutions.contains(&resolution_id) {
                    choices.insert(
                        semantic_fingerprint(resolved_choice.1.as_ref())?,
                        resolved_choice,
                    );
                }
            }
            if choices.len() > 1 {
                let mut event_ids = Vec::new();
                for decision in &decisions {
                    check_cancelled(&mut cancelled)?;
                    event_ids.push(decision.0.clone());
                }
                for resolution in target_resolutions {
                    check_cancelled(&mut cancelled)?;
                    event_ids.push(resolution.0.clone());
                }
                sort_by_cancellable(&mut event_ids, Ord::cmp, &mut cancelled)?;
                state.conflicts.push(Conflict {
                    kind: "resolution",
                    key: target.clone(),
                    event_ids,
                });
                continue;
            }
            if invalid_resolution || choices.is_empty() {
                continue;
            }
            let mut selected = Vec::with_capacity(choices.len());
            for choice in choices.into_values() {
                check_cancelled(&mut cancelled)?;
                selected.push(choice);
            }
            selected
        } else {
            let mut distinct = BTreeSet::new();
            for (_, replacement, _) in &decisions {
                check_cancelled(&mut cancelled)?;
                distinct.insert(semantic_fingerprint(replacement.as_ref())?);
            }
            if distinct.len() > 1 {
                let mut event_ids = Vec::with_capacity(decisions.len());
                for decision in &decisions {
                    check_cancelled(&mut cancelled)?;
                    event_ids.push(decision.0.clone());
                }
                state.conflicts.push(Conflict {
                    kind: "correction",
                    key: target.clone(),
                    event_ids,
                });
                continue;
            }
            decisions
        };
        if let Some((correction_id, replacement, _provenance)) = selected.first().cloned() {
            for (event_id, replacement, provenance) in &selected {
                state.corrections.push(CorrectionRecord {
                    correction_event_id: event_id.clone(),
                    target_event_id: target.clone(),
                    replacement: replacement.clone(),
                    resolution_status: if replacement.is_some() {
                        "applied"
                    } else {
                        "withdrawn"
                    },
                    provenance: provenance.clone(),
                });
            }
            let mut sources = Vec::new();
            for (_, _, source) in &selected {
                check_cancelled(&mut cancelled)?;
                sources.push(source.clone());
            }
            for source in correction_history {
                check_cancelled(&mut cancelled)?;
                sources.push(source);
            }
            for source in resolution_history {
                check_cancelled(&mut cancelled)?;
                sources.push(source);
            }
            sources.push(base_provenance);
            sort_by_cancellable(
                &mut sources,
                |a, b| a.event_id.cmp(&b.event_id),
                &mut cancelled,
            )?;
            let mut unique_sources = Vec::with_capacity(sources.len());
            for source in sources {
                check_cancelled(&mut cancelled)?;
                if unique_sources
                    .last()
                    .is_none_or(|prior: &Provenance| prior.event_id != source.event_id)
                {
                    unique_sources.push(source);
                }
            }
            if let Some(replacement) = replacement {
                active.insert(correction_id, (replacement, unique_sources));
            } else {
                state.withdrawals.push(Withdrawal {
                    target_event_id: target,
                    provenance: unique_sources,
                });
            }
        }
        let _ = base;
    }
    // A resolution with no correction group must never silently count as
    // materialized. Keep its dependency pending until the complete event set
    // contains a resolvable target and competing decisions.
    for target_resolutions in resolutions_by_target.values() {
        check_cancelled(&mut cancelled)?;
        for (resolution_id, target, decision_ids, _outcome, _source) in target_resolutions {
            check_cancelled(&mut cancelled)?;
            if handled_resolution_ids.contains(resolution_id) {
                continue;
            }
            let target_is_assertion = known_assertion_event_ids.contains(target);
            let target_exists = known_event_ids.contains(target);
            if !target_is_assertion && target_exists {
                invalid_event_ids.insert(resolution_id.clone());
                state.conflicts.push(Conflict {
                    kind: "invalid_resolution_target",
                    key: target.clone(),
                    event_ids: vec![resolution_id.clone()],
                });
                continue;
            }
            state.pending.push(PendingDependency {
                event_id: resolution_id.clone(),
                target_id: target.clone(),
                reason: if !target_exists {
                    "missing_assertion"
                } else if decision_ids.is_empty() {
                    "missing_decision"
                } else {
                    "missing_correction_set"
                },
            });
        }
    }
    for (id, (assertion, source)) in assertions {
        check_cancelled(&mut cancelled)?;
        active.insert(id, (assertion, vec![source]));
    }

    let mut fact_candidates = Vec::<(String, String, FactValue, Vec<Provenance>)>::new();
    let mut relation_candidates = Vec::<(String, String, String, Vec<Provenance>)>::new();
    for (event_id, (assertion, provenance)) in active {
        check_cancelled(&mut cancelled)?;
        match assertion {
            SemanticAssertion::Fact {
                subject_entity_id,
                predicate,
                value,
                confidence: _,
            } => {
                let Some(subject) = entities.get(&subject_entity_id) else {
                    state.pending.push(PendingDependency {
                        event_id,
                        target_id: subject_entity_id,
                        reason: "missing_entity",
                    });
                    continue;
                };
                if predicate == "person.birth_date"
                    && subject.kind != cryptex_events::EntityKind::Person
                {
                    invalid_event_ids
                        .extend(provenance.iter().map(|source| source.event_id.clone()));
                    state.conflicts.push(Conflict {
                        kind: "wrong_subject_kind",
                        key: subject.entity_id.clone(),
                        event_ids: provenance
                            .iter()
                            .map(|source| source.event_id.clone())
                            .collect(),
                    });
                    continue;
                }
                let value = match value {
                    FactValue::EntityRef(referenced) => {
                        let Some(entity) = entities.get(&canonical_uuid(&referenced)) else {
                            state.pending.push(PendingDependency {
                                event_id,
                                target_id: canonical_uuid(&referenced),
                                reason: "missing_entity",
                            });
                            continue;
                        };
                        FactValue::EntityRef(entity.representative_id.clone())
                    }
                    other => other,
                };
                let subject_id = subject.representative_id.clone();
                fact_candidates.push((subject_id, predicate, value, provenance));
            }
            SemanticAssertion::Relation {
                subject_entity_id,
                predicate,
                object_entity_id,
                confidence: _,
            } => {
                let (Some(subject), Some(object)) = (
                    entities.get(&subject_entity_id),
                    entities.get(&object_entity_id),
                ) else {
                    let missing = if !entities.contains_key(&subject_entity_id) {
                        subject_entity_id
                    } else {
                        object_entity_id
                    };
                    state.pending.push(PendingDependency {
                        event_id,
                        target_id: missing,
                        reason: "missing_entity",
                    });
                    continue;
                };
                relation_candidates.push((
                    subject.representative_id.clone(),
                    predicate,
                    object.representative_id.clone(),
                    provenance,
                ));
            }
        }
    }

    // Aggregate exact semantic duplicates; birth_date is the only single-valued
    // predicate in policy v1, and disagreement remains an explicit conflict.
    let mut by_single =
        BTreeMap::<String, BTreeMap<String, Vec<(FactValue, Vec<Provenance>)>>>::new();
    let mut by_multi = BTreeMap::<String, Vec<(String, String, FactValue, Vec<Provenance>)>>::new();
    for (subject, predicate, value, provenance) in fact_candidates {
        check_cancelled(&mut cancelled)?;
        if predicate == "person.birth_date" {
            let value_key = semantic_key("", "", &value)?;
            by_single
                .entry(format!("{subject}|{predicate}"))
                .or_default()
                .entry(value_key)
                .or_default()
                .push((value, provenance));
        } else {
            let key = semantic_key(&subject, &predicate, &value)?;
            by_multi
                .entry(key)
                .or_default()
                .push((subject, predicate, value, provenance));
        }
    }
    for (key, values) in by_single {
        check_cancelled(&mut cancelled)?;
        if values.len() > 1 {
            let mut event_ids = Vec::new();
            for entries in values.values() {
                check_cancelled(&mut cancelled)?;
                for (_, sources) in entries {
                    for source in sources {
                        check_cancelled(&mut cancelled)?;
                        event_ids.push(source.event_id.clone());
                    }
                }
            }
            state.conflicts.push(Conflict {
                kind: "single_valued_fact",
                key,
                event_ids,
            });
            continue;
        }
        if let Some((_, entries)) = values.into_iter().next() {
            let (value, _) = entries[0].clone();
            let subject_entity_id = key.split('|').next().unwrap_or_default().to_owned();
            let mut provenance = Vec::new();
            for (_, sources) in entries {
                for source in sources {
                    check_cancelled(&mut cancelled)?;
                    provenance.push(source);
                }
            }
            state.facts.push(Fact {
                semantic_id: derived_id(
                    "fact-single",
                    &json!([
                        canonical_uuid(expected_brain_id),
                        &subject_entity_id,
                        "person.birth_date"
                    ]),
                )?,
                subject_entity_id,
                predicate: "person.birth_date".into(),
                value,
                provenance,
            });
        }
    }
    for entries in by_multi.into_values() {
        check_cancelled(&mut cancelled)?;
        let (subject_entity_id, predicate, value, _) = entries[0].clone();
        let mut provenance = Vec::new();
        for (_, _, _, sources) in entries {
            for source in sources {
                check_cancelled(&mut cancelled)?;
                provenance.push(source);
            }
        }
        state.facts.push(Fact {
            semantic_id: derived_id(
                "fact-multi",
                &json!([
                    canonical_uuid(expected_brain_id),
                    &subject_entity_id,
                    &predicate,
                    value_json(&value)
                ]),
            )?,
            subject_entity_id,
            predicate,
            value,
            provenance,
        });
    }
    let mut relation_groups =
        BTreeMap::<String, Vec<(String, String, String, Vec<Provenance>)>>::new();
    for (subject, predicate, object, provenance) in relation_candidates {
        check_cancelled(&mut cancelled)?;
        let key = format!("{subject}|{predicate}|{object}");
        relation_groups
            .entry(key)
            .or_default()
            .push((subject, predicate, object, provenance));
    }
    state.entities.reserve(entities.len());
    for entity in entities.into_values() {
        check_cancelled(&mut cancelled)?;
        state.entities.push(entity);
    }
    sort_by_cancellable(
        &mut state.entities,
        |a, b| a.entity_id.cmp(&b.entity_id),
        &mut cancelled,
    )?;
    sort_by_cancellable(
        &mut state.aliases,
        |a, b| {
            a.normalized_key
                .cmp(&b.normalized_key)
                .then(a.entity_id.cmp(&b.entity_id))
                .then(a.spelling.cmp(&b.spelling))
        },
        &mut cancelled,
    )?;
    state.relations.reserve(relation_groups.len());
    for (_, entries) in relation_groups {
        check_cancelled(&mut cancelled)?;
        let (subject_entity_id, predicate, object_entity_id, _) = entries[0].clone();
        let mut provenance = Vec::new();
        for (_, _, _, sources) in entries {
            for source in sources {
                check_cancelled(&mut cancelled)?;
                provenance.push(source);
            }
        }
        state.relations.push(Relation {
            semantic_id: derived_id(
                "relation",
                &json!([
                    canonical_uuid(expected_brain_id),
                    &subject_entity_id,
                    &predicate,
                    &object_entity_id
                ]),
            )?,
            subject_entity_id,
            predicate,
            object_entity_id,
            provenance,
        });
    }
    sort_by_cancellable(
        &mut state.facts,
        |a, b| {
            a.subject_entity_id
                .cmp(&b.subject_entity_id)
                .then(a.predicate.cmp(&b.predicate))
                .then(a.semantic_id.cmp(&b.semantic_id))
        },
        &mut cancelled,
    )?;
    sort_by_cancellable(
        &mut state.relations,
        |a, b| {
            a.subject_entity_id
                .cmp(&b.subject_entity_id)
                .then(a.predicate.cmp(&b.predicate))
                .then(a.object_entity_id.cmp(&b.object_entity_id))
        },
        &mut cancelled,
    )?;
    sort_by_cancellable(
        &mut state.conflicts,
        |a, b| {
            a.kind
                .cmp(b.kind)
                .then(a.key.cmp(&b.key))
                .then(a.event_ids.cmp(&b.event_ids))
        },
        &mut cancelled,
    )?;
    sort_by_cancellable(
        &mut state.pending,
        |a, b| {
            a.target_id
                .cmp(&b.target_id)
                .then(a.event_id.cmp(&b.event_id))
                .then(a.reason.cmp(b.reason))
        },
        &mut cancelled,
    )?;
    sort_by_cancellable(
        &mut state.corrections,
        |a, b| a.correction_event_id.cmp(&b.correction_event_id),
        &mut cancelled,
    )?;
    let mut unresolved = BTreeSet::new();
    for pending in &state.pending {
        check_cancelled(&mut cancelled)?;
        unresolved.insert(pending.event_id.clone());
    }
    for record in &records {
        check_cancelled(&mut cancelled)?;
        let event_id = canonical_uuid(&record.event().event_id);
        if !unresolved.contains(&event_id) && !invalid_event_ids.contains(&event_id) {
            state.materialized_event_ids.push(event_id);
        }
    }
    sort_by_cancellable(&mut state.materialized_event_ids, Ord::cmp, &mut cancelled)?;
    Ok(state)
}

/// Stable merge sort with a cancellation check in every merge pass. Standard
/// slice sorting cannot be interrupted while a large history is being ordered.
fn sort_by_cancellable<T, F, C>(
    values: &mut Vec<T>,
    mut compare: F,
    cancelled: &mut C,
) -> Result<(), ReducerError>
where
    F: FnMut(&T, &T) -> Ordering,
    C: FnMut() -> bool,
{
    let len = values.len();
    if len < 2 {
        return if cancelled() {
            Err(ReducerError::Cancelled)
        } else {
            Ok(())
        };
    }
    let mut order = Vec::with_capacity(len);
    for index in 0..len {
        if cancelled() {
            return Err(ReducerError::Cancelled);
        }
        order.push(index);
    }
    let mut scratch = Vec::with_capacity(len);
    for _ in 0..len {
        if cancelled() {
            return Err(ReducerError::Cancelled);
        }
        scratch.push(0usize);
    }
    let mut width = 1usize;
    while width < len {
        let mut start = 0usize;
        while start < len {
            let middle = start.saturating_add(width).min(len);
            let end = middle.saturating_add(width).min(len);
            let (mut left, mut right, mut out) = (start, middle, start);
            while left < middle && right < end {
                if cancelled() {
                    return Err(ReducerError::Cancelled);
                }
                if compare(&values[order[left]], &values[order[right]]) != Ordering::Greater {
                    scratch[out] = order[left];
                    left += 1;
                } else {
                    scratch[out] = order[right];
                    right += 1;
                }
                out += 1;
            }
            while left < middle {
                if cancelled() {
                    return Err(ReducerError::Cancelled);
                }
                scratch[out] = order[left];
                left += 1;
                out += 1;
            }
            while right < end {
                if cancelled() {
                    return Err(ReducerError::Cancelled);
                }
                scratch[out] = order[right];
                right += 1;
                out += 1;
            }
            start = end;
        }
        std::mem::swap(&mut order, &mut scratch);
        width = width.saturating_mul(2);
    }
    if cancelled() {
        return Err(ReducerError::Cancelled);
    }
    let mut source: Vec<Option<T>> = Vec::with_capacity(len);
    for value in std::mem::take(values) {
        if cancelled() {
            return Err(ReducerError::Cancelled);
        }
        source.push(Some(value));
    }
    let mut sorted = Vec::with_capacity(len);
    for index in order {
        if cancelled() {
            return Err(ReducerError::Cancelled);
        }
        sorted.push(source[index].take().expect("each sort index is unique"));
    }
    *values = sorted;
    Ok(())
}

fn provenance(event: &cryptex_events::V2KnowledgeAssertionEvent) -> Provenance {
    let (authorization, confidence) = match &event.payload {
        KnowledgeAssertionPayload::EntityDeclare {
            publication_authorization,
            ..
        }
        | KnowledgeAssertionPayload::EntityAlias {
            publication_authorization,
            ..
        }
        | KnowledgeAssertionPayload::EntityMerge {
            publication_authorization,
            ..
        }
        | KnowledgeAssertionPayload::EntityMergeRevoke {
            publication_authorization,
            ..
        }
        | KnowledgeAssertionPayload::AssertionCorrect {
            publication_authorization,
            ..
        }
        | KnowledgeAssertionPayload::AssertionResolveConflict {
            publication_authorization,
            ..
        } => (publication_authorization, None),
        KnowledgeAssertionPayload::FactAssert {
            publication_authorization,
            confidence,
            ..
        }
        | KnowledgeAssertionPayload::RelationAssert {
            publication_authorization,
            confidence,
            ..
        } => (publication_authorization, Some(*confidence)),
    };
    Provenance {
        event_id: canonical_uuid(&event.event_id),
        device_id: event.device_id.clone(),
        device_seq: event.device_seq.to_u64().unwrap_or_default(),
        signing_key_id: event.signing_key_id.as_str().to_owned(),
        confidence,
        authorization_receipt_id: canonical_uuid(&authorization.receipt_id),
        classification: event.classification,
    }
}

fn verified_record_event_id(record: &VersionedRecord) -> Option<&str> {
    match record {
        VersionedRecord::VerifiedV1(event) => Some(&event.event_id),
        VersionedRecord::VerifiedV2Supported(event) => Some(&event.event().event_id),
        VersionedRecord::VerifiedUnsupported(event) => Some(&event.event_id),
        VersionedRecord::VerifiedSecurityV3(_) => None,
        VersionedRecord::Unverifiable(_) => None,
    }
}

fn stable_event_cmp(
    a: &cryptex_events::V2KnowledgeAssertionEvent,
    b: &cryptex_events::V2KnowledgeAssertionEvent,
) -> Ordering {
    a.device_id
        .as_bytes()
        .cmp(b.device_id.as_bytes())
        .then_with(|| {
            a.device_seq
                .to_u64()
                .unwrap_or_default()
                .cmp(&b.device_seq.to_u64().unwrap_or_default())
        })
        .then_with(|| uuid_bytes(&a.event_id).cmp(&uuid_bytes(&b.event_id)))
}

fn uuid_bytes(id: &str) -> Vec<u8> {
    let compact: Vec<u8> = id.bytes().filter(|byte| *byte != b'-').collect();
    compact
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            u8::from_str_radix(std::str::from_utf8(pair).unwrap_or("00"), 16).unwrap_or_default()
        })
        .collect()
}

fn canonical_uuid(id: &str) -> String {
    id.to_ascii_lowercase()
}

fn graph_has_cycle<C>(
    graph: &BTreeMap<String, BTreeSet<String>>,
    cancelled: &mut C,
) -> Result<bool, ReducerError>
where
    C: FnMut() -> bool,
{
    let mut indegree = BTreeMap::new();
    for node in graph.keys() {
        if cancelled() {
            return Err(ReducerError::Cancelled);
        }
        indegree.insert(node.clone(), 0usize);
    }
    for dependencies in graph.values() {
        if cancelled() {
            return Err(ReducerError::Cancelled);
        }
        for dependency in dependencies {
            if cancelled() {
                return Err(ReducerError::Cancelled);
            }
            if let Some(count) = indegree.get_mut(dependency) {
                *count += 1;
            }
        }
    }
    let mut ready = BTreeSet::new();
    for (node, count) in &indegree {
        if cancelled() {
            return Err(ReducerError::Cancelled);
        }
        if *count == 0 {
            ready.insert(node.clone());
        }
    }
    let mut visited_count = 0usize;
    while let Some(node) = ready.pop_first() {
        if cancelled() {
            return Err(ReducerError::Cancelled);
        }
        visited_count += 1;
        if let Some(dependencies) = graph.get(&node) {
            for dependency in dependencies {
                if cancelled() {
                    return Err(ReducerError::Cancelled);
                }
                if let Some(count) = indegree.get_mut(dependency) {
                    *count -= 1;
                    if *count == 0 {
                        ready.insert(dependency.clone());
                    }
                }
            }
        }
    }
    Ok(visited_count != graph.len())
}

fn topological_nodes_cancellable<C>(
    graph: &BTreeMap<String, BTreeSet<String>>,
    cancelled: &mut C,
) -> Result<Vec<String>, ReducerError>
where
    C: FnMut() -> bool,
{
    let mut visited = BTreeSet::new();
    let mut output = Vec::with_capacity(graph.len());
    for node in graph.keys() {
        if cancelled() {
            return Err(ReducerError::Cancelled);
        }
        if !visited.insert(node.clone()) {
            continue;
        }
        let mut stack = Vec::new();
        if let Some(dependencies) = graph.get(node) {
            stack.push((node.as_str(), dependencies.iter()));
        } else {
            output.push(node.clone());
            continue;
        }
        while !stack.is_empty() {
            if cancelled() {
                return Err(ReducerError::Cancelled);
            }
            let dependency = stack
                .last_mut()
                .and_then(|(_, dependencies)| dependencies.next())
                .map(String::as_str);
            match dependency {
                Some(dependency) if visited.insert(dependency.to_owned()) => {
                    if let Some(dependencies) = graph.get(dependency) {
                        stack.push((dependency, dependencies.iter()));
                    } else {
                        output.push(dependency.to_owned());
                    }
                }
                Some(_) => {}
                None => {
                    let (finished, _) = stack.pop().expect("nonempty traversal stack");
                    output.push(finished.to_owned());
                }
            }
        }
    }
    Ok(output)
}

fn normalize_alias(value: &str) -> String {
    let nfc: String = value.nfc().collect();
    let trimmed = nfc.trim_matches(is_unicode_16_whitespace);
    unicode16_casefold::fold(trimmed)
}

fn is_unicode_16_whitespace(ch: char) -> bool {
    matches!(ch, '\u{0009}'..='\u{000D}' | '\u{0020}' | '\u{0085}' | '\u{00A0}' | '\u{1680}'
        | '\u{2000}'..='\u{200A}' | '\u{2028}' | '\u{2029}' | '\u{202F}' | '\u{205F}' | '\u{3000}')
}

struct EntityUnionFind {
    parent: BTreeMap<String, String>,
    size: BTreeMap<String, usize>,
    minimum_id: BTreeMap<String, String>,
}

impl EntityUnionFind {
    fn new<'a, I, C>(ids: I, cancelled: &mut C) -> Result<Self, ReducerError>
    where
        I: IntoIterator<Item = &'a String>,
        C: FnMut() -> bool,
    {
        let mut parent = BTreeMap::new();
        let mut size = BTreeMap::new();
        let mut minimum_id = BTreeMap::new();
        for id in ids {
            if cancelled() {
                return Err(ReducerError::Cancelled);
            }
            parent.insert(id.clone(), id.clone());
            size.insert(id.clone(), 1);
            minimum_id.insert(id.clone(), id.clone());
        }
        Ok(Self {
            parent,
            size,
            minimum_id,
        })
    }

    fn root<C>(&mut self, id: &str, cancelled: &mut C) -> Result<String, ReducerError>
    where
        C: FnMut() -> bool,
    {
        let mut current = id.to_owned();
        let mut path = Vec::new();
        loop {
            if cancelled() {
                return Err(ReducerError::Cancelled);
            }
            let parent = self
                .parent
                .get(&current)
                .ok_or_else(|| ReducerError::InvalidEvent(id.to_owned()))?;
            if parent == &current {
                break;
            }
            path.push(current);
            current = parent.clone();
        }
        for item in path {
            if cancelled() {
                return Err(ReducerError::Cancelled);
            }
            self.parent.insert(item, current.clone());
        }
        Ok(current)
    }

    fn union_all<C>(&mut self, ids: &[String], cancelled: &mut C) -> Result<(), ReducerError>
    where
        C: FnMut() -> bool,
    {
        let Some(first) = ids.first() else {
            return Err(ReducerError::InvalidEvent(String::new()));
        };
        let mut root = self.root(first, cancelled)?;
        for id in ids.iter().skip(1) {
            if cancelled() {
                return Err(ReducerError::Cancelled);
            }
            let candidate = self.root(id, cancelled)?;
            if candidate == root {
                continue;
            }
            let root_size = *self
                .size
                .get(&root)
                .ok_or_else(|| ReducerError::InvalidEvent(root.clone()))?;
            let candidate_size = *self
                .size
                .get(&candidate)
                .ok_or_else(|| ReducerError::InvalidEvent(candidate.clone()))?;
            let (parent, child) = if root_size > candidate_size
                || (root_size == candidate_size && root < candidate)
            {
                (root, candidate)
            } else {
                (candidate, root)
            };
            let combined_size = root_size.saturating_add(candidate_size);
            let parent_min = self
                .minimum_id
                .get(&parent)
                .cloned()
                .ok_or_else(|| ReducerError::InvalidEvent(parent.clone()))?;
            let child_min = self
                .minimum_id
                .remove(&child)
                .ok_or_else(|| ReducerError::InvalidEvent(child.clone()))?;
            self.parent.insert(child.clone(), parent.clone());
            self.size.insert(parent.clone(), combined_size);
            self.size.remove(&child);
            self.minimum_id
                .insert(parent.clone(), parent_min.min(child_min));
            root = parent;
        }
        Ok(())
    }

    fn representative<C>(&mut self, id: &str, cancelled: &mut C) -> Result<String, ReducerError>
    where
        C: FnMut() -> bool,
    {
        let root = self.root(id, cancelled)?;
        self.minimum_id
            .get(&root)
            .cloned()
            .ok_or_else(|| ReducerError::InvalidEvent(id.to_owned()))
    }
}

fn value_json(value: &FactValue) -> serde_json::Value {
    match value {
        FactValue::Text(s) => json!(["text", s]),
        FactValue::Decimal(s) => json!(["decimal", s]),
        FactValue::Boolean(b) => json!(["boolean", b]),
        FactValue::Date(s) => json!(["date", s]),
        FactValue::EntityRef(s) => json!(["entity_ref", canonical_uuid(s)]),
    }
}

fn semantic_key(subject: &str, predicate: &str, value: &FactValue) -> Result<String, ReducerError> {
    serde_json_canonicalizer::to_string(&json!([subject, predicate, value_json(value)]))
        .map_err(|_| ReducerError::CanonicalizationFailed)
}

fn semantic_fingerprint(value: Option<&SemanticAssertion>) -> Result<String, ReducerError> {
    let value = value.map_or_else(
        || json!(["withdraw"]),
        |assertion| match assertion {
            SemanticAssertion::Fact {
                subject_entity_id,
                predicate,
                value,
                confidence,
            } => json!([
                "fact",
                canonical_uuid(subject_entity_id),
                predicate,
                value_json(value),
                confidence_name(*confidence)
            ]),
            SemanticAssertion::Relation {
                subject_entity_id,
                predicate,
                object_entity_id,
                confidence,
            } => json!([
                "relation",
                canonical_uuid(subject_entity_id),
                predicate,
                canonical_uuid(object_entity_id),
                confidence_name(*confidence)
            ]),
        },
    );
    serde_json_canonicalizer::to_string(&value).map_err(|_| ReducerError::CanonicalizationFailed)
}

fn confidence_name(confidence: Confidence) -> &'static str {
    match confidence {
        Confidence::Confirmed => "confirmed",
        Confidence::Inferred => "inferred",
        Confidence::Uncertain => "uncertain",
    }
}

fn derived_id(domain: &str, key: &serde_json::Value) -> Result<String, ReducerError> {
    let canonical =
        serde_json_canonicalizer::to_vec(key).map_err(|_| ReducerError::CanonicalizationFailed)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"cryptex-m4-derived-id-v1\0");
    hasher.update(domain.as_bytes());
    hasher.update(b"\0");
    hasher.update(&canonical);
    let mut hex = String::with_capacity(64);
    for byte in hasher.finalize().as_bytes() {
        use std::fmt::Write as _;
        write!(&mut hex, "{byte:02x}").map_err(|_| ReducerError::CanonicalizationFailed)?;
    }
    Ok(hex)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::VerifyingKey;
    use serde_json::Value;

    const BRAIN: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    const PERSON_A: &str = "11111111-1111-4111-8111-111111111111";
    const PERSON_B: &str = "22222222-2222-4222-8222-222222222222";
    const RECEIPT: &str = "99999999-9999-4999-8999-999999999999";

    struct SyntheticTrustedDaemonKey(VerifyingKey);

    impl cryptex_events::TrustedDaemonKeyResolver for SyntheticTrustedDaemonKey {
        fn trusted_daemon_key(&mut self, _device_id: &str, _key_id: &str) -> Option<VerifyingKey> {
            Some(self.0)
        }
    }

    fn event(id_nibble: char, seq: u64, value: Value) -> Vec<u8> {
        let device_id = format!("device-{id_nibble}");
        event_on_device(id_nibble, seq, value, &device_id)
    }

    fn event_on_device(id_nibble: char, seq: u64, value: Value, device_id: &str) -> Vec<u8> {
        event_with_brain(id_nibble, seq, value, device_id, BRAIN)
    }

    fn event_with_brain(
        id_nibble: char,
        seq: u64,
        value: Value,
        device_id: &str,
        brain_id: &str,
    ) -> Vec<u8> {
        use ed25519_dalek::{Signer, SigningKey};

        let id = format!("{id_nibble}1111111-1111-4111-8111-111111111111");
        let key = SigningKey::from_bytes(&[0x42; 32]);
        let key_id =
            cryptex_events::signing_key_id_from_verifying_key(&key.verifying_key()).unwrap();
        let mut raw_value = json!({
            "schema_version": 2,
            "event_id": id,
            "brain_id": brain_id,
            "device_id": device_id,
            "device_seq": seq.to_string(),
            "created_at": "2026-10-08T00:00:00Z",
            "event_type": "knowledge.assertion",
            "payload_schema_version": 1,
            "classification": "internal",
            "signing_key_id": key_id.as_str(),
            "payload": value,
            "prev_content_hash": "00".repeat(32),
            "content_hash": "00".repeat(32),
            "signature": "00".repeat(64)
        });
        let signable = {
            let mut value = raw_value.clone();
            value.as_object_mut().unwrap().remove("content_hash");
            value.as_object_mut().unwrap().remove("signature");
            serde_json_canonicalizer::to_vec(&value).unwrap()
        };
        let mut hasher = blake3::Hasher::new();
        hasher.update(cryptex_events::V2_CONTENT_HASH_DOMAIN);
        hasher.update(&[0]);
        hasher.update(&signable);
        let content_hash = *hasher.finalize().as_bytes();
        let signature = key.sign(
            &[
                cryptex_events::V2_SIGNATURE_DOMAIN,
                &[0],
                content_hash.as_slice(),
            ]
            .concat(),
        );
        let encode_hex = |bytes: &[u8]| {
            bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        };
        raw_value["content_hash"] = Value::String(encode_hex(&content_hash));
        raw_value["signature"] = Value::String(encode_hex(&signature.to_bytes()));
        let raw_record = serde_json_canonicalizer::to_vec(&raw_value).unwrap();
        let cryptex_events::VersionedRecord::VerifiedV2Supported(record) =
            cryptex_events::read_versioned_record(&raw_record, &key.verifying_key()).unwrap()
        else {
            panic!("expected cryptographically verified v2 assertion");
        };
        assert_eq!(record.raw_record(), raw_record);
        raw_record
    }

    fn verified_history(records: &[Vec<u8>]) -> VerifiedV2History {
        use ed25519_dalek::SigningKey;
        let key = SigningKey::from_bytes(&[0x42; 32]).verifying_key();
        let raw_records: Vec<_> = records.iter().map(Vec::as_slice).collect();
        cryptex_events::verify_v2_history(&raw_records, SyntheticTrustedDaemonKey(key))
            .expect("synthetic test event history must verify")
    }

    fn reduce_raw(brain_id: &str, records: &[Vec<u8>]) -> Result<KnowledgeState, ReducerError> {
        let history = verified_history(records);
        reduce_verified_events(brain_id, &history)
    }

    fn raw_event_id(record: &[u8]) -> String {
        serde_json::from_slice::<Value>(record).unwrap()["event_id"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    fn auth() -> Value {
        json!({"basis":"direct_user","receipt_id":RECEIPT})
    }
    fn declare(id: &str, label: &str) -> Value {
        json!({
            "kind":"entity.declare","entity_id":id,"entity_kind":"person","label":label,
            "publication_authorization":auth()
        })
    }
    fn fact(subject: &str, predicate: &str, value: Value, confidence: &str) -> Value {
        json!({
            "kind":"fact.assert","subject_entity_id":subject,"predicate":predicate,"value":value,
            "confidence":confidence,"publication_authorization":auth()
        })
    }

    #[test]
    fn replay_order_and_identical_event_duplicates_are_deterministic() {
        let a = event('1', 1, declare(PERSON_A, "Ada"));
        let b = event(
            '2',
            2,
            fact(
                PERSON_A,
                "favorite.color",
                json!({"kind":"text","value":"Blue"}),
                "inferred",
            ),
        );
        let c = event(
            '3',
            3,
            fact(
                PERSON_A,
                "favorite.color",
                json!({"kind":"text","value":"Blue"}),
                "uncertain",
            ),
        );
        let forward = reduce_raw(BRAIN, &[a.clone(), b.clone(), c.clone(), b.clone()]).unwrap();
        let reverse = reduce_raw(BRAIN, &[c, b, a]).unwrap();
        assert_eq!(forward, reverse);
        assert_eq!(forward.facts.len(), 1);
        assert_eq!(forward.facts[0].provenance.len(), 2);
        assert_eq!(forward.materialized_event_ids.len(), 3);
    }

    #[test]
    fn security_history_reducer_applies_cutoff_before_knowledge_materialization() {
        use ed25519_dalek::SigningKey;
        use std::collections::BTreeMap;

        let raw = event('1', 1, declare(PERSON_A, "Ada"));
        let key = SigningKey::from_bytes(&[0x42; 32]).verifying_key();
        let history = cryptex_events::verify_security_history(
            &[raw.as_slice()],
            BRAIN,
            SyntheticTrustedDaemonKey(key),
        )
        .unwrap();

        let excluded = reduce_verified_security_history(BRAIN, &history, &BTreeMap::new()).unwrap();
        assert!(excluded.entities.is_empty());
        assert!(excluded.materialized_event_ids.is_empty());

        let included = reduce_verified_security_history(
            BRAIN,
            &history,
            &BTreeMap::from([("device-1".to_owned(), 1)]),
        )
        .unwrap();
        assert_eq!(included.entities.len(), 1);
        assert_eq!(included.materialized_event_ids.len(), 1);
        assert_eq!(
            reduce_verified_security_history_cancellable(
                BRAIN,
                &history,
                &BTreeMap::from([("device-1".to_owned(), 1)]),
                || true,
            ),
            Err(ReducerError::Cancelled)
        );
    }

    #[test]
    fn sqlite_rebuild_rejects_frame_positions_not_bound_to_verified_records() {
        let raw = event('1', 1, declare(PERSON_A, "Ada"));
        let history = verified_history(&[raw]);
        let VersionedRecord::VerifiedV2Supported(record) = &history.records()[0] else {
            panic!("expected supported verified v2 event");
        };
        let event = record.event();
        let position = sqlite::CanonicalFramePosition {
            event_id: event.event_id.clone(),
            device_id: event.device_id.clone(),
            pack_index: 0,
            frame_offset: 0,
            content_hash: event.content_hash.as_str().to_owned(),
        };
        let progress = sqlite::DeviceMaterializationProgress {
            device_id: event.device_id.clone(),
            integrity_position: Some(position.clone()),
            safe_position: Some(position.clone()),
            blocking_reason: None,
        };
        let mut db = cryptex_projection::ProjectionDb::open_in_memory().unwrap();
        sqlite::migrate_knowledge_schema(db.connection_mut()).unwrap();
        let mut mismatched = position.clone();
        mismatched.content_hash = "0".repeat(64);
        let result = sqlite::rebuild_from_verified_history(
            db.connection_mut(),
            BRAIN,
            &history,
            &[mismatched],
            std::slice::from_ref(&progress),
        );
        assert!(matches!(
            result,
            Err(sqlite::KnowledgeStoreError::Schema(_))
        ));
        let status: String = db
            .connection()
            .query_row(
                "SELECT rebuild_status FROM knowledge_meta WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "not_built");

        sqlite::rebuild_from_verified_history(
            db.connection_mut(),
            BRAIN,
            &history,
            &[position],
            &[progress],
        )
        .unwrap();
        let status: String = db
            .connection()
            .query_row(
                "SELECT rebuild_status FROM knowledge_meta WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "ready");
    }

    #[test]
    fn birth_date_disagreement_conflicts_but_unknown_predicates_are_multivalued() {
        let records = vec![
            event('1', 1, declare(PERSON_A, "Ada")),
            event(
                '2',
                2,
                fact(
                    PERSON_A,
                    "person.birth_date",
                    json!({"kind":"date","value":"1815-12-10"}),
                    "confirmed",
                ),
            ),
            event(
                '3',
                3,
                fact(
                    PERSON_A,
                    "person.birth_date",
                    json!({"kind":"date","value":"1816-12-10"}),
                    "confirmed",
                ),
            ),
            event(
                '4',
                4,
                fact(
                    PERSON_A,
                    "future.measure",
                    json!({"kind":"decimal","value":"1"}),
                    "inferred",
                ),
            ),
            event(
                '5',
                5,
                fact(
                    PERSON_A,
                    "future.measure",
                    json!({"kind":"decimal","value":"2"}),
                    "inferred",
                ),
            ),
        ];
        let result = reduce_raw(BRAIN, &records).unwrap();
        assert!(
            result
                .facts
                .iter()
                .all(|f| f.predicate != "person.birth_date")
        );
        assert!(
            result
                .conflicts
                .iter()
                .any(|c| c.kind == "single_valued_fact")
        );
        assert_eq!(
            result
                .facts
                .iter()
                .filter(|f| f.predicate == "future.measure")
                .count(),
            2
        );
    }

    #[test]
    fn alias_matching_uses_unicode_16_full_casefold_without_fuzzy_matching() {
        let records = vec![
            event('1', 1, declare(PERSON_A, "Straße")),
            event(
                '2',
                2,
                json!({"kind":"entity.alias","entity_id":PERSON_A,"alias":"Straße","publication_authorization":auth()}),
            ),
            event(
                '4',
                4,
                json!({"kind":"entity.alias","entity_id":PERSON_A,"alias":"STRASSE","publication_authorization":auth()}),
            ),
            event(
                '3',
                3,
                json!({"kind":"entity.alias","entity_id":PERSON_A,"alias":"Strasse!","publication_authorization":auth()}),
            ),
        ];
        let result = reduce_raw(BRAIN, &records).unwrap();
        assert_eq!(result.aliases.len(), 2);
        assert!(
            result
                .aliases
                .iter()
                .any(|alias| alias.provenance.len() == 2)
        );
        assert!(
            result
                .aliases
                .iter()
                .any(|alias| alias.provenance.len() == 1)
        );
        assert!(
            result
                .aliases
                .iter()
                .any(|alias| alias.spelling.ends_with('!'))
        );
    }

    #[test]
    fn explicit_merge_consolidates_representative_but_missing_refs_stay_pending() {
        let merge_event = event(
            '4',
            4,
            json!({"kind":"entity.merge","entity_ids":[PERSON_A,PERSON_B],"publication_authorization":auth()}),
        );
        let records = vec![
            event('1', 1, declare(PERSON_A, "Ada")),
            event('2', 2, declare(PERSON_B, "Ada Lovelace")),
            merge_event,
            event(
                '5',
                5,
                fact(
                    "33333333-3333-4333-8333-333333333333",
                    "unknown.ref",
                    json!({"kind":"boolean","value":true}),
                    "inferred",
                ),
            ),
        ];
        let result = reduce_raw(BRAIN, &records).unwrap();
        assert_eq!(result.entities[0].representative_id, PERSON_A);
        assert_eq!(result.entities[1].representative_id, PERSON_A);
        assert_eq!(result.entity_merges.len(), 1);
        assert_eq!(result.entity_merges[0].status, EntityMergeStatus::Active);
        assert!(result.pending.iter().any(|p| p.reason == "missing_entity"));
        assert!(
            !result
                .materialized_event_ids
                .iter()
                .any(|id| id.starts_with('5'))
        );
    }

    #[test]
    fn same_labels_do_not_merge_and_entity_refs_follow_only_explicit_merge() {
        let records = vec![
            event('1', 1, declare(PERSON_A, "Alex")),
            event('2', 2, declare(PERSON_B, "Alex")),
            event(
                '3',
                3,
                fact(
                    PERSON_A,
                    "mentor",
                    json!({"kind":"entity_ref","value":PERSON_B}),
                    "inferred",
                ),
            ),
        ];
        let before_merge = reduce_raw(BRAIN, &records).unwrap();
        assert_ne!(
            before_merge.entities[0].representative_id,
            before_merge.entities[1].representative_id
        );
        assert_eq!(
            before_merge.facts[0].value,
            FactValue::EntityRef(PERSON_B.into())
        );

        let mut after_records = records;
        after_records.push(event('4', 4, json!({
            "kind":"entity.merge","entity_ids":[PERSON_A,PERSON_B],"publication_authorization":auth()
        })));
        let after_merge = reduce_raw(BRAIN, &after_records).unwrap();
        assert_eq!(
            after_merge.facts[0].value,
            FactValue::EntityRef(PERSON_A.into())
        );
    }

    #[test]
    fn revoked_entity_merge_splits_classes_but_preserves_merge_and_revoke_provenance() {
        let merge_id = "31111111-1111-4111-8111-111111111111";
        let records = vec![
            event('1', 1, declare(PERSON_A, "Ada")),
            event('2', 2, declare(PERSON_B, "Ada L.")),
            event(
                '3',
                3,
                json!({
                    "kind":"entity.merge","entity_ids":[PERSON_A,PERSON_B],"publication_authorization":auth()
                }),
            ),
            event(
                '4',
                4,
                json!({
                    "kind":"entity.merge_revoke","target_merge_event_id":merge_id,"publication_authorization":auth()
                }),
            ),
        ];
        let result = reduce_raw(BRAIN, &records).unwrap();
        assert_ne!(
            result.entities[0].representative_id,
            result.entities[1].representative_id
        );
        assert_eq!(result.entity_merges[0].status, EntityMergeStatus::Revoked);
        assert_eq!(result.entity_merges[0].provenance.len(), 2);
    }

    #[test]
    fn merge_revoke_to_known_non_merge_event_is_invalid_not_pending() {
        let declaration = event('1', 1, declare(PERSON_A, "Ada"));
        let revoke = event(
            '2',
            2,
            json!({
                "kind":"entity.merge_revoke",
                "target_merge_event_id":"11111111-1111-4111-8111-111111111111",
                "publication_authorization":auth()
            }),
        );
        let revoke_id = raw_event_id(&revoke);
        let result = reduce_raw(BRAIN, &[declaration, revoke]).unwrap();

        assert!(
            result
                .pending
                .iter()
                .all(|pending| pending.reason != "missing_merge")
        );
        assert!(result.conflicts.iter().any(|conflict| {
            conflict.kind == "invalid_merge_revoke_reference"
                && conflict.key == "11111111-1111-4111-8111-111111111111"
        }));
        assert!(!result.materialized_event_ids.contains(&revoke_id));
    }

    #[test]
    fn same_alias_for_independent_entities_is_ambiguous_not_a_merge() {
        let result = reduce_raw(BRAIN, &[
            event('1', 1, declare(PERSON_A, "Ada")),
            event('2', 2, declare(PERSON_B, "Grace")),
            event('3', 3, json!({"kind":"entity.alias","entity_id":PERSON_A,"alias":"Engineer","publication_authorization":auth()})),
            event('4', 4, json!({"kind":"entity.alias","entity_id":PERSON_B,"alias":"engineer","publication_authorization":auth()})),
        ]).unwrap();
        assert!(result.aliases.is_empty());
        assert_eq!(
            result
                .conflicts
                .iter()
                .filter(|conflict| conflict.kind == "ambiguous_alias")
                .count(),
            1
        );
        assert_ne!(
            result.entities[0].representative_id,
            result.entities[1].representative_id
        );
    }

    #[test]
    fn alias_class_is_withheld_while_any_target_is_unresolved() {
        let records = [
            event('1', 1, declare(PERSON_A, "Ada")),
            event(
                '2',
                2,
                json!({"kind":"entity.alias","entity_id":PERSON_A,"alias":"Engineer","publication_authorization":auth()}),
            ),
            event(
                '3',
                3,
                json!({"kind":"entity.alias","entity_id":PERSON_B,"alias":"engineer","publication_authorization":auth()}),
            ),
        ];
        let reduced = reduce_raw(BRAIN, &records).unwrap();
        assert!(reduced.aliases.is_empty());
        assert!(reduced.pending.iter().any(|item| {
            item.event_id == "31111111-1111-4111-8111-111111111111"
                && item.reason == "missing_entity"
        }));
    }

    #[test]
    fn late_cross_device_entity_dependency_replays_pending_fact() {
        let pending_fact = event(
            '1',
            1,
            fact(
                "33333333-3333-4333-8333-333333333333",
                "project.role",
                json!({"kind":"text","value":"owner"}),
                "inferred",
            ),
        );
        let late_entity = event_on_device(
            '2',
            1,
            declare("33333333-3333-4333-8333-333333333333", "Project X"),
            "device-b",
        );
        let before = reduce_raw(BRAIN, std::slice::from_ref(&pending_fact)).unwrap();
        assert!(before.facts.is_empty());
        assert!(
            before
                .pending
                .iter()
                .any(|pending| pending.reason == "missing_entity")
        );
        assert!(
            !before
                .materialized_event_ids
                .contains(&raw_event_id(&pending_fact))
        );

        let after = reduce_raw(BRAIN, &[pending_fact.clone(), late_entity.clone()]).unwrap();
        let permuted = reduce_raw(BRAIN, &[late_entity, pending_fact]).unwrap();
        assert_eq!(after, permuted);
        assert_eq!(after.facts.len(), 1);
        assert!(after.pending.is_empty());
    }

    #[test]
    fn cancelled_reduction_returns_no_partial_knowledge_state() {
        let raw = event('1', 1, declare(PERSON_A, "Synthetic"));
        let key = ed25519_dalek::SigningKey::from_bytes(&[0x42; 32]).verifying_key();
        let history = cryptex_events::verify_v2_history(&[&raw], SyntheticTrustedDaemonKey(key))
            .expect("synthetic fixture verifies");
        assert_eq!(
            reduce_verified_events_cancellable(BRAIN, &history, || true),
            Err(ReducerError::Cancelled),
        );
    }

    #[test]
    fn large_sort_is_cancellable_without_returning_partial_order() {
        let mut values: Vec<_> = (0..4096).rev().collect();
        let mut checks = 0usize;
        let result = sort_by_cancellable(&mut values, Ord::cmp, &mut || {
            checks += 1;
            checks > 64
        });
        assert_eq!(result, Err(ReducerError::Cancelled));
        assert!(checks > 64);
        assert_eq!(values, (0..4096).rev().collect::<Vec<_>>());
    }

    fn synthetic_resolution_graph(size: usize) -> BTreeMap<String, BTreeSet<String>> {
        (0..size)
            .map(|index| {
                let id = format!("{index:08}");
                let dependencies = if index + 1 < size {
                    BTreeSet::from([format!("{:08}", index + 1)])
                } else {
                    BTreeSet::new()
                };
                (id, dependencies)
            })
            .collect()
    }

    #[test]
    fn entity_union_find_is_order_independent_and_cancellable() {
        let ids: Vec<_> = (0..2_048)
            .map(|index| format!("entity-{index:04}"))
            .collect();
        let mut forward = EntityUnionFind::new(ids.iter(), &mut || false).unwrap();
        forward.union_all(&ids, &mut || false).unwrap();
        let mut reverse = EntityUnionFind::new(ids.iter(), &mut || false).unwrap();
        let reverse_ids: Vec<_> = ids.iter().rev().cloned().collect();
        reverse.union_all(&reverse_ids, &mut || false).unwrap();
        assert_eq!(
            forward.representative(&ids[1_024], &mut || false).unwrap(),
            "entity-0000"
        );
        assert_eq!(
            reverse.representative(&ids[1_024], &mut || false).unwrap(),
            "entity-0000"
        );

        let mut candidate = EntityUnionFind::new(ids.iter(), &mut || false).unwrap();
        let mut checks = 0usize;
        assert_eq!(
            candidate.union_all(&ids, &mut || {
                checks += 1;
                checks > 64
            }),
            Err(ReducerError::Cancelled)
        );
    }

    #[test]
    fn long_resolution_graph_is_iterative_and_cancellable() {
        let graph = synthetic_resolution_graph(20_000);
        assert!(!graph_has_cycle(&graph, &mut || false).unwrap());
        let ordered = topological_nodes_cancellable(&graph, &mut || false).unwrap();
        assert_eq!(ordered.len(), graph.len());
        assert_eq!(ordered.first().map(String::as_str), Some("00019999"));
        assert_eq!(ordered.last().map(String::as_str), Some("00000000"));

        let mut checks = 0usize;
        assert_eq!(
            graph_has_cycle(&graph, &mut || {
                checks += 1;
                checks > 128
            }),
            Err(ReducerError::Cancelled)
        );
        let mut checks = 0usize;
        assert_eq!(
            topological_nodes_cancellable(&graph, &mut || {
                checks += 1;
                checks > 128
            }),
            Err(ReducerError::Cancelled)
        );

        let mut cyclic = graph;
        cyclic
            .get_mut("00019999")
            .expect("last graph node")
            .insert("00000000".into());
        assert!(graph_has_cycle(&cyclic, &mut || false).unwrap());
    }

    #[test]
    fn persisted_late_cross_device_dependency_matches_fresh_full_rebuild() {
        use crate::sqlite::{
            DeviceMaterializationProgress, migrate_knowledge_schema, rebuild_from_verified_history,
        };

        let entity_id = "33333333-3333-4333-8333-333333333333";
        let pending_fact = event_on_device(
            '1',
            1,
            fact(
                entity_id,
                "project.role",
                json!({"kind":"text","value":"owner"}),
                "inferred",
            ),
            "device-a",
        );
        let late_entity = event_on_device('2', 1, declare(entity_id, "Project X"), "device-b");
        let key = ed25519_dalek::SigningKey::from_bytes(&[0x42; 32]).verifying_key();
        let mut db_incremental = cryptex_projection::ProjectionDb::open_in_memory().unwrap();
        migrate_knowledge_schema(db_incremental.connection_mut()).unwrap();

        let before =
            cryptex_events::verify_v2_history(&[&pending_fact], SyntheticTrustedDaemonKey(key))
                .unwrap();
        let before_record = &before.records()[0];
        let fact_position = canonical_position(before_record, 32);
        let before_progress = [
            DeviceMaterializationProgress {
                device_id: "device-a".into(),
                integrity_position: Some(fact_position.clone()),
                safe_position: None,
                blocking_reason: Some("pending_dependency".into()),
            },
            DeviceMaterializationProgress {
                device_id: "device-b".into(),
                integrity_position: None,
                safe_position: None,
                blocking_reason: None,
            },
        ];
        rebuild_from_verified_history(
            db_incremental.connection_mut(),
            BRAIN,
            &before,
            std::slice::from_ref(&fact_position),
            &before_progress,
        )
        .unwrap();
        assert_eq!(
            db_incremental
                .connection()
                .query_row(
                    "SELECT COUNT(*) FROM knowledge_pending_dependencies",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        assert_eq!(
            db_incremental.connection().query_row(
                "SELECT safe_frame_offset FROM knowledge_materialization_state WHERE device_id = 'device-a'",
                [],
                |row| row.get::<_, Option<i64>>(0),
            ).unwrap(),
            None,
            "the unresolved source event must not advance the safe watermark"
        );

        let after = cryptex_events::verify_v2_history(
            &[&pending_fact, &late_entity],
            SyntheticTrustedDaemonKey(key),
        )
        .unwrap();
        let positions = after
            .records()
            .iter()
            .map(|record| canonical_position(record, 32))
            .collect::<Vec<_>>();
        let after_progress = [
            DeviceMaterializationProgress {
                device_id: "device-a".into(),
                integrity_position: Some(positions[0].clone()),
                safe_position: Some(positions[0].clone()),
                blocking_reason: None,
            },
            DeviceMaterializationProgress {
                device_id: "device-b".into(),
                integrity_position: Some(positions[1].clone()),
                safe_position: Some(positions[1].clone()),
                blocking_reason: None,
            },
        ];
        rebuild_from_verified_history(
            db_incremental.connection_mut(),
            BRAIN,
            &after,
            &positions,
            &after_progress,
        )
        .unwrap();

        let mut db_full = cryptex_projection::ProjectionDb::open_in_memory().unwrap();
        migrate_knowledge_schema(db_full.connection_mut()).unwrap();
        rebuild_from_verified_history(
            db_full.connection_mut(),
            BRAIN,
            &after,
            &positions,
            &after_progress,
        )
        .unwrap();
        assert_eq!(
            knowledge_rows(db_incremental.connection()),
            knowledge_rows(db_full.connection()),
            "late dependency replay must converge to the fresh full rebuild"
        );
        assert_eq!(
            db_incremental
                .connection()
                .query_row(
                    "SELECT COUNT(*) FROM knowledge_pending_dependencies",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        assert_eq!(
            db_incremental
                .connection()
                .query_row(
                    "SELECT COUNT(*) FROM knowledge_assertions WHERE predicate = 'project.role'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1,
            "the formerly pending fact is now materialized"
        );
    }

    fn canonical_position(
        record: &cryptex_events::VersionedRecord,
        frame_offset: u64,
    ) -> crate::sqlite::CanonicalFramePosition {
        let (event_id, device_id, content_hash) = match record {
            cryptex_events::VersionedRecord::VerifiedV2Supported(record) => (
                record.event().event_id.clone(),
                record.event().device_id.as_str().to_owned(),
                record.event().content_hash.as_str().to_owned(),
            ),
            _ => panic!("expected supported synthetic v2 record"),
        };
        crate::sqlite::CanonicalFramePosition {
            event_id,
            device_id,
            pack_index: 0,
            frame_offset,
            content_hash,
        }
    }

    type KnowledgeRows = (Vec<(String, String)>, Vec<(String, String)>, i64);

    fn knowledge_rows(connection: &rusqlite::Connection) -> KnowledgeRows {
        let entities = connection
            .prepare("SELECT entity_id, record_json FROM knowledge_entities ORDER BY entity_id")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let assertions = connection
            .prepare("SELECT assertion_key, record_json FROM knowledge_assertions ORDER BY assertion_key")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let provenance = connection
            .query_row("SELECT COUNT(*) FROM knowledge_provenance", [], |row| {
                row.get(0)
            })
            .unwrap();
        (entities, assertions, provenance)
    }

    #[test]
    fn conflicting_event_id_and_wrong_brain_fail_closed() {
        let record = event('1', 1, declare(PERSON_A, "Ada"));
        let conflicting = event_on_device('1', 1, declare(PERSON_A, "Grace"), "device-other");
        let key = ed25519_dalek::SigningKey::from_bytes(&[0x42; 32]).verifying_key();
        let conflict_error = cryptex_events::verify_v2_history(
            &[&record, &conflicting],
            SyntheticTrustedDaemonKey(key),
        )
        .unwrap_err();
        assert_eq!(
            conflict_error.reason,
            cryptex_events::ValidationReasonCode::IntegrityFailure
        );

        let other_brain = event_with_brain(
            '2',
            2,
            declare(PERSON_B, "Grace"),
            "device-b",
            "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
        );
        assert!(matches!(
            reduce_raw(BRAIN, &[record, other_brain]),
            Err(ReducerError::InvalidEvent(_))
        ));
    }

    #[test]
    fn alias_key_uses_unicode_sixteen_full_casefold_output() {
        assert_eq!(normalize_alias("Straße"), "strasse");
        assert_eq!(normalize_alias("\u{1C89}"), "\u{1C8A}");
    }

    #[test]
    fn corrected_assertion_replaces_target_and_missing_target_is_pending() {
        let target = "21111111-1111-4111-8111-111111111111";
        let corrected = event(
            '3',
            3,
            json!({
                "kind":"assertion.correct","target_event_id":target,
                "replacement":{"kind":"fact","subject_entity_id":PERSON_A,"predicate":"favorite.color","value":{"kind":"text","value":"Green"},"confidence":"inferred"},
                "publication_authorization":auth()
            }),
        );
        let missing = event(
            '4',
            4,
            json!({
                "kind":"assertion.correct","target_event_id":"51111111-1111-4111-8111-111111111111",
                "replacement":null,"publication_authorization":auth()
            }),
        );
        let result = reduce_raw(
            BRAIN,
            &[
                event('1', 1, declare(PERSON_A, "Ada")),
                event(
                    '2',
                    2,
                    fact(
                        PERSON_A,
                        "favorite.color",
                        json!({"kind":"text","value":"Blue"}),
                        "inferred",
                    ),
                ),
                corrected,
                missing,
            ],
        )
        .unwrap();
        assert_eq!(result.facts.len(), 1);
        assert_eq!(result.facts[0].value, FactValue::Text("Green".into()));
        assert!(
            result
                .pending
                .iter()
                .any(|p| p.reason == "missing_assertion")
        );
    }

    #[test]
    fn duplicate_corrections_consolidate_and_keep_original_and_correction_provenance() {
        let target = "21111111-1111-4111-8111-111111111111";
        let replacement = json!({
            "kind":"fact","subject_entity_id":PERSON_A,"predicate":"favorite.color",
            "value":{"kind":"text","value":"Green"},"confidence":"inferred"
        });
        let result = reduce_raw(BRAIN, &[
            event('1', 1, declare(PERSON_A, "Ada")),
            event('2', 2, fact(PERSON_A, "favorite.color", json!({"kind":"text","value":"Blue"}), "inferred")),
            event('3', 3, json!({"kind":"assertion.correct","target_event_id":target,"replacement":replacement,"publication_authorization":auth()})),
            event('4', 4, json!({"kind":"assertion.correct","target_event_id":target,"replacement":replacement,"publication_authorization":auth()})),
        ]).unwrap();
        assert_eq!(result.facts.len(), 1);
        assert_eq!(result.facts[0].value, FactValue::Text("Green".into()));
        assert_eq!(result.facts[0].provenance.len(), 3);
        assert_eq!(
            result.facts[0]
                .provenance
                .iter()
                .filter(|p| p.confidence == Some(Confidence::Inferred))
                .count(),
            3
        );
        assert!(result.conflicts.is_empty());
    }

    #[test]
    fn explicit_withdrawal_retains_target_and_decision_history() {
        let result = reduce_raw(BRAIN, &[
            event('1', 1, declare(PERSON_A, "Ada")),
            event('2', 2, fact(PERSON_A, "favorite.color", json!({"kind":"text","value":"Blue"}), "inferred")),
            event('3', 3, json!({
                "kind":"assertion.correct","target_event_id":"21111111-1111-4111-8111-111111111111",
                "replacement":null,"publication_authorization":auth()
            })),
        ]).unwrap();
        assert!(result.facts.is_empty());
        assert_eq!(result.withdrawals.len(), 1);
        assert_eq!(
            result.withdrawals[0].target_event_id,
            "21111111-1111-4111-8111-111111111111"
        );
        assert_eq!(result.withdrawals[0].provenance.len(), 2);
    }

    #[test]
    fn conflicting_corrections_require_complete_explicit_resolution() {
        let target = "21111111-1111-4111-8111-111111111111";
        let records = [
            event('1', 1, declare(PERSON_A, "Ada")),
            event(
                '2',
                2,
                fact(
                    PERSON_A,
                    "favorite.color",
                    json!({"kind":"text","value":"Blue"}),
                    "inferred",
                ),
            ),
            event(
                '3',
                3,
                json!({
                    "kind":"assertion.correct","target_event_id":target,
                    "replacement":{"kind":"fact","subject_entity_id":PERSON_A,"predicate":"favorite.color","value":{"kind":"text","value":"Green"},"confidence":"inferred"},
                    "publication_authorization":auth()
                }),
            ),
            event(
                '4',
                4,
                json!({
                    "kind":"assertion.correct","target_event_id":target,
                    "replacement":{"kind":"fact","subject_entity_id":PERSON_A,"predicate":"favorite.color","value":{"kind":"text","value":"Red"},"confidence":"inferred"},
                    "publication_authorization":auth()
                }),
            ),
        ];
        let unresolved = reduce_raw(BRAIN, &records).unwrap();
        assert!(unresolved.facts.is_empty());
        assert!(
            unresolved
                .conflicts
                .iter()
                .any(|conflict| conflict.kind == "correction")
        );

        let mut resolved_records = records.to_vec();
        resolved_records.push(event('5', 5, json!({
            "kind":"assertion.resolve_conflict","target_event_id":target,
            "decision_event_ids":["31111111-1111-4111-8111-111111111111","41111111-1111-4111-8111-111111111111"],
            "outcome":{"kind":"select","decision_event_id":"31111111-1111-4111-8111-111111111111"},
            "publication_authorization":auth()
        })));
        let resolved = reduce_raw(BRAIN, &resolved_records).unwrap();
        assert_eq!(resolved.facts.len(), 1);
        assert_eq!(resolved.facts[0].value, FactValue::Text("Green".into()));
        assert_eq!(resolved.facts[0].provenance.len(), 4);
        assert!(resolved.conflicts.is_empty());
    }

    #[test]
    fn standalone_resolution_stays_pending_and_is_not_materialized() {
        let resolution = event(
            '5',
            5,
            json!({
                "kind":"assertion.resolve_conflict",
                "target_event_id":"21111111-1111-4111-8111-111111111111",
                "decision_event_ids":[
                    "31111111-1111-4111-8111-111111111111",
                    "41111111-1111-4111-8111-111111111111"
                ],
                "outcome":{"kind":"withdraw"},
                "publication_authorization":auth()
            }),
        );
        let state = reduce_raw(BRAIN, std::slice::from_ref(&resolution)).unwrap();
        assert!(
            !state
                .materialized_event_ids
                .contains(&raw_event_id(&resolution))
        );
        assert_eq!(state.pending.len(), 1);
        assert_eq!(state.pending[0].reason, "missing_assertion");
    }

    #[test]
    fn cyclic_resolution_references_remain_unresolved() {
        let target = "21111111-1111-4111-8111-111111111111";
        let correction_a = "31111111-1111-4111-8111-111111111111";
        let correction_b = "41111111-1111-4111-8111-111111111111";
        let resolution_a = "51111111-1111-4111-8111-111111111111";
        let resolution_b = "61111111-1111-4111-8111-111111111111";
        let records = [
            event('1', 1, declare(PERSON_A, "Ada")),
            event(
                '2',
                2,
                fact(
                    PERSON_A,
                    "favorite.color",
                    json!({"kind":"text","value":"Blue"}),
                    "inferred",
                ),
            ),
            event(
                '3',
                3,
                json!({"kind":"assertion.correct","target_event_id":target,"replacement":{"kind":"fact","subject_entity_id":PERSON_A,"predicate":"favorite.color","value":{"kind":"text","value":"Red"},"confidence":"inferred"},"publication_authorization":auth()}),
            ),
            event(
                '4',
                4,
                json!({"kind":"assertion.correct","target_event_id":target,"replacement":{"kind":"fact","subject_entity_id":PERSON_A,"predicate":"favorite.color","value":{"kind":"text","value":"Green"},"confidence":"inferred"},"publication_authorization":auth()}),
            ),
            event(
                '5',
                5,
                json!({"kind":"assertion.resolve_conflict","target_event_id":target,"decision_event_ids":[correction_a,correction_b,resolution_b],"outcome":{"kind":"select","decision_event_id":correction_a},"publication_authorization":auth()}),
            ),
            event(
                '6',
                6,
                json!({"kind":"assertion.resolve_conflict","target_event_id":target,"decision_event_ids":[correction_a,correction_b,resolution_a],"outcome":{"kind":"select","decision_event_id":correction_b},"publication_authorization":auth()}),
            ),
        ];
        let state = reduce_raw(BRAIN, &records).unwrap();
        assert!(state.facts.is_empty());
        assert!(
            state
                .conflicts
                .iter()
                .any(|conflict| conflict.kind == "decision_cycle")
        );
    }

    #[test]
    fn later_resolution_can_explicitly_resolve_competing_resolution_events() {
        let target = "21111111-1111-4111-8111-111111111111";
        let correction_a = "31111111-1111-4111-8111-111111111111";
        let correction_b = "41111111-1111-4111-8111-111111111111";
        let resolution_a = "51111111-1111-4111-8111-111111111111";
        let resolution_b = "61111111-1111-4111-8111-111111111111";
        let records = [
            event('1', 1, declare(PERSON_A, "Ada")),
            event(
                '2',
                2,
                fact(
                    PERSON_A,
                    "favorite.color",
                    json!({"kind":"text","value":"Blue"}),
                    "inferred",
                ),
            ),
            event(
                '3',
                3,
                json!({"kind":"assertion.correct","target_event_id":target,"replacement":{"kind":"fact","subject_entity_id":PERSON_A,"predicate":"favorite.color","value":{"kind":"text","value":"Red"},"confidence":"inferred"},"publication_authorization":auth()}),
            ),
            event(
                '4',
                4,
                json!({"kind":"assertion.correct","target_event_id":target,"replacement":{"kind":"fact","subject_entity_id":PERSON_A,"predicate":"favorite.color","value":{"kind":"text","value":"Green"},"confidence":"inferred"},"publication_authorization":auth()}),
            ),
            event(
                '5',
                5,
                json!({"kind":"assertion.resolve_conflict","target_event_id":target,"decision_event_ids":[correction_a,correction_b],"outcome":{"kind":"select","decision_event_id":correction_a},"publication_authorization":auth()}),
            ),
            event(
                '6',
                6,
                json!({"kind":"assertion.resolve_conflict","target_event_id":target,"decision_event_ids":[correction_a,correction_b],"outcome":{"kind":"select","decision_event_id":correction_b},"publication_authorization":auth()}),
            ),
            event(
                '7',
                7,
                json!({"kind":"assertion.resolve_conflict","target_event_id":target,"decision_event_ids":[correction_a,correction_b,resolution_a,resolution_b],"outcome":{"kind":"select","decision_event_id":resolution_a},"publication_authorization":auth()}),
            ),
        ];
        let state = reduce_raw(BRAIN, &records).unwrap();
        assert_eq!(state.facts.len(), 1);
        assert_eq!(state.facts[0].value, FactValue::Text("Red".into()));
        assert!(state.conflicts.is_empty());
    }

    #[test]
    fn terminal_resolution_cannot_omit_a_competing_resolution() {
        let target = "21111111-1111-4111-8111-111111111111";
        let correction_a = "31111111-1111-4111-8111-111111111111";
        let correction_b = "41111111-1111-4111-8111-111111111111";
        let resolution_a = "51111111-1111-4111-8111-111111111111";
        let records = [
            event('1', 1, declare(PERSON_A, "Ada")),
            event(
                '2',
                2,
                fact(
                    PERSON_A,
                    "favorite.color",
                    json!({"kind":"text","value":"Blue"}),
                    "inferred",
                ),
            ),
            event(
                '3',
                3,
                json!({
                    "kind":"assertion.correct","target_event_id":target,
                    "replacement":{"kind":"fact","subject_entity_id":PERSON_A,"predicate":"favorite.color","value":{"kind":"text","value":"Red"},"confidence":"inferred"},
                    "publication_authorization":auth()
                }),
            ),
            event(
                '4',
                4,
                json!({
                    "kind":"assertion.correct","target_event_id":target,
                    "replacement":{"kind":"fact","subject_entity_id":PERSON_A,"predicate":"favorite.color","value":{"kind":"text","value":"Green"},"confidence":"inferred"},
                    "publication_authorization":auth()
                }),
            ),
            event(
                '5',
                5,
                json!({
                    "kind":"assertion.resolve_conflict","target_event_id":target,
                    "decision_event_ids":[correction_a,correction_b],
                    "outcome":{"kind":"select","decision_event_id":correction_a},
                    "publication_authorization":auth()
                }),
            ),
            event(
                '6',
                6,
                json!({
                    "kind":"assertion.resolve_conflict","target_event_id":target,
                    "decision_event_ids":[correction_a,correction_b],
                    "outcome":{"kind":"select","decision_event_id":correction_a},
                    "publication_authorization":auth()
                }),
            ),
            event(
                '7',
                7,
                json!({
                    "kind":"assertion.resolve_conflict","target_event_id":target,
                    "decision_event_ids":[correction_a,correction_b,resolution_a],
                    "outcome":{"kind":"select","decision_event_id":correction_a},
                    "publication_authorization":auth()
                }),
            ),
        ];

        let state = reduce_raw(BRAIN, &records).unwrap();

        assert!(state.facts.is_empty());
        assert!(state.pending.iter().any(|pending| {
            pending.reason == "incomplete_decision_set"
                && pending.event_id == "61111111-1111-4111-8111-111111111111"
        }));
        assert!(state.pending.iter().any(|pending| {
            pending.reason == "incomplete_decision_set"
                && pending.event_id == "71111111-1111-4111-8111-111111111111"
        }));
    }
}
