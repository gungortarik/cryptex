# Cryptex

Cryptex is a cross-platform, local-first personal memory and knowledge system for macOS and Windows.

Current implementation milestone: **M8 — Device trust and replication (in progress, not complete)**.

## Implementation status — 2026-10-29

Active branch: `codex/m8-device-trust`. This branch contains a Windows-focused
implementation checkpoint, not a release or an accepted M8 completion.

| Area | Implemented in this checkpoint | Still required |
| --- | --- | --- |
| Security events and trust | Strict v3 security-event reading, verified trust history, local trust/cutoff reduction, and cutoff-aware content validation | Independent security review and remaining protocol/compatibility gates |
| Windows owner approval | Journaled bootstrap exchange, Desktop approval UI, connection-bound single-use confirmation, restriction/self-retirement append, and duplicate-safe recovery | Interactive native Desktop verification, nonzero cutoff boundaries, and packaged denial/cancel/retry/error cases |
| Import and recovery | Bounded staging, signature/identity/chain checks, policy gates, committed-marker validation and cleanup, and conservative holds for incomplete publication | Production Syncthing-to-staging-to-publication wiring, safe replay, and remaining durability/crash-boundary proof |
| macOS | Peer-identity feasibility investigation and a continuation prompt | Authenticated peer identity, equivalent owner-operation integration, and native parity verification; this approval route currently fails closed |

Earlier Windows evidence reports **517 daemon library tests passed,
0 failed, 4 ignored**, strict daemon/Desktop release Clippy, formatting,
TypeScript/Vite build, and an unbundled Desktop/sidecar package smoke pass.
The separate binary test executable was blocked by Windows Application Control
(OS error 4551) on that run. These results do not prove an interactive native
owner flow, installer readiness, macOS parity, or end-to-end peer replication.
See the [owner-operation evidence](evidence/m8/m8-windows-owner-security-operation-ipc-append-recovery-20261029.md)
for exact commands and limits.

**Checkpoint verification update:** the fresh pre-publication daemon library
run is **not green**: 515 passed, 3 failed, 4 ignored. An isolated rerun of the
owned-process HTTP group produced 4 passed and 4 failed with
`Endpoint(PeerNotOwned)` rather than the expected owned-server result. The
cause remains unresolved; no ownership check or test assertion was weakened.
The daemon binary tests now pass **12/12**. Workspace release Clippy across
all targets, formatting, frontend build, and Desktop policy tests (**18/18**)
pass. See [checkpoint verification](evidence/m8/m8-checkpoint-publication-20261029.md).

**Next continuation point:** investigate the owned-process HTTP test failures,
then verify the Windows Desktop owner flow using a
fresh synthetic profile outside the repository, including confirmation,
cancellation, denial, retry, exact nonzero cutoff boundaries, and recovery.
Then address production transport/publication, macOS parity, and independent
security review before evaluating the M8 completion gate. ADR-0019's protocol
addendum remains PROPOSED overall; owner acceptance of sections 2–4 as the
Windows implementation basis is not full protocol or milestone acceptance.

The [current handoff](docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md) is the detailed
continuation record. Its dated entries are historical evidence: statements
such as "No commit, push" describe those individual runs, not the publication
status of this checkpoint.

Authoritative project documentation lives under `docs/`. Runtime data must remain outside this source repository.

## Current implementation handoff

Before starting implementation or review work, read [`docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md`](docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md). It records the current milestone, verified evidence, open blockers, and the exact next task. Authoritative specifications and accepted ADRs still take precedence.

## Standalone Desktop build

Use `pnpm --filter @cryptex/desktop tauri:package-smoke` for a standalone probe
or `pnpm --filter @cryptex/desktop tauri:package --bundles app` for the macOS
application bundle. These commands build frontend assets and stage the sidecar.

Rust `--release` alone does not select Tauri's embedded frontend. A release
compiled without `tauri/custom-protocol` now exits with
`RELEASE_FRONTEND_MODE_REJECTED` before Tauri/daemon startup. Direct release
Desktop tests/builds must explicitly enable `--features tauri/custom-protocol`;
they do not replace frontend build or native package preparation. Debug
development through the existing Vite/Tauri commands remains supported.
