# M9 — Attachments

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## Purpose

M9 productizes canonical attachment/blob storage and integrity behavior.

## Authority

Follow `AGENTS.md`, `docs/00_MASTER_SPEC.md`, `docs/06_STORAGE_ARCHITECTURE.md`, `docs/09_FORGET_PURGE_AND_DATA_LIFECYCLE.md`, `docs/13_OBSIDIAN_INTEGRATION.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/21_DEFINITION_OF_DONE.md` and this contract.

## Scope

M9 includes:

- SHA-256 blob store
- deduplication
- reference tracking
- integrity repair
- projection copy/hard-link behavior

## Boundary with M6

M6 proves the **derived projection side** of attachment handling using synthetic attachment fixtures and a stable projection boundary/interface.

M6 must not implement:

- the canonical attachment blob store
- SHA-256 blob-store ownership
- deduplication
- canonical reference tracking
- integrity repair

Those responsibilities begin here in M9.

M9 connects the real canonical attachment/blob subsystem to the projection boundary already proven in M6, using the accepted hard-link/copy fallback evidence rather than redesigning the projection contract.

## Required invariants

- Attachment blobs are canonical durable data.
- Derived attachment projections are not canonical.
- Dedup/reference logic must not delete still-referenced data.
- Cross-platform projection behavior must follow accepted attachment-projection evidence.
- Integrity failures must be explicit rather than silently ignored.
- M9 must preserve the M6 projection boundary rather than making the Obsidian vault canonical.

## Non-goals

M9 does not silently add universal OCR/PDF/DOCX extraction as a V1 requirement.

Full attachment reference-aware purge behavior belongs to M10.

## Verification

M9 must verify relevant dedup/reference behavior, integrity checking/repair, projection behavior and platform-specific copy/hard-link cases.

Verification must demonstrate that the real canonical attachment backend can drive the already-proven M6 projection boundary without making generated projection output canonical.

## Completion gate

M9 is complete only when scope items and relevant tests pass, required CI is green, Definition of Done is satisfied and human review is complete.

Only then may M10 begin.
