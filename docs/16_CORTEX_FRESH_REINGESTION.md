# 16 — Cryptex Cortex Fresh Re-ingestion

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the accepted transition boundary from legacy Cortex data into Cryptex.

Cortex is not directly migrated as Cryptex runtime architecture.

It is treated as a read-only legacy source for controlled fresh re-ingestion into a new Cryptex Brain.

## 2. No direct runtime migration

Cryptex does not inherit Cortex runtime machinery, canonical layout or implementation assumptions merely because they existed previously.

Legacy structure is evidence/input, not architecture authority for Cryptex.

The new Cryptex Brain follows the Cryptex Master Spec and accepted milestones/ADRs.

## 3. Read-only legacy source

Cortex is treated as read-only during the transition.

Re-ingestion must not require mutating the original Cortex source in place.

The transition process should preserve the ability to inspect original source/provenance while constructing new Cryptex canonical history.

## 4. Fresh re-ingestion model

The target is a fresh Cryptex Brain built through accepted Cryptex ingestion/canonical-write paths.

Legacy data must not be copied blindly into canonical event folders as though existing legacy files were already valid Cryptex canonical events.

Re-ingested content must satisfy the applicable Cryptex validation, provenance, classification and event-model rules.

## 5. M15 scope

M15 Cortex Fresh Re-ingestion includes:

- read-only inventory
- category extraction/import
- dedup/conflict review
- provenance
- acceptance/cutover

This milestone occurs only after the core Cryptex product and recovery architecture are implemented sufficiently to support safe onboarding.

## 6. Inventory

The transition begins with read-only inventory of the legacy source.

Inventory must identify what is available for controlled extraction without treating legacy runtime state as canonical Cryptex state.

The exact inventory format/tooling is an M15 implementation detail.

## 7. Category extraction/import

M15 may extract/import categories of useful legacy content into the new Cryptex model.

Import behavior must use supported Cryptex canonical paths rather than direct canonical-file mutation.

The exact mapping from legacy categories to Cryptex event/knowledge semantics must be defined and reviewed during M15.

## 8. Deduplication and conflict review

Fresh re-ingestion must account for duplicate or conflicting material rather than blindly duplicating it into the new Brain.

M15 includes explicit dedup/conflict review.

The exact matching algorithm and thresholds are not selected by this document.

## 9. Provenance

Imported/re-ingested material must retain useful provenance showing its legacy source according to the accepted M15 contract.

Provenance must support later explanation/review without making the legacy source a continuing runtime dependency.

The exact provenance fields belong to implementation.

## 10. Acceptance and cutover

M15 includes explicit acceptance/cutover behavior.

Cutover must not occur merely because extraction code ran successfully.

The re-ingested Cryptex Brain must pass the relevant validation, quality, recovery and review criteria before being treated as accepted personal memory state.

## 11. No early personal-data development

Early milestones use Golden Brain and synthetic Dev Brain data.

Real Cortex/personal memory must not be used during early milestone development.

This reduces privacy risk and prevents legacy data quirks from silently steering architecture before core behavior is verified.

## 12. Backup/recovery prerequisite

Real personal Brain onboarding occurs only after backup/recovery confidence at the Private Beta / Release Candidate stage.

Cortex re-ingestion must not become the first time recovery assumptions are tested with important personal data.

## 13. Security and privacy

Legacy Cortex content is untrusted input from the perspective of the new Cryptex ingestion boundary.

Re-ingestion must preserve applicable:

- classification policy
- provenance
- daemon-controlled canonical writes
- least privilege
- secret/recovery boundaries

Legacy files must not gain special authority because they originated from a prior personal system.

## 14. AI use

M15 may use whatever extraction/review assistance is later explicitly accepted, but AI must not become required for durable ingestion correctness unless the specification changes.

AI output remains untrusted and subject to Cryptex validation/review rules.

Restricted memory remains protected according to Cryptex privacy policy.

## 15. Non-goals

The Cortex transition does not mean:

- preserving the old runtime architecture
- preserving legacy directory layout as canonical Cryptex layout
- treating legacy databases as canonical Cryptex databases
- importing every legacy artifact automatically
- bypassing dedup/conflict review
- bypassing provenance
- using real personal data to drive M0–M14 architecture decisions

## 16. Failure behavior

M15 must handle applicable cases such as:

- unreadable legacy items
- duplicate content
- ambiguous category mapping
- conflicting facts
- incomplete provenance
- interrupted import
- rejected/invalid target writes

Failures must be reviewable rather than silently discarded.

## 17. Reversibility before cutover

Before final acceptance/cutover, the transition process should preserve a clear separation between legacy source and new Cryptex Brain so failed/rejected import attempts do not require destructive modification of the source.

The exact staging mechanism belongs to M15.

## 18. Milestone discipline

M0–M14 must not opportunistically implement Cortex migration/re-ingestion product behavior beyond synthetic fixtures or explicit test scaffolding required by their own milestones.

M15 is the dedicated transition milestone.

## 19. Explicitly unresolved

This document does not select:

- legacy inventory file format
- category mapping rules
- dedup algorithm/thresholds
- conflict-review UI
- exact provenance field layout
- exact staging/import tool implementation
- exact cutover checklist beyond applicable milestone/Definition of Done requirements

These are M15 implementation decisions.

## 20. Source basis

This document consolidates existing Cortex-transition requirements from:

- `docs/00_MASTER_SPEC.md`
- `docs/18_IMPLEMENTATION_READINESS_PLAN.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`

It does not convert Cortex legacy architecture into Cryptex architecture.
