# M0 — Foundation Gate Checklist

**Status:** ACCEPTED
**Baseline:** 2026-09-28

## Purpose

This checklist is the completion gate for `docs/milestones/M0_FOUNDATION.md`.

A checkbox may be marked complete only after the requirement has actual verified evidence.

Configuration presence, an implementation claim, compilation alone, or success on only one required platform is not sufficient evidence.

## Governing documents

- `AGENTS.md`
- `docs/00_MASTER_SPEC.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/21_DEFINITION_OF_DONE.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/30_ADR_INDEX.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`
- `docs/33_CURSOR_HANDOFF.md`
- `docs/milestones/M0_FOUNDATION.md`

## A. Repository and executable foundation

- [ ] Repository state is clean and required CI checks are green.
  - Evidence:

- [x] Rust workspace builds successfully.
  - Evidence: `evidence/m0/windows-release-build-20261002.md` records the full Windows release workspace build. Cross-platform CI remains a separate gate.

- [ ] Blank Cryptex Desktop launches successfully on macOS.
  - Evidence:

- [ ] Blank Cryptex Desktop launches successfully on Windows.
  - Evidence:

- [ ] `cryptexd` proof-of-concept runs as designed.
  - Evidence:

- [ ] Golden Brain fixture foundation exists and is synthetic-only.
  - Evidence:

- [ ] Doctor foundation exists and reports actual implemented health state rather than configuration-only state.
  - Evidence:

## B. P0-1 — Physical Event Storage Layout

- [ ] One-file-per-event layout was benchmarked at required realistic scales.
  - Evidence:

- [ ] Immutable sealed segment/pack layout was benchmarked.
  - Evidence:

- [ ] Hybrid layout was evaluated only if evidence justified it.
  - Evidence:

- [ ] Required measurements cover append latency, enumeration, initial sync/scan, incremental sync, rebuild performance, disk overhead, crash behavior, purge complexity and corruption blast radius.
  - Evidence:

- [ ] Relevant storage-layout behavior was evaluated on macOS and Windows.
  - Evidence:

- [ ] Selected layout is supported by recorded evidence and recovery/scale considerations.
  - Evidence:

- [ ] ADR-0013 is ACCEPTED with the selected physical storage layout.
  - Evidence:

## C. P0-2 — Background Runtime / Atomic Update Lifecycle

- [ ] Main-window close behavior was verified.
  - Evidence:

- [x] Reboot/login startup behavior was verified.
  - Evidence: `evidence/m0/p0-2/windows-reboot-login-survival-20261002.md` records a real Windows reboot/login, startup launch correlation and the complete 600-second daemon survival observation. `evidence/m0/p0-2/macos-native-supervision-evidence.md` records physical Mac LaunchAgent reboot/login. Both are test-only registrations; production installation remains open.

- [ ] Crash detection/restart behavior was verified.
  - Evidence:

- [ ] Single-daemon-instance behavior was verified.
  - Evidence:

- [ ] Desktop reconnect behavior was verified.
  - Evidence:

- [ ] Compatible Desktop + daemon update behavior was verified.
  - Evidence:

- [ ] Rollback/error path was verified.
  - Evidence:

- [ ] Lifecycle/update behavior does not damage canonical data.
  - Evidence:

- [ ] Required lifecycle behavior was verified on macOS and Windows.
  - Evidence:

- [ ] ADR-0012 is ACCEPTED with the selected supervision/update approach.
  - Evidence:

## D. P0-3 — Local IPC Transport

- [x] macOS local IPC proof works with safe socket-path handling.
  - Evidence: `evidence/m0/p0-3/windows-ipc-spike-20261002.md` records physical Mac socket ownership/mode, a 28-byte path and process-level requests.

- [x] Windows named-pipe proof works.
  - Evidence: `evidence/m0/p0-3/windows-ipc-spike-20261002.md`; release binary revalidation in `evidence/m0/windows-release-build-20261002.md`.

- [x] Current-user-only permissions/ACL behavior is verified.
  - Evidence: `evidence/m0/p0-3/windows-cross-account-acl-20261002.md` records actual authenticated different-account denial on a Windows CI VM plus owner connectivity before/after; physical Mac socket ownership/mode evidence is in `evidence/m0/p0-3/windows-ipc-spike-20261002.md`. Production integration remains open.

- [x] Protocol version negotiation is verified.
  - Evidence: physical Windows and Mac spike runs in `evidence/m0/p0-3/windows-ipc-spike-20261002.md` reject an incompatible version.

- [x] Desktop/CLI/MCP client-role handling is verified as required by the spike.
  - Evidence: physical role-handshake and unknown-role rejection tests in `evidence/m0/p0-3/windows-ipc-spike-20261002.md`. These are spike clients; production application integration remains open.

- [x] Daemon restart/reconnect behavior is verified.
  - Evidence: physical Windows and Mac spike clients reconnect to a restarted server process in `evidence/m0/p0-3/windows-ipc-spike-20261002.md`. Full Desktop reconnect remains open in P0-2.

- [x] Malformed request handling is verified.
  - Evidence: physical malformed-frame tests on both platforms in `evidence/m0/p0-3/windows-ipc-spike-20261002.md`.

- [x] Oversized request handling is verified.
  - Evidence: physical 65,537-byte frame rejection tests on both platforms in `evidence/m0/p0-3/windows-ipc-spike-20261002.md`.

- [ ] ADR-0007 implementation addendum records the exact IPC implementation/library and relevant security details.
  - Evidence:

## E. P0-4 — Managed Syncthing Integration

- [ ] Pinned Syncthing process/sidecar lifecycle is verified on macOS.
  - Evidence:

- [ ] Pinned Syncthing process/sidecar lifecycle is verified on Windows.
  - Evidence:

- [ ] Cryptex uses an isolated Syncthing config/home.
  - Evidence:

- [ ] No collision with user-global Syncthing configuration is verified.
  - Evidence:

- [ ] REST API authentication is verified.
  - Evidence:

- [ ] Folder/device pairing flow is verified.
  - Evidence:

- [ ] Pause/resume/status behavior is verified.
  - Evidence:

- [ ] Conflict/versioning visibility is verified.
  - Evidence:

- [ ] Upgrade-strategy compatibility has evidence.
  - Evidence:

- [ ] Required third-party license/notices handling is documented.
  - Evidence:

- [ ] ADR-0004 implementation addendum records selected packaging/supervision.
  - Evidence:

## F. Cross-platform, security and failure verification

- [ ] Applicable M0 platform-sensitive behavior is verified on both macOS and Windows.
  - Evidence:

- [ ] No device has been made canonical master.
  - Evidence:

- [ ] Runtime data remains outside the source repository.
  - Evidence:

- [ ] Development runtime data uses Cryptex-Dev app-data roots.
  - Evidence:

- [ ] Canonical writers remain within the permitted daemon boundary for implemented M0 behavior.
  - Evidence:

- [ ] Relevant failure paths have tests or reproducible verification.
  - Evidence:

- [ ] Relevant security checks/review are complete.
  - Evidence:

- [ ] No real personal/Cortex data was used for M0 testing.
  - Evidence:

## G. Quality and completion workflow

- [ ] Relevant formatting checks pass.
  - Evidence:

- [ ] Relevant lint checks pass.
  - Evidence:

- [ ] Relevant unit tests pass.
  - Evidence:

- [ ] Relevant integration tests pass.
  - Evidence:

- [ ] Required cross-platform tests pass.
  - Evidence:

- [ ] Required CI checks are green at the M0 completion commit.
  - Evidence:

- [ ] Verifier reviewed M0 acceptance criteria against actual repository/runtime state.
  - Evidence:

- [ ] Security review is complete for security-sensitive M0 work.
  - Evidence:

- [ ] Human diff/review is complete.
  - Evidence:

## H. Architecture and documentation gate

- [ ] All four P0 spikes are resolved with retained evidence.
  - Evidence:

- [ ] ADR-0012 is ACCEPTED.
  - Evidence:

- [ ] ADR-0013 is ACCEPTED.
  - Evidence:

- [ ] ADR-0007 implementation addendum is complete.
  - Evidence:

- [ ] ADR-0004 implementation addendum is complete.
  - Evidence:

- [ ] `docs/30_ADR_INDEX.md` reflects final M0 ADR status.
  - Evidence:

- [ ] Any architecture change discovered through M0 evidence has been recorded through the ADR process.
  - Evidence:

- [ ] Master Spec has been updated if an accepted architecture decision changed.
  - Evidence:

- [ ] No M1-or-later feature was implemented opportunistically.
  - Evidence:

## Final gate

- [ ] Every applicable M0 checklist item above has verified evidence.
- [ ] `docs/milestones/M0_FOUNDATION.md` completion criteria are satisfied.
- [ ] `docs/21_DEFINITION_OF_DONE.md` is satisfied.
- [ ] M0 may be reported COMPLETE.
- [ ] M1 is authorized to begin.

Until all applicable final-gate items are verified, M0 remains IN PROGRESS.
