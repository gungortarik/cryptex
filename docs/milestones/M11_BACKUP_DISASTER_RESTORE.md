# M11 — Backup + Disaster Restore

**Status:** PROPOSED  
**Baseline:** 2026-09-28

## Purpose

M11 productizes encrypted filesystem-target backup, verification and disaster restore.

## Authority

Follow `AGENTS.md`, `docs/00_MASTER_SPEC.md`, `docs/08_SECURITY_DEVICE_TRUST_AND_RECOVERY_ROOT.md`, `docs/10_BACKUP_RESTORE.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/21_DEFINITION_OF_DONE.md` and this contract.

## Prerequisite spike

Before backup productization, the required P1 backup encryption/container spike must select an audited high-level format/library and test large objects, interrupted backup, restore and key recovery.

## Scope

M11 includes:

- filesystem backup target
- encryption
- designated scheduled-backup writer
- retention
- verification
- sandbox restore rehearsal
- Recovery Kit first-device bootstrap
- manual export bundle

## Required invariants

- Backup does not become canonical memory.
- Successful upload/write is not verification.
- One device performs scheduled retention in V1, but is not canonical master.
- Recovery must restore both content and trust/bootstrap capability.
- No custom cryptography.

## Verification

M11 must verify relevant interrupted backup, damaged backup, large-object handling, sandbox restore, recovery bootstrap, retention behavior and derived-state rebuild after restore.

## Completion gate

M11 is complete only when the prerequisite spike is resolved, scope items and relevant recovery/security/failure tests pass, required CI is green, Definition of Done is satisfied and human review is complete.

Only then may M12 begin.
