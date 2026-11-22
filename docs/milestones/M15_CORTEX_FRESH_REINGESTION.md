# M15 — Cortex Fresh Re-ingestion

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## Purpose

M15 performs controlled fresh re-ingestion from legacy Cortex into a new Cryptex Brain without importing Cortex runtime architecture.

## Authority

Follow `AGENTS.md`, `docs/00_MASTER_SPEC.md`, `docs/16_CORTEX_FRESH_REINGESTION.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/21_DEFINITION_OF_DONE.md` and this contract.

## Scope

M15 includes:

- read-only inventory
- category extraction/import
- dedup/conflict review
- provenance
- acceptance/cutover

## Required invariants

- Cortex remains a read-only legacy source during transition.
- Legacy runtime/database structure does not become Cryptex architecture authority.
- Imported content enters through supported Cryptex canonical paths.
- Provenance is preserved.
- Dedup/conflict review is explicit.
- Real personal data is introduced only after required backup/recovery confidence exists.

## Verification

### First step: read-only source inventory

Run `cargo run --locked -p cryptex-cortex-inventory -- SOURCE_DIRECTORY` to inspect a selected legacy directory. Its JSON report on stdout is intended for local review and may contain private filenames. Regular files carry a relative path, byte length and a lowercased filename extension when present. Directory entries include empty directories. Extensions are hints for later extraction review. The tool reads filesystem metadata only. Content readability, classification and content identity must be checked during extraction.

Exit status 0 indicates a complete listing. Status 2 signals findings such as inaccessible directories, indirect or special entries, or a name that cannot be represented in UTF-8. A finding uses the containing directory when the item's name is unavailable. An empty path refers to the selected root. The traversal reports a depth limit beyond 64 nested directories. Above 100,000 visited entries it returns a root-level limit finding and discards the partial list. Use a stable source snapshot when comparing inventories across runs.

### Milestone verification

M15 must verify relevant inventory/import failure behavior, provenance, dedup/conflict review, interrupted import/retry behavior and acceptance/cutover against the real target Cryptex model.

## Completion gate

M15 is complete only when scope items and relevant recovery/privacy/failure tests pass, required CI is green, Definition of Done is satisfied and human review is complete.

Only then may M16 begin.
