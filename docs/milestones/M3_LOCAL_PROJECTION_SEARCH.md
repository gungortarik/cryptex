# M3 — Local Projection + Search

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## Purpose

M3 implements the rebuildable local SQLite/search projection over canonical Cryptex memory.

## Authority

Follow `AGENTS.md`, `docs/00_MASTER_SPEC.md`, `docs/03_MEMORY_MODEL.md`, `docs/04_EVENT_SCHEMA_AND_INVARIANTS.md`, `docs/06_STORAGE_ARCHITECTURE.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/21_DEFINITION_OF_DONE.md` and this contract.

## Scope

M3 includes:

- SQLite rebuildable projection
- SQLite FTS5 search
- metadata filters
- rebuild/checkpoint behavior
- corruption rebuild tests

## Required invariants

- SQLite/search state is derived and disposable.
- Canonical event data remains the source of truth.
- Loss/corruption of the local database must not destroy canonical memory.
- Core search remains useful offline with no LLM.
- A derived database must not become required recovery state.

## Non-goals

M3 does not implement:

- M4 semantic knowledge consolidation
- post-V1 vector/semantic search
- broad Desktop search UX from M13
- replication product behavior from M8

## Verification

M3 must verify relevant:

- clean projection build from canonical data
- rebuild after missing/corrupt derived state
- FTS5 query behavior and metadata filters
- checkpoint/restart behavior
- cross-platform behavior
- failure transparency rather than silent corruption

## Completion gate

M3 is complete only when all scope items are verified, corruption/rebuild tests pass, required CI is green, Definition of Done is satisfied and human review is complete.

Only then may M4 begin.
