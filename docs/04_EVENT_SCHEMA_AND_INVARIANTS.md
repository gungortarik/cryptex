# 04 — Cryptex Event Schema and Invariants

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the event-level invariants that are already fixed by the Cryptex architecture.

It intentionally does not invent exact field names, wire formats or serialization details that are not yet specified.

The exact physical event packaging is resolved by the M0 P0-1 benchmark and ADR-0013 before M2 product implementation.

## 2. Authority and scope

This document must remain consistent with:

1. `docs/00_MASTER_SPEC.md`
2. accepted ADRs relevant to event storage/integrity
3. `docs/03_MEMORY_MODEL.md`
4. `docs/06_STORAGE_ARCHITECTURE.md`
5. `docs/20_MILESTONE_ROADMAP.md`
6. `docs/21_DEFINITION_OF_DONE.md`
7. `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
8. `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`

If implementation evidence requires a different invariant, the ADR/specification process must be used before changing architecture.

## 3. Canonical event rule

Canonical durable memory is immutable event data.

An accepted canonical event is never edited in place.

Corrections, supersession and later semantic changes are represented through additional durable history rather than mutation of prior event bytes.

## 4. Logical model versus physical packaging

The logical event model is frozen.

The physical packaging is not frozen before M0 P0-1.

M0 must benchmark:

- one immutable file per event
- immutable sealed per-device segment/pack files
- hybrid thresholded segments only if evidence justifies them

No event-schema implementation may assume that the final production architecture is one file per event before ADR-0013 is accepted.

## 5. Attribution and integrity

Canonical events must be attributable and tamper-evident.

M2 implements the required hashes, signatures and integrity chain behavior.

Integrity/signature failures must never be silently swallowed.

Invalid or unverifiable canonical data must be handled explicitly according to the applicable milestone's validation/quarantine rules.

## 6. Device and sequence foundation

M1 establishes install/device identity and sequence-allocation foundation.

Event ordering/security behavior must not rely on wall-clock timestamps as security authority.

The audited architecture uses device sequence cutoff semantics for post-revocation acceptance.

Exact sequence-field names and encoding are not selected by this document.

## 7. Time semantics

Clock values may be useful for display, timeline and provenance behavior.

Clock skew must not become security authority.

Security-sensitive ordering such as device revocation acceptance uses the sequence/trust rules defined by the relevant milestone and security specification.

## 8. Schema source of truth

When event/domain schema implementation begins, Rust domain types generate the required JSON Schema and TypeScript bindings according to the accepted architecture.

CI must detect schema/binding drift once that generation system exists.

This document does not predefine the exact generated field layout.

## 9. Schema version behavior

M2 includes schema-version behavior.

Unsupported or incompatible schema versions must not be silently interpreted as valid known events.

The implementation must provide explicit compatible handling, rejection or quarantine according to the accepted M2 contract.

Exact version-number policy and compatibility windows are not selected here unless defined by a later versioning specification.

## 10. Validation before trusted use

Incoming or newly created canonical event data must pass the validation/integrity boundaries required by its milestone before being treated as trusted application state.

Replication transport success alone is not semantic acceptance.

Local clients do not bypass daemon-controlled validation merely because they run on the same machine.

## 11. Single writer invariant

`cryptexd` is the only supported canonical writer on a device.

Desktop, CLI, MCP, Obsidian and AI adapters request writes through daemon-controlled APIs/IPC.

No client may directly create, alter or replace canonical event files as a supported path.

## 12. Atomicity and durability

M2 includes atomic-write behavior for canonical event storage.

A failed/interrupted write must not silently leave corrupted canonical data treated as valid.

The exact filesystem/container mechanics depend on the physical packaging selected by M0 and must be verified against that accepted architecture.

## 13. Quarantine

M2 includes quarantine behavior for invalid/corrupt/incompatible canonical input.

Quarantine must preserve explicit failure visibility rather than silently dropping integrity problems.

The exact quarantine directory/layout and metadata format are not selected by this document.

## 14. Corrections and supersession

Canonical history is append-only/immutable in semantic terms.

A correction produces new durable history that may supersede earlier information semantically.

The earlier event remains part of provenance/history unless later Purge semantics legitimately remove content according to the accepted deletion architecture.

## 15. Forget and Purge interaction

Forget and Purge do not authorize arbitrary in-place editing of canonical event history.

Forget is reversible semantic exclusion from normal retrieval/AI context.

Purge is deliberate content removal and may retain minimal opaque integrity/ledger bridge data where required to preserve verification and stale-device suppression without retaining purged content.

The exact Purge mechanism belongs to M10.

M0 only measures purge complexity as part of physical-layout evaluation.

## 16. Classification and confidence

The memory model defines classification values:

- `public`
- `internal`
- `private`
- `restricted`

and confidence values:

- `confirmed`
- `inferred`
- `uncertain`

How these values are represented in exact event schemas is not selected by this document.

Any representation must preserve the privacy/authority semantics defined by the applicable specifications.

## 17. Untrusted inputs

Imported memory, Markdown, attachment metadata and AI output are untrusted input.

Untrusted input must not silently create security/global policy or trusted system rules merely because it is encoded into an event proposal.

Security/system namespaces remain daemon-authoritative.

## 18. Cross-platform invariants

Canonical event behavior must be portable across macOS and Windows.

The event model must not depend on:

- platform-specific path separators
- case behavior available on only one OS
- platform-specific line endings
- canonical-data symlinks

Canonical text is UTF-8 with LF where text representation applies.

## 19. Replication invariant

Trusted peers may create valid immutable events while offline.

After reconnection, canonical history can converge by immutable event union plus deterministic Cryptex processing.

Syncthing does not define event validity or semantic conflict resolution.

## 20. M0 boundary

M0 does not implement the full M2 event store.

M0 may create only enough synthetic benchmark/proof code to evaluate physical packaging at realistic scale and to support required P0 evidence.

The M0 event-layout result must be recorded in ADR-0013 before M2 uses the selected production layout.

## 21. M2 boundary

M2 owns productization of the immutable event store, including:

- finalized physical packaging from P0
- schemas generated from Rust types
- atomic writes
- hashes/signatures/integrity chain
- quarantine
- schema-version behavior

M2 must follow the active Definition of Done and accepted storage/security specifications.

## 22. Explicitly unresolved

This document does not select:

- exact event field names
- exact serialization format
- exact hash/signature library
- exact event type enum
- exact schema-version encoding
- exact quarantine layout
- final physical event file/segment naming
- final per-device segment thresholds

Those choices require their applicable milestone/ADR/evidence.

## 23. Source basis

This document consolidates existing event invariants from:

- `docs/00_MASTER_SPEC.md`
- `AGENTS.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`

It does not independently invent the event wire/schema format.
