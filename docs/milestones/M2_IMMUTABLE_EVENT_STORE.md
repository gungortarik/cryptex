# M2 — Immutable Event Store

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## Purpose

M2 productizes the canonical immutable event store using the physical storage architecture selected by M0 P0-1 and accepted in ADR-0013.

## Authority

Follow `AGENTS.md`, `docs/00_MASTER_SPEC.md`, accepted ADRs, `docs/04_EVENT_SCHEMA_AND_INVARIANTS.md`, `docs/06_STORAGE_ARCHITECTURE.md`, `docs/08_SECURITY_DEVICE_TRUST_AND_RECOVERY_ROOT.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/21_DEFINITION_OF_DONE.md` and this contract.

## Prerequisites

- M0 gate is green.
- ADR-0013 is ACCEPTED.
- M1 identity/sequence foundations are complete where required by event attribution/integrity.

## Scope

M2 includes:

- finalized physical packaging from M0
- schemas generated from Rust domain types
- atomic canonical writes
- hashes/signatures/integrity-chain behavior
- quarantine for invalid/corrupt/incompatible canonical input
- schema-version behavior

## Required invariants

- Existing canonical events are never edited in place.
- `cryptexd` is the only supported canonical writer.
- Corruption, integrity failures and schema incompatibility are never silently swallowed.
- Replication transport success is not semantic acceptance.
- Canonical behavior is portable across macOS and Windows.

## Non-goals

M2 does not implement:

- SQLite/search projection from M3
- knowledge consolidation from M4
- full replication product behavior from M8
- Forget/Purge product workflows from M10

## Verification

M2 must verify relevant:

- atomic/failure write behavior
- integrity validation
- incompatible schema handling
- quarantine behavior
- cross-platform storage behavior
- rebuild inputs required by later derived systems

## Completion gate

M2 is complete only when all scope items and applicable failure/security/cross-platform tests pass, required CI is green, Definition of Done is satisfied and human review is complete.

Only then may M3 begin.
