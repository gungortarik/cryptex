# ADR-0012 implementation proposal — Desktop-owned runtime

**Status:** PROPOSED — implementation direction approved by the project owner; M0 acceptance pending required production evidence
**Date:** 2026-10-03
**Scope:** P0-2 background runtime and atomic update lifecycle

## Source-record note

The original ADR-0012 source is absent from this repository. This scoped
proposal records the implementation direction selected from the P0-2 evidence;
it does not reconstruct the missing original ADR or mark the P0-2 gate complete.

## Context

P0-2 compares native per-user daemon supervision, a Desktop-owned supervisor,
and a justified hybrid. Test-only lifecycle, renderer reconnect, window-close,
login and synthetic update proofs exist on Windows and macOS. A Mac release
Desktop run now physically verifies Desktop-owned daemon startup/restart, login
startup, renderer IPC, Stop without restart, registration removal and cleanup.
Physical Mac tray Open/Quit also passed in `7b32222`; Windows release-mode physical verification,
signed packaging, compatible component updates, rollback and canonical-data
preservation also remain open. See the run evidence for the exact scope.

## Proposed decision

Use **Mode B with per-user login startup**:

- Cryptex Desktop is the per-user login-start application and the runtime
  supervisor while it is running.
- Desktop owns the `cryptexd` sidecar process. The daemon remains the only
  canonical writer; Desktop only supervises and communicates over the approved
  local IPC boundary.
- Closing the main window hides Desktop and leaves the supervisor/daemon alive.
  An explicit Quit action stops the daemon cleanly and exits Desktop.
- The Windows and macOS login-start adapters launch Desktop, never a separately
  registered daemon process.
- OS-specific startup registration and native tray/menu behavior stay behind
  platform adapters.

This is an implementation direction, not acceptance of ADR-0012 or authorization
to leave M0. Implementation must preserve the Master Spec and P0-2 contract.

## Alternatives and evidence

- **Native daemon supervision:** macOS LaunchAgent restart evidence passed.
  Windows per-user Run starts at login and a later ten-minute survival proof
  passed, but no-elevation crash restart is not established; `schtasks /Create`
  was denied in the tested user context.
- **Desktop-owned supervision:** process-level crash restart, single instance,
  window lifecycle and renderer reconnect have test-harness evidence on both
  systems. The Mac release Desktop physically owned a synthetic daemon through
  startup and crash/restart; physical tray Open and active-daemon Quit also
  passed. The equivalent Windows release-mode physical flow remains open.
  This is release-executable evidence, not
  production acceptance.
- **Hybrid:** no additional native daemon supervisor is proposed. Per-user OS
  startup launches Desktop; the Desktop supervisor owns the daemon lifecycle.

The detailed comparison is in
`evidence/m0/p0-2/supervision-candidate-comparison-20261003.md`.

## Required implementation and proof before acceptance

1. Bundle a target-matched `cryptexd` sidecar and resolve it without a developer
   environment variable. Release package-smoke builds stage and resolve the
   sidecar on Windows and macOS; signed installer packaging remains open.
2. Start or safely attach to one daemon when Desktop starts; detect unexpected
   child exit, wait for lock release, restart with bounded retry, and reconnect
   clients. Do not restart after an explicit clean Quit. The Mac release run
   verified this lifecycle with a synthetic daemon; Windows physical release
   verification remains open.
3. Add native tray/menu actions for Open and Quit and verify close/hide behavior.
   Both Mac tray actions are physically verified, including Quit exit 0 with
   an active daemon. Windows physical tray verification remains open.
4. Register per-user login startup for Desktop with least privilege and provide
   a reversible user-facing setting or documented control. The Mac setting was
   physically enabled, exercised through real sign-out/sign-in, and removed;
   Windows physical release verification remains open.
5. Prove actual login startup, crash/restart, duplicate rejection and shutdown
   on Windows and macOS using packaged builds. The Mac release-mode run passed
   these checks including tray Open/Quit; the Windows release-mode physical run
   remains open.
6. Prove a compatible Desktop+daemon package update and rollback with canonical
   data unchanged on both platforms.

Until all proofs pass and the evidence comparison is updated, the ADR remains
PROPOSED and M0 remains open.

## Security and recovery constraints

- Runtime data remains in the OS application-support location, outside the Git
  checkout and away from visible home/Desktop/Documents folders by default.
- Desktop receives no canonical-write path or capability; all canonical writes
  continue through `cryptexd` and the approved IPC authorization boundary.
- Update failure must restore the previous compatible package and must not
  mutate or remove canonical events.
- Startup registration is per-user, least-privilege and removable; no elevated
  service or public endpoint is introduced.

## Acceptance state

**Not accepted.** The project owner approved this direction for implementation.
The Mac release-executable lifecycle proof now includes physical tray Open/Quit.
Windows release-mode physical proof, signed package integration,
compatible update/rollback and canonical-data preservation are still required
before ADR-0012 can be marked ACCEPTED.
