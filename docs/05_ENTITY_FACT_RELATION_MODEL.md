# 05 — Cryptex Entity, Fact and Relation Model

**Status:** PROPOSED  
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the fixed semantic boundaries for Cryptex entities, facts, relations and knowledge consolidation.

It preserves the distinction between immutable canonical history and derived/materialized knowledge state.

It intentionally does not invent the final M4 algorithms, thresholds or schemas.

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

The exact entity-ID format, alias storage representation and resolution algorithm are not selected by this document.

## 6. Alias behavior

Aliases allow multiple names/labels to refer to the same resolved entity when accepted by the knowledge model.

Aliases must not erase provenance or silently mutate canonical historical records.

The knowledge engine may materialize alias state from canonical history.

Exact alias matching, normalization and conflict rules belong to M4 evidence/implementation.

## 7. Fact model

A FACT is a scalar property known about an entity.

Materialized active facts are derived state.

Facts may carry logical state/confidence according to the accepted memory model.

A newer accepted fact may supersede an older fact semantically while preserving the history that produced both.

The exact fact-key/type schema is not selected here.

## 8. Relation model

A RELATION represents a relationship between entities.

The materialized relationship graph is derived and rebuildable.

Relationship state must not become canonical merely because graph traversal or visualization uses it.

The exact relation-type taxonomy, directionality representation and cardinality rules belong to M4.

## 9. Confidence

Knowledge confidence values are:

- `confirmed`
- `inferred`
- `uncertain`

Confidence must remain visible as semantic information rather than being discarded by materialization.

The exact algorithm for determining or changing confidence is intentionally unresolved until the knowledge-engine milestone.

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

The exact algorithm and tie-breaking details are not selected by this document unless specified by a later accepted ADR/specification.

Wall-clock time must not become security authority.

## 13. Provenance

Derived facts and relations must retain sufficient provenance to connect current materialized knowledge back to relevant canonical history according to the implemented milestone.

The knowledge engine must not silently collapse history in a way that prevents later explanation/correction behavior.

Exact provenance-field layout belongs to implementation specifications.

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

## 17. Obsidian relationship

Obsidian receives a derived materialized view of Cryptex knowledge.

Generated notes/links may reflect entities, facts and relations, but the vault is not canonical.

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

## 21. Explicitly unresolved

This document does not select:

- entity-ID format
- alias normalization algorithm
- entity-resolution thresholds
- fact-key/type taxonomy
- relation-type taxonomy
- semantic merge heuristics
- confidence-scoring algorithm
- detailed tie-breaking algorithm
- exact provenance schema
- exact materialized database schema

These details require M4 implementation evidence and/or accepted ADR/specification updates.

## 22. Source basis

This document consolidates existing knowledge-model requirements from:

- `docs/00_MASTER_SPEC.md`
- `AGENTS.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`

It does not independently select unresolved M4 algorithms or schema details.
