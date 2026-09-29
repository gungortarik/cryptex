# M8 — Device Trust + Replication

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## Purpose

M8 productizes trusted-device replication on top of the identity, event and managed-Syncthing foundations established earlier.

## Authority

Follow `AGENTS.md`, `docs/00_MASTER_SPEC.md`, accepted ADRs, `docs/07_SYNC_AND_CONFLICT_PROTOCOL.md`, `docs/08_SECURITY_DEVICE_TRUST_AND_RECOVERY_ROOT.md`, `docs/15_PLATFORM_MACOS_WINDOWS.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/21_DEFINITION_OF_DONE.md` and this contract.

## Prerequisites

- M0 managed-Syncthing proof is complete.
- ADR-0004 implementation addendum is complete.
- M1 identity/sequence foundations are complete.
- M2 canonical event validation/integrity foundations are complete.

## Scope

M8 includes:

- pairing UI/flow
- Syncthing transport controller
- incoming validation
- offline convergence
- revoke/retire/compromise behavior
- sequence cutoff
- replication protection status

## Required invariants

- No device is canonical master.
- Syncthing is transport, not semantic authority.
- Pairing transport alone is not Cryptex trust.
- Incoming data must pass Cryptex validation/trust/integrity checks.
- Wall-clock time is not revocation security authority.
- macOS and Windows are first-class replication targets.

## Non-goals

M8 does not implement full Forget/Purge cleanup from M10, backup/restore from M11 or M16 sync-stress hardening beyond tests required for M8 correctness.

## Verification

M8 must verify relevant pairing/trust behavior, incoming validation, offline/reconnect convergence, revoke/retire/compromise handling, sequence cutoff and truthful replication health/protection status.

## Completion gate

M8 is complete only when all scope items and relevant cross-platform/security/failure tests pass, required CI is green, Definition of Done is satisfied and human review is complete.

Only then may M9 begin.
