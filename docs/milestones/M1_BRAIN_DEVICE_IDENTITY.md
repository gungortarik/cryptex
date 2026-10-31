# M1 — Brain + Device Identity

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## Purpose

M1 establishes the identity and trust foundations required by later canonical events, replication and disaster recovery.

M1 may begin only after the M0 Foundation gate is green and the required P0 decisions/addenda are complete.

## Authority

Follow `AGENTS.md`, `docs/00_MASTER_SPEC.md`, accepted ADRs, `docs/08_SECURITY_DEVICE_TRUST_AND_RECOVERY_ROOT.md`, `docs/15_PLATFORM_MACOS_WINDOWS.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/21_DEFINITION_OF_DONE.md` and this contract.

## Scope

M1 includes:

- immutable Brain identity metadata
- install/device identity
- operating-system secure-storage integration
- sequence-allocation foundation
- pairing data structures
- Recovery Kit data model and public recovery metadata

## Required invariants

- No device becomes canonical master.
- Sensitive device credentials remain in operating-system protected storage.
- Offline recovery bootstrap material is not part of normal device runtime state.
- Wall-clock time is not security authority.
- Sequence foundations must support later device-sequence cutoff semantics.
- Canonical writes remain daemon-controlled.
- macOS and Windows are first-class targets.

## Brain identity lifecycle boundary

- `cryptexd lifecycle hold` and `login-hold` do not create or require a Brain identity. Creation remains an explicit daemon-owned setup operation; M1 provides the durable persistence foundation, not a first-run product surface.
- `identity/brain_identity.json` under the selected Cryptex app-data root is canonical, durable Brain metadata, not disposable runtime state. M1 does not claim that the record is replicated or recoverable on a reinstalled device.
- M2 event records must bind to the existing `brain_id`; M8 replication must transfer this metadata unchanged; M11 restore must preserve it. M1 does not claim those later behaviors.

## Non-goals

M1 does not implement:

- full immutable event-store behavior from M2
- production pairing/replication UI and revoke/retire/compromise workflows from M8
- encrypted backup/restore product behavior from M11
- broad Desktop product UI from M13

## Verification

M1 must provide evidence for the identity, protected-storage, sequence and recovery-model behavior it introduces, including relevant cross-platform, security and failure checks.

Sensitive credentials must not appear in repository files or privacy-unsafe logs.

## Completion gate

M1 is complete only when:

- all M1 scope items are implemented and verified
- relevant tests/CI are green
- security review for identity/trust handling is complete
- Definition of Done is satisfied
- human review is complete

Only then may M2 begin.
