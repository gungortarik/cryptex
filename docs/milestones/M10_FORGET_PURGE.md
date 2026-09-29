# M10 — Forget + Purge

**Status:** PROPOSED  
**Baseline:** 2026-09-28

## Purpose

M10 productizes reversible Forget and destructive Purge semantics across local state, replication and reachable Cryptex-controlled history.

## Authority

Follow `AGENTS.md`, `docs/00_MASTER_SPEC.md`, `docs/07_SYNC_AND_CONFLICT_PROTOCOL.md`, `docs/09_FORGET_PURGE_AND_DATA_LIFECYCLE.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/21_DEFINITION_OF_DONE.md` and this contract.

## Scope

M10 includes:

- logical forget/restore
- purge confirmation
- purge ledger/hash bridge
- stale-peer suppression
- version/snapshot cleanup
- attachment reference-aware purge

## Required invariants

- Forget is reversible and removes content from normal retrieval/AI context.
- Purge wins semantically over stale replicas.
- Minimal retained integrity/ledger data must not preserve purged content beyond what the accepted architecture permits.
- Physical purge claims remain best-effort across reachable Cryptex-controlled state.
- Inaccessible lost devices/provider-internal history are outside guaranteed deletion.
- Purge/recovery are not autonomous AI tools.

## Verification

M10 must verify relevant stale-peer behavior, interrupted/incomplete cleanup, reachable version/snapshot cleanup, attachment-reference safety, recovery interaction and truthful deletion-status reporting.

## Completion gate

M10 is complete only when scope items and relevant failure/security/recovery tests pass, required CI is green, Definition of Done is satisfied and human review is complete.

Only then may M11 begin.
