# 05 — Cryptex Entity, Fact and Relation Model

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the fixed semantic boundaries for Cryptex entities, facts, relations and knowledge consolidation.

It preserves the distinction between immutable canonical history and derived/materialized knowledge state.

Initial M4 schema and deterministic semantics are defined by accepted
ADR-0017. This document preserves the high-level model and points to that ADR
for the concrete v2 payload, reducer, and derived-store contract.

## 2. Authority and scope

This document must remain consistent with:

1. `docs/00_MASTER_SPEC.md`
2. accepted ADRs relevant to knowledge semantics
3. `docs/03_MEMORY_MODEL.md`
4. `docs/04_EVENT_SCHEMA_AND_INVARIANTS.md`
5. `docs/20_MILESTONE_ROADMAP.md`
6. `docs/21_DEFINITION_OF_DONE.md`
7. `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`

If implementation evidence conflicts with an accepted invariant, implementation stops and the specification/ADR process is followed.

## 3. Semantic concepts

Cryptex uses:

- **ENTITY** — an identifiable subject such as a person, project, course, device, organization or concept
- **FACT** — a scalar property known about an entity
- **RELATION** — an entity-to-entity relationship

These concepts represent knowledge semantics.

Materialized entity/fact/relation state is derived and rebuildable rather than a separate mutable canonical source of truth.

## 4. Canonical history remains authoritative

Canonical immutable events preserve durable history.

The knowledge engine derives current entity/fact/relation state from accepted canonical history.

Therefore:

- current knowledge can change without editing old canonical events
- correction preserves provenance
- superseded knowledge can remain visible in history
- loss of materialized knowledge state must be recoverable from canonical data

## 5. Entity model

An ENTITY represents a subject that Cryptex can refer to consistently over time.

M4 includes:

- entity resolution
- aliases
- semantic consolidation

Initial M4 entity IDs are explicit UUIDs. Independent cross-device IDs remain
separate until an explicit authorized merge; fuzzy-name matching never merges
entities. Alias normalization and ambiguity behavior follow ADR-0017.

## 6. Alias behavior

Aliases allow multiple names/labels to refer to the same resolved entity when accepted by the knowledge model.

Aliases must not erase provenance or silently mutate canonical historical records.

The knowledge engine may materialize alias state from canonical history.

Exact alias normalization and conflict rules follow accepted ADR-0017. An alias
matching multiple entities remains ambiguous and does not merge them.

## 7. Fact model

A FACT is a scalar property known about an entity.

Materialized active facts are derived state.

Facts may carry logical state/confidence according to the accepted memory model.

An accepted correction or withdrawal may change which fact is active while
preserving the history that produced every assertion. Arrival time and
Last-Write-Wins do not select a value.

The initial fact key/value schema and cardinality policy follow accepted
ADR-0017. `person.birth_date` is single-valued per person; all other and
unknown predicates are multi-valued by default. Different values for a
multi-valued predicate coexist; they are not automatically conflicts.
Single-valued conflicts have no winner until explicit authorized
correction/withdrawal history addresses them.

## 8. Relation model

A RELATION represents a relationship between entities.

The materialized relationship graph is derived and rebuildable.

Relationship state must not become canonical merely because graph traversal or visualization uses it.

The initial relation representation follows accepted ADR-0017. Relations are
multi-valued by default; different targets coexist, with no inferred inverse
relations or fuzzy entity merge.

## 9. Confidence

Knowledge confidence values are:

- `confirmed`
- `inferred`
- `uncertain`

Confidence must remain visible as semantic information rather than being discarded by materialization.

For structured M4 assertions, confidence follows this vocabulary and remains
separate from authorization and lifecycle. `cryptexd` validates the explicit
publication request; a signature plus receipt reference does not independently
prove human approval or factual correctness to remote peers. AI proposals stay
in local staging until explicitly authorized for publication.

## 10. Knowledge lifecycle state

The broader memory model defines:

- `ACTIVE`
- `SUPERSEDED`
- `FORGOTTEN`
- `PURGED`

Materialized knowledge must respect these semantics.

In particular:

- superseded information must not appear as current active knowledge merely because it still exists in history
- forgotten content must be excluded from normal retrieval/AI context according to the accepted Forget model
- purged content must obey accepted Purge semantics and stale-peer suppression rules

## 11. Correction and supersession

Correction is additive in canonical history.

An accepted correction creates new durable history that changes the derived interpretation.

It must not rewrite prior canonical events in place.

M4 owns correction/supersede product behavior for knowledge consolidation.

## 12. Deterministic consolidation

Offline-first convergence requires deterministic knowledge consolidation from accepted canonical history.

Two trusted peers that receive the same valid canonical history should be able to converge on the same derived knowledge state under the same accepted rules.

Initial deterministic replay, dependency resolution, correction handling,
cardinality, normalization, and stable tie-break rules follow accepted ADR-0017.
Missing cross-device references remain pending and block contiguous safe
materialized progress until resolved; event arrival order and wall-clock time
cannot decide the result.

Wall-clock time must not become security authority.

## 13. Provenance

Derived facts and relations must retain sufficient provenance to connect current materialized knowledge back to relevant canonical history according to the implemented milestone.

The knowledge engine must not silently collapse history in a way that prevents later explanation/correction behavior.

M4 provenance follows accepted ADR-0017 and is rebuilt from canonical events,
including source event/device/sequence, signing key, confidence,
authorization basis/receipt reference, classification, and contribution role.

## 14. Classification and privacy

Knowledge classification values are:

- `public`
- `internal`
- `private`
- `restricted`

Derived knowledge views must preserve classification semantics required by downstream access control.

Restricted memory is not exposed to AI by default.

A derived projection must not weaken classification merely because it is easier to query.

## 15. Untrusted input and authority tiers

Imported content and AI output are untrusted input.

The audit requires authority tiers/protected namespaces so untrusted content cannot silently become security/global policy.

Therefore entity/fact/relation consolidation must not allow ordinary inferred content to install or replace daemon-authoritative security/system rules.

The detailed authority model belongs to the dedicated privacy/authority specification.

## 16. Search relationship

M3 implements local SQLite/FTS projection and filtering.

M4 adds semantic entity/fact/relation consolidation.

Search/index structures remain derived.

The order matters: M4 must not silently turn the search database into canonical knowledge storage.

M4 knowledge state is additive and rebuildable within M3's existing
`projection.sqlite`; M3 schema v1 remains unchanged. Content retrieval is
conditional on the daemon's current `RetrievalSafetyState`, and a Brain-wide
hold overrides normal search availability.

## 17. Obsidian relationship

Obsidian receives a derived materialized view of Cryptex knowledge.

Generated notes/links may reflect entities, facts and relations, but the vault is not canonical.

Daemon-controlled Obsidian projection generation must honor the same retrieval
safety decision. The daemon cannot revoke plaintext files/copies already
written where the operating system or user can access them.

Manual managed-note edits become proposals and only affect canonical memory after daemon-controlled validation/review according to the Obsidian milestone.

## 18. AI relationship

AI clients consume knowledge only through controlled Cryptex integration boundaries.

They do not directly mutate materialized knowledge databases or canonical event files.

AI-generated assertions are proposals/untrusted input until accepted through the appropriate Cryptex path.

## 19. Rebuild requirement

Materialized knowledge state must be rebuildable from canonical durable history.

Relevant later tests must cover:

- missing derived knowledge state
- corrupt derived knowledge state
- deterministic rebuild
- correction/supersede reconstruction
- confidence/state reconstruction

The full implementation/testing belongs to M4 and later hardening.

## 20. M4 boundary

M4 Knowledge Engine includes:

- entity resolution
- aliases
- facts
- relations
- confidence/state
- correction/supersede
- semantic consolidation

M0–M3 must not opportunistically implement the full knowledge engine.

Earlier milestones may define only the foundations explicitly required by their contracts.

## 21. Remaining versioned decisions

The initial M4 payload, reducer semantics, confidence/authorization
separation, cardinality, provenance, and additive derived schema are selected
by accepted ADR-0017. Later payload versions, changes to reducer/cardinality/
normalization policy, and production v2 writer activation require their
explicit version or activation gate; acceptance here does not enable them.

## 22. Source basis

This document consolidates existing knowledge-model requirements from:

- `docs/00_MASTER_SPEC.md`
- `AGENTS.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`

Initial M4 algorithms and schema details are specified by accepted
`docs/adr/ADR-0017-structured-event-evolution-and-knowledge-materialization.md`;
this document does not independently change that decision.
