# M3 Phase 0 kickoff — Local Projection + Search

**Status:** Phase 0 planning; product implementation not started.  
**Branch:** `m3/local-projection-search`  
**Base:** `m0/foundation` at M2 merge `817b6a126cf6e9cd3e8a27b1348ec977bdc9dcdd` (PR #8)  
**Implementer:** Cursor for M3 product code, continuing the owner's established Cursor implementation workflow.

## 1. Gate and authority

M2 was completed and merged through PR #8 after the owner-delegated closeout review. Its final Mac host checks and dual-OS CI are recorded in `evidence/m2/m2-final-ci-gate-20261008.md`; review/integration disposition is in `evidence/m2/m2-owner-delegated-review-20261008.md`.

M3 follows the accepted contract at `docs/milestones/M3_LOCAL_PROJECTION_SEARCH.md`, under `docs/00_MASTER_SPEC.md`, `docs/03_MEMORY_MODEL.md`, `docs/04_EVENT_SCHEMA_AND_INVARIANTS.md`, `docs/06_STORAGE_ARCHITECTURE.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/21_DEFINITION_OF_DONE.md`, and accepted ADRs. If implementation would require an architectural decision that conflicts with those authorities, stop and prepare an ADR proposal before product code.

## 2. M3 scope

- Build a disposable, rebuildable local SQLite projection from verified M2 canonical events.
- Provide offline SQLite FTS5 search and metadata filters.
- Verify clean rebuild, missing/corrupt database rebuild, checkpoint/restart, cross-platform behavior, and explicit failure reporting.

M3 does not implement M4 entity/fact/relation consolidation, M8 replication workflows, M13 broad Desktop search UX, semantic/vector search, or any mutation of canonical `.open`/`.seg` data.

## 3. Fixed boundaries

- Canonical event data stays authoritative; SQLite and FTS are disposable derived state.
- `cryptexd` remains the only supported canonical writer. Projection persistence must not become a canonical write path.
- Consume only events that pass M2 validation/integrity. Never build trusted projection state from an invalid or partial sealed pack.
- The current production M2 event is `memory.note`; do not invent M4 semantic fields or imply unsupported event coverage.
- Keep runtime state outside the repository and out of visible home/Desktop/Documents folders by default.
- Search remains local/offline and requires no LLM.

## 4. Phase 0 tasks before implementation

Cursor should inspect the merged M2 reader/storage APIs, workspace dependencies, and test harnesses, then produce a reviewable M3 implementation plan covering:

1. projection ownership and the narrow API boundary between verified canonical reads, projection writes, and search queries;
2. minimal projection fields and FTS content derived from the exact M2 event schema, without adding a new canonical schema;
3. runtime database placement consistent with the accepted application-data roots and rebuildable-state boundary;
4. SQLite driver/build configuration and FTS5 availability on Windows and macOS;
5. transaction, checkpoint, schema-version, corruption-detection, and rebuild semantics;
6. tests and platform verification, with a single final Actions run only if the M3 gate requires it.

Do not add a dependency pin, freeze a projection schema, or change canonical/M2 formats merely to start coding. If one of these choices requires an ADR under repository authority, submit that ADR for owner review before implementing the dependent behavior. Otherwise, keep the design small and within the accepted M3 contract.

## 5. Current state

- M2 merged: PR #8, merge commit `817b6a1`.
- M3 branch starts from that merge commit.
- This kickoff is planning only; no product files, runtime databases, or Actions are changed/created here.
