# 03 — Cryptex Memory Model

**Status:** PROPOSED  
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

Materialized entity/fact/relation state is derived and rebuildable.

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

The exact event schema belongs to `docs/04_EVENT_SCHEMA_AND_INVARIANTS.md` and later M1/M2 implementation work.

## 6. Entity role

An ENTITY represents an identifiable subject in the user's knowledge system.

Examples may include a person, project, course, device, organization or concept.

Entity resolution, aliases and merge/consolidation behavior belong to M4 Knowledge Engine.

This document does not define a final entity-ID format, resolution algorithm or similarity threshold.

## 7. Fact role

A FACT is a scalar property known about an entity.

Facts may have confidence and lifecycle state according to the model defined below.

Materialized active facts are derived state.

Correction of a fact must preserve history rather than editing canonical history in place.

Exact fact schemas and consolidation rules belong to the later knowledge-model specification and M4.

## 8. Relation role

A RELATION represents an entity-to-entity relationship.

Materialized relationship graph state is derived and rebuildable.

The relationship graph must not become a separate canonical source of truth merely because it is convenient for visualization or querying.

Detailed relation types and consolidation behavior belong to M4.

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

The exact rules for promoting, lowering or reconciling confidence belong to M4 and must not be guessed during earlier milestones.

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

The exact provenance fields are not selected by this document.

## 14. Corrections and supersession

Corrections do not rewrite existing canonical events.

New accepted information may supersede older material semantically.

The historical chain remains available for timeline/provenance behavior unless later purge semantics deliberately remove content according to the accepted deletion model.

Detailed correction/consolidation algorithms belong to M4.

## 15. Offline convergence

Trusted desktops may create memory while disconnected.

Canonical immutable events can converge by union after reconnection, followed by deterministic Cryptex validation and knowledge consolidation.

No wall-clock ordering may become security authority.

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

The exact projection schemas belong to later milestones.

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

## 20. Explicitly unresolved

This document does not select:

- exact event JSON/binary schema fields
- exact entity ID format
- exact fact or relation type enums
- entity-resolution algorithm
- consolidation thresholds
- confidence-calculation algorithm
- exact provenance field layout
- semantic merge heuristics

Those details remain governed by their dedicated specifications, milestones and evidence.

## 21. Source basis

This document consolidates existing memory-model requirements from:

- `docs/00_MASTER_SPEC.md`
- `AGENTS.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`

It does not independently redesign the Cryptex memory model.
