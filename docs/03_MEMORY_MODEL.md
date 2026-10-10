# 03 — Cryptex Memory Model

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the logical Cryptex memory model already established by the Master Spec.

It explains the durable concepts and the separation between canonical history and derived knowledge state without inventing schema fields or algorithms that belong to later milestones.

## 2. Authority and scope

This document must remain consistent with:

1. `docs/00_MASTER_SPEC.md`
2. accepted ADRs relevant to memory semantics
3. `docs/02_V1_SCOPE_AND_NON_GOALS.md`
4. `docs/06_STORAGE_ARCHITECTURE.md`
5. `docs/20_MILESTONE_ROADMAP.md`
6. `docs/21_DEFINITION_OF_DONE.md`

If implementation evidence conflicts with the Master Spec, the conflict must be raised rather than silently encoded into the model.

## 3. Core durable concepts

Cryptex uses these logical concepts:

- **EVENT** — something happened or changed
- **ENTITY** — a person, project, course, device, organization, concept or other identifiable subject
- **FACT** — a scalar property known about an entity
- **RELATION** — an entity-to-entity relationship
- **ARTIFACT** — an attachment or external content object

These concepts describe memory semantics.

They do not imply that every concept is stored as a separately mutable canonical record.

## 4. Canonical history versus materialized knowledge

Canonical durable memory is immutable event data plus the other explicitly canonical durable objects defined by the Master Spec.

Materialized entity/fact/relation state is derived and rebuildable. Under
accepted ADR-0017, M4 materializes only explicitly structured, verified v2
assertions; it never promotes arbitrary v1 `memory.note` free text into
entities, facts, or relations.

Therefore:

- events preserve durable history
- derived knowledge represents the currently materialized interpretation of that history
- correcting knowledge must not require editing an existing canonical event in place
- loss of derived knowledge state must not destroy canonical memory

## 5. Event role

An EVENT records that something happened or changed.

Existing canonical events are immutable.

Events are the basis from which later systems can derive:

- timeline/history
- current facts
- relationships
- provenance
- correction/supersede state
- search/index projections

The exact event schema belongs to `docs/04_EVENT_SCHEMA_AND_INVARIANTS.md`.
ADR-0016's v1 contract remains unchanged; accepted ADR-0017 defines the
separately validated initial v2 structured assertion envelope and keeps its
production writer disabled behind a separate activation gate.

## 6. Entity role

An ENTITY represents an identifiable subject in the user's knowledge system.

Examples may include a person, project, course, device, organization or concept.

For the initial M4 contract, entity IDs are explicit UUIDs in ADR-0017
payloads. Independent device declarations remain distinct unless an explicit
authorized merge event equates them; fuzzy name similarity never merges
entities. Alias normalization and ambiguity behavior follow ADR-0017.

## 7. Fact role

A FACT is a scalar property known about an entity.

Facts may have confidence and lifecycle state according to the model defined below.

Materialized active facts are derived state.

Correction of a fact must preserve history rather than editing canonical history in place.

The initial typed fact schema and predicate/cardinality policy follow accepted
ADR-0017. `person.birth_date` is single-valued for a person; all unregistered
or unknown predicates are multi-valued. Distinct values under multi-valued
predicates coexist and are not conflicts solely because they differ.
Single-valued conflicts remain unresolved until explicit authorized
correction/withdrawal history addresses them; timestamps and Last-Write-Wins
never choose a winner.

## 8. Relation role

A RELATION represents an entity-to-entity relationship.

Materialized relationship graph state is derived and rebuildable.

The relationship graph must not become a separate canonical source of truth merely because it is convenient for visualization or querying.

The initial relation representation and default multi-valued cardinality
follow accepted ADR-0017. Distinct targets coexist; replay does not infer
inverse relations or merge entities by similarity.

## 9. Artifact role

An ARTIFACT represents attachment or external content associated with memory.

Attachment blobs are canonical durable data according to the Master Spec.

Attachment storage/integrity behavior is implemented in the milestone that owns artifacts/attachments.

This document does not define universal extraction behavior; the audit explicitly does not assume universal OCR/PDF/DOCX extraction for V1.

## 10. Memory states

The logical memory states are:

- `ACTIVE`
- `SUPERSEDED`
- `FORGOTTEN`
- `PURGED`

These states represent semantic lifecycle, not permission to mutate historical events in place.

### ACTIVE

The memory currently participates in normal retrieval/knowledge behavior according to applicable rules.

### SUPERSEDED

Later accepted information replaces earlier information semantically while preserving historical provenance.

### FORGOTTEN

Content is removed from normal retrieval and AI context but remains reversible.

### PURGED

Content is subject to deliberate removal semantics.

Minimal opaque integrity/ledger data may remain where required to preserve integrity verification and stale-device suppression without retaining purged content.

Detailed Forget/Purge workflows belong to their dedicated specification and M10.

## 11. Confidence

Cryptex confidence values are:

- `confirmed`
- `inferred`
- `uncertain`

Confidence is part of the logical knowledge model.

For structured M4 assertions, confidence is separate from publication
authorization and lifecycle. A trusted daemon validates the authorized
publication request before signing. A device signature or receipt reference
authenticates the daemon's attestation under this trust model; it does not
independently prove human approval or factual correctness to a remote peer.
AI-assisted proposals remain local staging until explicit authorized
publication and cannot be marked `confirmed` merely because they were signed.

## 12. Classification

Cryptex classification values are:

- `public`
- `internal`
- `private`
- `restricted`

Classification is used to enforce privacy boundaries, especially for AI access.

Restricted memory is not exposed to AI by default.

The full authority/privacy policy belongs to its dedicated specification and AI milestone.

## 13. Provenance

Cryptex memory must preserve provenance sufficient to explain where durable knowledge came from according to the implemented milestone.

Canonical immutable history is the basis for provenance.

Derived knowledge must not discard the ability to trace current state back to relevant canonical history.

For M4, provenance fields follow accepted ADR-0017: source event/device/
sequence, signing key, confidence, authorization basis/receipt reference,
classification, and contribution role. Canonical events remain authoritative;
these fields are reproducible in a complete rebuild.

## 14. Corrections and supersession

Corrections do not rewrite existing canonical events.

New accepted information may supersede older material semantically.

The historical chain remains available for timeline/provenance behavior unless later purge semantics deliberately remove content according to the accepted deletion model.

Initial deterministic correction, withdrawal, conflict, and explicit
resolution rules follow accepted ADR-0017. They retain history and never use
wall-clock time or Last-Write-Wins.

## 15. Offline convergence

Trusted desktops may create memory while disconnected.

Canonical immutable events can converge by union after reconnection, followed by deterministic Cryptex validation and knowledge consolidation.

No wall-clock ordering may become security authority. Missing cross-device
references remain pending and cannot be marked applied or permanently skipped;
materialization progress is contiguous safe progress distinct from integrity
verification progress.

Conflict/convergence transport behavior belongs to `docs/07_SYNC_AND_CONFLICT_PROTOCOL.md`.

## 16. Writer boundary

`cryptexd` is the only supported canonical writer on a device.

Desktop, CLI, MCP, Obsidian and AI integrations must not mutate canonical memory files directly.

This boundary applies equally to event creation, correction, Forget/Purge requests and later knowledge-affecting operations that produce canonical changes.

## 17. AI authority boundary

AI output is untrusted input.

AI may propose or request memory operations only through controlled Cryptex boundaries.

AI output must not silently become:

- device trust
- security policy
- global system rules
- unrestricted canonical mutation

Protected/security namespaces remain daemon-authoritative.

## 18. Derived projections

Derived projections may include:

- SQLite state
- FTS search
- active fact materialization
- relationship graph cache
- Obsidian projection
- diagnostics/cache state

All remain rebuildable and disposable.

M4 adds disposable, rebuildable knowledge tables to the existing M3
`projection.sqlite`; M3 schema v1 remains unchanged. All content retrieval is
subject to `RetrievalSafetyState`, with Brain-wide hold taking precedence.
Daemon-controlled Obsidian generation must use the same gate, but previously
written plaintext files/copies cannot be revoked by the daemon.

## 19. Milestone boundaries

The logical model is implemented incrementally:

- M1 establishes Brain/device identity and sequence foundations
- M2 implements immutable event storage and integrity behavior
- M3 implements local projection/search
- M4 implements entity/fact/relation consolidation
- M5 adds capture/timeline behavior
- M9 implements attachment storage/integrity
- M10 implements Forget/Purge workflows

Earlier milestones must not opportunistically implement later semantic layers.

## 20. Remaining versioned decisions

The initial M4 event, reducer, confidence/authorization, cardinality,
provenance, and derived-schema contracts are selected by accepted ADR-0017.
Changing event payload schemas, cardinality/normalization policy, or enabling
production v2 writing requires its explicit version or activation gate.
- semantic merge heuristics

Those details remain governed by their dedicated specifications, milestones and evidence.

## 21. Source basis

This document consolidates existing memory-model requirements from:

- `docs/00_MASTER_SPEC.md`
- `AGENTS.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`

It does not independently redesign the Cryptex memory model. Initial M4
semantics are further specified by accepted
`docs/adr/ADR-0017-structured-event-evolution-and-knowledge-materialization.md`.
