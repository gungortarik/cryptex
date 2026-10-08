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

mod unicode16_casefold;

pub const CARDINALITY_POLICY_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReducerError {
    InvalidEvent(String),
    ConflictingEventId(String),
    CanonicalizationFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    pub event_id: String,
    pub device_id: String,
    pub device_seq: u64,
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
    cryptex_core::validate::parse_uuid_str(expected_brain_id)
        .map_err(|_| ReducerError::InvalidEvent(expected_brain_id.to_owned()))?;
    let known_event_ids: BTreeSet<_> = input
        .records()
        .iter()
        .filter_map(verified_record_event_id)
        .map(canonical_uuid)
        .collect();
    let mut by_id = BTreeMap::<String, VerifiedV2KnowledgeAssertion>::new();
    for record in input.assertions() {
        let event = record.event();
        event
            .validate()
            .map_err(|_| ReducerError::InvalidEvent(event.event_id.clone()))?;
        if canonical_uuid(&event.brain_id) != canonical_uuid(expected_brain_id) {
            return Err(ReducerError::InvalidEvent(event.event_id.clone()));
        }
        let canonical_id = canonical_uuid(&event.event_id);
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

    let mut records: Vec<_> = by_id.into_values().collect();
    records.sort_by(|left, right| stable_event_cmp(left.event(), right.event()));
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
    let revoked_targets: BTreeSet<_> = revoked_merges
        .iter()
        .map(|(_, target, _)| target.clone())
        .collect();
    let mut active_merges = Vec::new();
    for (merge_id, ids, _provenance) in &merges {
        if revoked_targets.contains(merge_id) {
            continue;
        }
        if ids.iter().any(|id| !entities.contains_key(id)) {
            state.pending.push(PendingDependency {
                event_id: merge_id.clone(),
                target_id: ids
                    .iter()
                    .find(|id| !entities.contains_key(*id))
                    .cloned()
                    .unwrap_or_default(),
                reason: "missing_entity",
            });
            continue;
        }
        union_entities(&mut entities, ids);
        active_merges.push(merge_id.clone());
    }
    for (revoke_event_id, target_merge_id, _) in &revoked_merges {
        if !merges
            .iter()
            .any(|(id, _, _)| canonical_uuid(id) == *target_merge_id)
        {
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
    for (merge_id, ids, merge_provenance) in &merges {
        let status = if state
            .pending
            .iter()
            .any(|pending| pending.event_id == *merge_id)
        {
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
        provenance.extend(
            revoked_merges
                .iter()
                .filter(|(_, target, _)| target == merge_id)
                .map(|(_, _, source)| source.clone()),
        );
        provenance.sort_by(|a, b| a.event_id.cmp(&b.event_id));
        state.entity_merges.push(EntityMergeRecord {
            event_id: merge_id.clone(),
            entity_ids: ids.clone(),
            representative_id,
            status,
            provenance,
        });
    }
    state
        .entity_merges
        .sort_by(|a, b| a.event_id.cmp(&b.event_id));

    let mut alias_classes: Vec<Vec<(String, String, Provenance)>> = Vec::new();
    aliases.sort_by(|a, b| {
        normalize_alias(&a.1)
            .cmp(&normalize_alias(&b.1))
            .then(a.1.cmp(&b.1))
            .then(a.0.cmp(&b.0))
    });
    for alias in aliases {
        let key = normalize_alias(&alias.1);
        if let Some(group) = alias_classes
            .iter_mut()
            .find(|group| normalize_alias(&group[0].1) == key)
        {
            group.push(alias);
        } else {
            alias_classes.push(vec![alias]);
        }
    }
    for group in alias_classes {
        let normalized_key = group
            .iter()
            .map(|(_, spelling, _)| normalize_alias(spelling))
            .min()
            .unwrap_or_default();
        let mut targets = BTreeSet::new();
        let mut provenance = Vec::new();
        let mut spelling = String::new();
        let mut has_unresolved_target = false;
        for (entity_id, alias_spelling, source) in group {
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
        if targets.len() > 1 {
            state.conflicts.push(Conflict {
                kind: "ambiguous_alias",
                key: normalized_key.clone(),
                event_ids: provenance.iter().map(|p| p.event_id.clone()).collect(),
            });
        }
        if targets.len() == 1 && !has_unresolved_target {
            let entity_id = targets.iter().next().cloned().unwrap_or_default();
            provenance.sort_by(|a, b| a.event_id.cmp(&b.event_id));
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

    let mut active = BTreeMap::<String, (SemanticAssertion, Vec<Provenance>)>::new();
    for (target, mut decisions) in corrections {
        decisions.sort_by(|a, b| a.0.cmp(&b.0));
        let Some((base, base_provenance)) = assertions.remove(&target) else {
            let correction_ids: Vec<_> = decisions
                .iter()
                .map(|decision| decision.0.clone())
                .collect();
            if known_event_ids.contains(&target) {
                invalid_event_ids.extend(correction_ids.iter().cloned());
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
        let decision_ids: BTreeSet<_> = decisions.iter().map(|d| d.0.clone()).collect();
        let target_resolutions: Vec<_> = resolutions
            .iter()
            .filter(|resolution| resolution.1 == target)
            .collect();
        handled_resolution_ids.extend(
            target_resolutions
                .iter()
                .map(|resolution| resolution.0.clone()),
        );
        let correction_history: Vec<_> = decisions
            .iter()
            .map(|decision| decision.2.clone())
            .collect();
        let resolution_history: Vec<_> = target_resolutions
            .iter()
            .map(|resolution| resolution.4.clone())
            .collect();
        let selected: Vec<(String, Option<SemanticAssertion>, Provenance)> =
            if !target_resolutions.is_empty() {
                let resolution_ids: BTreeSet<_> = target_resolutions
                    .iter()
                    .map(|resolution| resolution.0.clone())
                    .collect();
                let resolution_edges: BTreeMap<_, _> = target_resolutions
                    .iter()
                    .map(|resolution| {
                        (
                            resolution.0.clone(),
                            resolution
                                .2
                                .iter()
                                .filter(|id| {
                                    target_resolutions
                                        .iter()
                                        .any(|candidate| candidate.0 == **id)
                                })
                                .cloned()
                                .collect::<BTreeSet<_>>(),
                        )
                    })
                    .collect();
                let cyclic_resolutions = cyclic_nodes(&resolution_edges);
                if !cyclic_resolutions.is_empty() {
                    let mut event_ids: Vec<_> = decision_ids.iter().cloned().collect();
                    event_ids.extend(resolution_ids.iter().cloned());
                    event_ids.sort();
                    event_ids.dedup();
                    state.conflicts.push(Conflict {
                        kind: "decision_cycle",
                        key: target.clone(),
                        event_ids,
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
                let terminal_resolutions: BTreeSet<_> = resolution_ids
                    .iter()
                    .filter(|candidate| {
                        !resolution_edges
                            .values()
                            .any(|dependencies| dependencies.contains(*candidate))
                    })
                    .cloned()
                    .collect();
                let mut resolved =
                    BTreeMap::<String, (String, Option<SemanticAssertion>, Provenance)>::new();
                for decision in &decisions {
                    resolved.insert(decision.0.clone(), decision.clone());
                }
                let mut invalid_resolution = false;
                let mut choices =
                    BTreeMap::<String, (String, Option<SemanticAssertion>, Provenance)>::new();
                for resolution_id in topological_nodes(&resolution_edges) {
                    let Some((_, _, supplied_ids, outcome, provenance)) = target_resolutions
                        .iter()
                        .find(|resolution| resolution.0 == resolution_id)
                    else {
                        continue;
                    };
                    let supplied: BTreeSet<_> = supplied_ids.iter().cloned().collect();
                    let mut missing_reference = false;
                    let mut invalid_reference = false;
                    for supplied_id in &supplied {
                        if decisions.iter().any(|decision| decision.0 == *supplied_id)
                            || resolution_ids.contains(supplied_id)
                        {
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
                    let complete_terminal_set: BTreeSet<_> = decision_ids
                        .iter()
                        .cloned()
                        .chain(
                            resolution_ids
                                .iter()
                                .filter(|id| **id != resolution_id)
                                .cloned(),
                        )
                        .collect();
                    if terminal_resolutions.contains(&resolution_id)
                        && supplied != complete_terminal_set
                    {
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
                    let mut event_ids: Vec<_> = decisions
                        .iter()
                        .map(|decision| decision.0.clone())
                        .collect();
                    event_ids.extend(
                        target_resolutions
                            .iter()
                            .map(|resolution| resolution.0.clone()),
                    );
                    event_ids.sort();
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
                choices.into_values().collect()
            } else {
                let distinct: BTreeSet<_> = decisions
                    .iter()
                    .map(|(_, replacement, _)| semantic_fingerprint(replacement.as_ref()))
                    .collect::<Result<_, _>>()?;
                if distinct.len() > 1 {
                    state.conflicts.push(Conflict {
                        kind: "correction",
                        key: target.clone(),
                        event_ids: decisions.iter().map(|d| d.0.clone()).collect(),
                    });
                    continue;
                }
                decisions
            };
        if let Some((correction_id, replacement, _provenance)) = selected.first().cloned() {
            let mut sources: Vec<_> = selected
                .iter()
                .map(|(_, _, source)| source.clone())
                .collect();
            sources.extend(correction_history);
            sources.extend(resolution_history);
            sources.push(base_provenance);
            sources.sort_by(|a, b| a.event_id.cmp(&b.event_id));
            sources.dedup_by(|a, b| a.event_id == b.event_id);
            if let Some(replacement) = replacement {
                active.insert(correction_id, (replacement, sources));
            } else {
                state.withdrawals.push(Withdrawal {
                    target_event_id: target,
                    provenance: sources,
                });
            }
        }
        let _ = base;
    }
    // A resolution with no correction group must never silently count as
    // materialized. Keep its dependency pending until the complete event set
    // contains a resolvable target and competing decisions.
    for (resolution_id, target, decision_ids, _outcome, _source) in &resolutions {
        if handled_resolution_ids.contains(resolution_id) {
            continue;
        }
        let target_is_assertion = records.iter().any(|record| {
            canonical_uuid(&record.event().event_id) == *target
                && matches!(
                    record.event().payload,
                    KnowledgeAssertionPayload::FactAssert { .. }
                        | KnowledgeAssertionPayload::RelationAssert { .. }
                )
        });
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
    active.extend(
        assertions
            .into_iter()
            .map(|(id, (assertion, source))| (id, (assertion, vec![source]))),
    );

    let mut fact_candidates = Vec::<(String, String, FactValue, Vec<Provenance>)>::new();
    let mut relation_candidates = Vec::<(String, String, String, Vec<Provenance>)>::new();
    for (event_id, (assertion, provenance)) in active {
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
        if values.len() > 1 {
            let event_ids = values
                .values()
                .flatten()
                .flat_map(|(_, sources)| sources.iter().map(|p| p.event_id.clone()))
                .collect();
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
                provenance: entries
                    .into_iter()
                    .flat_map(|(_, sources)| sources)
                    .collect(),
            });
        }
    }
    for entries in by_multi.into_values() {
        let (subject_entity_id, predicate, value, _) = entries[0].clone();
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
            provenance: entries
                .into_iter()
                .flat_map(|(_, _, _, sources)| sources)
                .collect(),
        });
    }
    let mut relation_groups =
        BTreeMap::<String, Vec<(String, String, String, Vec<Provenance>)>>::new();
    for (subject, predicate, object, provenance) in relation_candidates {
        let key = format!("{subject}|{predicate}|{object}");
        relation_groups
            .entry(key)
            .or_default()
            .push((subject, predicate, object, provenance));
    }
    state.entities = entities.into_values().collect();
    state.entities.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));
    state.aliases.sort_by(|a, b| {
        a.normalized_key
            .cmp(&b.normalized_key)
            .then(a.entity_id.cmp(&b.entity_id))
            .then(a.spelling.cmp(&b.spelling))
    });
    state.relations = relation_groups
        .into_values()
        .map(|entries| {
            let (subject_entity_id, predicate, object_entity_id, _) = entries[0].clone();
            Ok(Relation {
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
                provenance: entries
                    .into_iter()
                    .flat_map(|(_, _, _, sources)| sources)
                    .collect(),
            })
        })
        .collect::<Result<_, ReducerError>>()?;
    state.facts.sort_by(|a, b| {
        a.subject_entity_id
            .cmp(&b.subject_entity_id)
            .then(a.predicate.cmp(&b.predicate))
            .then(a.semantic_id.cmp(&b.semantic_id))
    });
    state.relations.sort_by(|a, b| {
        a.subject_entity_id
            .cmp(&b.subject_entity_id)
            .then(a.predicate.cmp(&b.predicate))
            .then(a.object_entity_id.cmp(&b.object_entity_id))
    });
    state.conflicts.sort_by(|a, b| {
        a.kind
            .cmp(b.kind)
            .then(a.key.cmp(&b.key))
            .then(a.event_ids.cmp(&b.event_ids))
    });
    state.pending.sort_by(|a, b| {
        a.target_id
            .cmp(&b.target_id)
            .then(a.event_id.cmp(&b.event_id))
            .then(a.reason.cmp(b.reason))
    });
    let unresolved: BTreeSet<_> = state
        .pending
        .iter()
        .map(|pending| pending.event_id.clone())
        .collect();
    state.materialized_event_ids = records
        .iter()
        .map(|record| canonical_uuid(&record.event().event_id))
        .filter(|event_id| !unresolved.contains(event_id) && !invalid_event_ids.contains(event_id))
        .collect();
    state.materialized_event_ids.sort();
    Ok(state)
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

fn cyclic_nodes(graph: &BTreeMap<String, BTreeSet<String>>) -> BTreeSet<String> {
    fn reaches(
        graph: &BTreeMap<String, BTreeSet<String>>,
        current: &str,
        target: &str,
        visited: &mut BTreeSet<String>,
    ) -> bool {
        let Some(next) = graph.get(current) else {
            return false;
        };
        next.iter().any(|node| {
            node == target
                || (visited.insert(node.clone()) && reaches(graph, node, target, visited))
        })
    }

    graph
        .keys()
        .filter(|node| reaches(graph, node, node, &mut BTreeSet::new()))
        .cloned()
        .collect()
}

fn topological_nodes(graph: &BTreeMap<String, BTreeSet<String>>) -> Vec<String> {
    fn visit(
        node: &str,
        graph: &BTreeMap<String, BTreeSet<String>>,
        visited: &mut BTreeSet<String>,
        output: &mut Vec<String>,
    ) {
        if !visited.insert(node.to_owned()) {
            return;
        }
        if let Some(dependencies) = graph.get(node) {
            for dependency in dependencies {
                visit(dependency, graph, visited, output);
            }
        }
        output.push(node.to_owned());
    }

    let mut visited = BTreeSet::new();
    let mut output = Vec::with_capacity(graph.len());
    for node in graph.keys() {
        visit(node, graph, &mut visited, &mut output);
    }
    output
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

fn union_entities(entities: &mut BTreeMap<String, Entity>, ids: &[String]) {
    let classes: BTreeSet<_> = ids
        .iter()
        .filter_map(|id| entities.get(id).map(|e| e.representative_id.clone()))
        .collect();
    let representative = classes.iter().min().cloned().unwrap_or_default();
    for entity in entities.values_mut() {
        if classes.contains(&entity.representative_id) {
            entity.representative_id = representative.clone();
        }
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
