# M9 — Attachments

**Status:** PROPOSED  
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

## Required invariants

- Attachment blobs are canonical durable data.
- Derived attachment projections are not canonical.
- Dedup/reference logic must not delete still-referenced data.
- Cross-platform projection behavior must follow accepted attachment-projection evidence.
- Integrity failures must be explicit rather than silently ignored.

## Non-goals

M9 does not silently add universal OCR/PDF/DOCX extraction as a V1 requirement.

Full attachment reference-aware purge behavior belongs to M10.

## Verification

M9 must verify relevant dedup/reference behavior, integrity checking/repair, projection behavior and platform-specific copy/hard-link cases.

## Completion gate

M9 is complete only when scope items and relevant tests pass, required CI is green, Definition of Done is satisfied and human review is complete.

Only then may M10 begin.
