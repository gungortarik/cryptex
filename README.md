# Cryptex

Cryptex is a cross-platform, local-first personal memory and knowledge system for macOS and Windows.

Current implementation milestone: **M8 — Device trust and replication (in progress, not complete)**.

## Implementation status — 2026-11-05

Active branch: `codex/m8-device-trust`. This branch contains a Windows-focused
implementation checkpoint, not a release or an accepted M8 completion.

| Area | Implemented in this checkpoint | Still required |
| --- | --- | --- |
| Security events and trust | Strict v3 security-event reading, verified trust history, local trust/cutoff reduction, and cutoff-aware content validation | Independent security review and remaining protocol/compatibility gates |
| Windows owner approval | Journaled bootstrap exchange, Desktop approval UI, connection-bound single-use confirmation, restriction/self-retirement append, and duplicate-safe recovery; native self-retirement review/cancel/retry and unapproved-peer denial passed on the second Windows VM | Native approved-peer restriction review at a nonzero cutoff and native restart/recovery flow; no confirmation on the owner's current device |
| Import and recovery | Bounded staging, signature/identity/chain checks, policy gates, committed-marker validation and cleanup, and conservative holds for incomplete publication | Production Syncthing-to-staging-to-publication wiring, safe replay, and remaining durability/crash-boundary proof |
| macOS | Peer-identity feasibility investigation and a continuation prompt | Authenticated peer identity, equivalent owner-operation integration, and native parity verification; this approval route currently fails closed |

**Latest Windows verification (owner-provided, second Windows/VM):** on
**26200.9711 / 25H2**, pinned source `a5f859a`, Rust 1.98.1, the standard-user
run passed **520 daemon library tests (0 failed, 4 ignored)**. The owner also
reported **12 binary tests passed**; the separate owned HTTP group passed
**8/8**, and the native TCP probe matched **60/60** samples. The four ignored
native fixtures, integration executables, and interactive Desktop flow are
not covered by these results.

The earlier elevated second-host run failed 43 tests at ownership/permission
boundaries; a probe confirmed new directories were owned by Administrators.
The standard-user rerun passed with unchanged security checks and assertions.
Elevated-launch compatibility remains an open limitation. On development build
26300.9818, four HTTP tests still fail alongside independently reproduced
native TCP-state mismatches; that host-specific anomaly is unresolved.
See the [diagnostic and verification evidence](evidence/m8/m8-windows-tcp-state-diagnostic-20261029.md)
for commands, result provenance, earlier failures, and scope limits.

Earlier development-host checks passed workspace release Clippy, format,
TypeScript/Vite build, Desktop policy tests (18/18), and an unbundled
Desktop/sidecar package smoke. The package smoke did not verify an interactive
owner dialog or installer. See the [checkpoint evidence](evidence/m8/m8-checkpoint-publication-20261029.md)
and [owner-operation evidence](evidence/m8/m8-windows-owner-security-operation-ipc-append-recovery-20261029.md).

The second Windows/VM completed the **unbundled release Desktop/daemon package
smoke** with Node 24.19.0, Cargo 1.98.1, and pnpm 11.28.0 at source `b733e43`.
Its initial interactive attempt exposed a renderer crash: after **Prepare
owner review**, the window went blank twice. The Tauri response serialized the
durable snake_case snapshot while React expected camelCase. The fix adds a
camelCase-only Desktop transport DTO and regression test without changing
journal bytes. The owner then rebuilt from a short temporary path and verified
the corrected self-retirement snapshot, then repeated the preparation after
cancellation and received a different transaction ID. Both **Cancel** actions
reported “Operation cancelled. No canonical event was written.” No
confirmation was performed. A valid-format but unapproved peer restriction
request was also denied with “Nothing was changed.” Targeted synthetic daemon
tests passed for an approved peer's exact nonzero cutoff/hash, rejection of a
later boundary, the owner restriction IPC flow, and self-retirement append
recovery. The approved-peer UI and native recovery flows remain unverified.
Details and exact verification scope are in the
[owner-review rendering evidence](evidence/m8/m8-windows-owner-review-rendering-20261103.md).

**Next continuation point (2026-11-05):** first resolve the native responder
pairing transcript failure (`invalid_transcript`) using the existing isolated
Windows pilot profiles. No responder approval, durable trust record, or sync
was created. Follow [`M8_WINDOWS_RESPONDER_PAIRING_HANDOFF.md`](docs/M8_WINDOWS_RESPONDER_PAIRING_HANDOFF.md)
and the latest section in the [current handoff](docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md);
do not repeat copy/paste attempts until the exact message mismatch is known.
After that pairing slice is closed, return to the Windows owner restriction
review at a nonzero cutoff and native restart/recovery flow. Do not test
permanent confirmation on the owner's current device. The two pre-fix review
attempts left owner-pending journal entries in their disposable profile after
`Ctrl+R` discarded the renderer handles; that profile was not reused. Then
address production transport/publication, macOS parity, and independent
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
