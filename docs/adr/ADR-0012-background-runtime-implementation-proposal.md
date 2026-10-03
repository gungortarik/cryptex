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
login and synthetic update proofs exist on Windows and macOS. Production Desktop
ownership, per-user Desktop startup, tray behavior, packaged component updates,
rollback and canonical-data preservation remain unverified.

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
  systems. The production Desktop does not yet own the supervisor.
- **Hybrid:** no additional native daemon supervisor is proposed. Per-user OS
  startup launches Desktop; the Desktop supervisor owns the daemon lifecycle.

The detailed comparison is in
`evidence/m0/p0-2/supervision-candidate-comparison-20261003.md`.

## Required implementation and proof before acceptance

1. Bundle a target-matched `cryptexd` sidecar and resolve it without a developer
   environment variable.
2. Start or safely attach to one daemon when Desktop starts; detect unexpected
   child exit, wait for lock release, restart with bounded retry, and reconnect
   clients. Do not restart after an explicit clean Quit.
3. Add native tray/menu actions for Open and Quit and verify close/hide behavior.
4. Register per-user login startup for Desktop with least privilege and provide
   a reversible user-facing setting or documented control.
5. Prove actual login startup, crash/restart, duplicate rejection and shutdown
   on Windows and macOS using packaged builds.
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
The production implementation and P0-2 evidence listed above are still
required before ADR-0012 can be marked ACCEPTED.
