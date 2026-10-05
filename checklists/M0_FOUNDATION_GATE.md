# M0 — Foundation Gate Checklist

**Status:** ACCEPTED
**Baseline:** 2026-09-28

## Purpose

This checklist is the completion gate for `docs/milestones/M0_FOUNDATION.md`.

A checkbox may be marked complete only after the requirement has actual verified evidence.

Configuration presence, an implementation claim, compilation alone, or success on only one required platform is not sufficient evidence.

Latest independent review: `evidence/m0/m0-independent-review-results-20261005.md`,
source `3de9d00`, following the earlier `8437559` implementer audit.
Retained proofs are scoped to M0 synthetic foundation behavior. Candidate CI and
both-OS release development-root correction PASS; native UI review, final
completion-source reconciliation and human/ADR acceptance remain open. Product
distribution and production canonical safety are not inferred.
Current continuation: Mac native renderer **SCOPED PASS** with fresh embedded
Tauri `.app`; initial raw Cargo `7f5ab2b` failure remains preserved. New `65ce7b9`
frontend-mode correction has focused Windows/Mac proofs and independent review.
Fresh-source final CI and human/ADR acceptance remain open. See
`evidence/m0/macos-embedded-frontend-visual-evidence-20261005.md`.

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
  - Evidence: final candidate 5c07411 is green in run 37377520994 (both readiness plus Windows ACL). Result-recording commits are docs only; final closure/PR source and remaining acceptance still require reconciliation. Newer doc HEAD is not falsely labeled tested.

- [x] Rust workspace builds successfully.
  - Evidence: `evidence/m0/windows-release-build-20261002.md` records the full Windows release workspace build. Cross-platform CI remains a separate gate.

- [x] Blank Cryptex Desktop launches successfully on macOS.
  - Evidence: `evidence/m0/p0-2/macos-tauri-owned-supervisor-restart-evidence-20261003.md` and `macos-owned-desktop-session-evidence-20261005.md`; real foundation Tauri process, physical tray and later staged native executable sessions. No product UI acceptance claim.
  - Initial raw release blank-window FAIL is preserved in `evidence/m0/macos-native-foundation-ui-blank-failure-20261005.md`. Fresh `65ce7b9` Tauri `.app` visibly rendered, with owned-session health and cleanup; see `evidence/m0/macos-embedded-frontend-visual-evidence-20261005.md`. Scoped visual check only; no theme/DPI matrix.

- [x] Blank Cryptex Desktop launches successfully on Windows.
  - Evidence: `evidence/m0/p0-2/windows-physical-release-tray-20261003.md`, `windows-console-visual-result-20261003.md` and `windows-owned-desktop-session-evidence-20261005.md`; actual foundation Desktop process/renderer and staged native sessions.

- [x] `cryptexd` proof-of-concept runs as designed.
  - Evidence: both-OS `evidence/m0/p0-2/*-native-interruption-evidence-20261005.md`; actual protocol-1 health, PID/parent/path correspondence, held daemon lock and clean exit/released lock/absent IPC.

- [x] Golden Brain fixture foundation exists and is synthetic-only.
  - Evidence: `evidence/m0/p0-2/windows-native-interruption-evidence-20261005.md` and `macos-native-interruption-evidence-20261005.md`; accepted-codec synthetic sealed packs/manifest preserved. Production event schema is outside this foundation.

- [x] Doctor foundation exists and reports actual implemented health state rather than configuration-only state.
  - Evidence: both-platform native-interruption evidence above records actual PID/IPC/held-lock health. `m0-foundation.mjs` keeps canonical/Syncthing checks `not-implemented`.

## B. P0-1 — Physical Event Storage Layout

- [x] One-file-per-event layout was benchmarked at required realistic scales.
  - Evidence: `evidence/m0/p0-1/macos-*`, `windows-*`; Syncthing 10k/100k files both directions; 500k files macOS→Windows r2 controlled timeout / impracticality (`syncthing-500k-files-macos-to-windows-r2-TIMED-FAILED.md`)

- [x] Immutable sealed segment/pack layout was benchmarked.
  - Evidence: `evidence/m0/p0-1/macos-*`, `windows-*`; Syncthing 10k/100k/500k segments both directions, initial + incremental

- [x] Hybrid layout was evaluated only if evidence justified it.
  - Evidence: not justified; A/B conclusive. ADR-0013 records hybrid as not selected.

- [x] Required measurements cover append latency, enumeration, initial sync/scan, incremental sync, rebuild performance, disk overhead, crash behavior, purge complexity and corruption blast radius.
  - Evidence: local dated runs; `syncthing-initial-sync-protocol.md`; `syncthing-incremental-workload-APPROVED-FOR-MEASUREMENT.md`; `purge-complexity.md`; `corruption-blast-radius.md`

- [x] Relevant storage-layout behavior was evaluated on macOS and Windows.
  - Evidence: both-platform local FS matrix and both-direction Syncthing cells as indexed in `evidence/m0/p0-1/README.md`

- [x] Selected layout is supported by recorded evidence and recovery/scale considerations.
  - Evidence: `docs/adr/ADR-0013-physical-event-storage-layout.md`

- [x] ADR-0013 is ACCEPTED with the selected physical storage layout.
  - Evidence: `docs/adr/ADR-0013-physical-event-storage-layout.md`; `docs/30_ADR_INDEX.md`

## C. P0-2 — Background Runtime / Atomic Update Lifecycle

- [x] Main-window `CloseRequested` hide/restore behavior was verified on both platforms.
  - Evidence: earlier GUI event-path proofs plus `evidence/m0/p0-2/windows-physical-release-tray-20261003.md` and `macos-physical-traffic-light-close-evidence-20261003.md`; literal native close and restore retain the daemon PID. Later Mac owned-supervisor evidence includes physical tray Open/Quit. Earlier Accessibility failures remain historical.

- [x] Reboot/login startup behavior was verified.
  - Evidence: `evidence/m0/p0-2/windows-reboot-login-survival-20261002.md` records a real Windows reboot/login, startup launch correlation and the complete 600-second daemon survival observation. `evidence/m0/p0-2/macos-native-supervision-evidence.md` records physical Mac LaunchAgent reboot/login. Both are test-only registrations; production installation remains open.

- [x] Mode B process-level crash detection/restart behavior was verified on both platforms.
  - Evidence: `evidence/m0/p0-2/windows-desktop-supervision-evidence.md` and `evidence/m0/p0-2/macos-desktop-supervision-evidence.md`. This verifies the comparison supervisor path; it does not select ADR-0012 or prove a production packaged supervisor.

- [x] Single-daemon-instance behavior was verified on both platforms.
  - Evidence: the Windows and macOS Mode B supervision evidence above records duplicate launch rejection while the daemon lock is held.

- [x] Desktop IPC reconnect behavior was verified on both platforms.
  - Evidence: `evidence/m0/p0-2/windows-desktop-ipc-reconnect-20261002.md` and `evidence/m0/p0-2/macos-desktop-ipc-reconnect-20261003.md` cover the shared Desktop Rust client across supervised restart. `windows-tauri-renderer-ipc-reconnect-20261003.md` and `macos-tauri-renderer-ipc-reconnect-20261003.md` additionally verify renderer health/reconnect commands after clean daemon replacement.

- [x] Compatible Desktop + daemon update behavior was verified.
  - Evidence: both-OS `evidence/m0/p0-2/*-changed-native-package-evidence-20261005.md`; complete staged pair selection and actual healthy candidate. Windows both role hashes changed, Mac Desktop only. M0 spike proof, not installed updater or semantic migration acceptance.

- [x] Synthetic rollback/error path was verified on both platforms.
  - Evidence: `evidence/m0/p0-2/windows-update-rollback-evidence-20261002.md` and `evidence/m0/p0-2/macos-update-rollback-evidence.md`. These are test-only staged binaries/markers, not installer/update UX.

- [x] Lifecycle/update behavior does not damage canonical data.
  - Evidence: both-OS changed-package and native-interruption reports preserve accepted-codec synthetic immutable sealed packs/manifest after each scenario. Scoped M0 fixture safety under the revised proposal; production canonical schema/signatures remain outside M0 and unverified.

- [x] Required lifecycle behavior was verified on macOS and Windows.
  - Evidence: both-OS lifecycle/tray/login/reconnect and native pair/recovery proofs indexed in the final audit. Namespace correction now has Windows 21/21 debug + release and Mac 14/14 debug + release evidence; no final installed/signed lifecycle claim.

- [ ] ADR-0012 is ACCEPTED with the selected supervision/update approach.
  - Evidence:

## D. P0-3 — Local IPC Transport

- [x] macOS local IPC proof works with safe socket-path handling.
  - Evidence: `evidence/m0/p0-3/windows-ipc-spike-20261002.md` records physical Mac socket ownership/mode, a 28-byte path and process-level requests.

- [x] Windows named-pipe proof works.
  - Evidence: `evidence/m0/p0-3/windows-ipc-spike-20261002.md`; release binary revalidation in `evidence/m0/windows-release-build-20261002.md`.

- [x] Current-user-only permissions/ACL behavior is verified.
  - Evidence: `evidence/m0/p0-3/windows-cross-account-acl-20261002.md` records actual authenticated different-account denial on a Windows CI VM plus owner connectivity before/after; physical Mac socket ownership/mode evidence is in `evidence/m0/p0-3/windows-ipc-spike-20261002.md`. The production read-only Desktop health command is integrated and exercised on both platforms; additional CLI/MCP production APIs remain absent.

- [x] Protocol version negotiation is verified.
  - Evidence: physical Windows and Mac spike runs in `evidence/m0/p0-3/windows-ipc-spike-20261002.md` reject an incompatible version.

- [x] Desktop/CLI/MCP client-role handling is verified as required by the spike.
  - Evidence: physical role-handshake and unknown-role rejection tests in `evidence/m0/p0-3/windows-ipc-spike-20261002.md`. These are spike clients; production application integration remains open.

- [x] Daemon restart/reconnect behavior is verified.
  - Evidence: physical Windows and Mac spike clients reconnect to a restarted server process in `evidence/m0/p0-3/windows-ipc-spike-20261002.md`; the production Desktop Rust client reconnects to real daemons on both platforms in the P0-2 evidence linked above.

- [x] Malformed request handling is verified.
  - Evidence: physical malformed-frame tests on both platforms in `evidence/m0/p0-3/windows-ipc-spike-20261002.md`.

- [x] Oversized request handling is verified.
  - Evidence: physical 65,537-byte frame rejection tests on both platforms in `evidence/m0/p0-3/windows-ipc-spike-20261002.md`.

- [x] ADR-0007 implementation addendum records the exact IPC implementation/library and relevant security details.
  - Evidence: accepted `docs/adr/ADR-0007-local-ipc-implementation-addendum.md` records `interprocess` 2.4.4, Windows DACL and macOS socket ownership/mode, protocol/capability boundaries, limitations and evidence.

## E. P0-4 — Managed Syncthing Integration

- [x] Pinned Syncthing process/sidecar lifecycle is verified on macOS.
  - Evidence: `evidence/m0/p0-4/macos-managed-syncthing-evidence-20261005.md`; staged 2.1.5 native binary, four clean absent monitors. Installed distribution remains separate.

- [x] Pinned Syncthing process/sidecar lifecycle is verified on Windows.
  - Evidence: `evidence/m0/p0-4/windows-managed-syncthing-evidence-20261005.md` and `notice-byte-portability-failure-and-fix-20261005.md`; staged native role/notice proof and four clean absent monitors.

- [x] Cryptex uses an isolated Syncthing config/home.
  - Evidence: both-platform managed evidence uses two fresh synthetic owned homes under Cryptex-Dev app data; no shipped production controller claim.

- [x] No collision with user-global Syncthing configuration is verified.
  - Evidence: both-platform managed/upgrade reports retain unchanged observed global config digests and isolated loopback profiles. This is scoped to those actual runs.

- [x] REST API authentication is verified.
  - Evidence: both-platform managed reports reject wrong API keys 403/403 and require authenticated readiness. Production secret storage is not claimed.

- [x] Folder/device pairing flow is verified.
  - Evidence: both-platform managed reports prove two local synthetic identities and actual transfer; cross-host device trust remains separate.

- [x] Pause/resume/status behavior is verified.
  - Evidence: both-platform managed reports verify configured pause/resume and actual idle/zero-needed/no-pull-error state.

- [x] Conflict/versioning visibility is verified.
  - Evidence: both-platform managed reports retain native history and both exact divergent conflict contents.

- [x] Upgrade-strategy compatibility has evidence.
  - Evidence: `evidence/m0/p0-4/windows-syncthing-upgrade-evidence-20261005.md` and `macos-syncthing-upgrade-evidence-20261005.md`; actual 2.1.2 -> 2.1.5 transition/restart/transfer and injected rejection/cold restore. No live canonical rollback, native migration-failure or power-loss claim.

- [x] Required third-party license/notices handling is documented.
  - Evidence: exact LICENSE/AUTHORS and upstream source/release links in `third_party/syncthing/2.1.5/README.md`, both-OS staged membership/byte proofs, and explicit distribution obligations in the revised ADR-0004/security review. This satisfies M0 handling documentation; delivered source-access/dependency notices and installer inclusion remain mandatory before distribution, not verified here.

- [x] ADR-0004 implementation addendum records selected packaging/supervision.
  - Evidence: `docs/adr/ADR-0004-managed-syncthing-implementation-addendum.md` records pinned native package/backend REST ownership, alternatives and both-OS evidence. PROPOSED status and human acceptance remain separate H gates.

## F. Cross-platform, security and failure verification

- [x] Applicable M0 platform-sensitive behavior is verified on both macOS and Windows.
  - Evidence: scoped proof matrix in final audit plus both-OS release-probe-root evidence; candidate CI PASS at 5c07411. Completion-source CI reconciliation and human acceptance remain separate.
  - New frontend-mode correction: focused Windows and Mac parity/native render PASS at source `65ce7b9`; raw Mac artifact digests are operator-reported and not independently rehashed from this host. See platform-specific evidence.

- [x] No device has been made canonical master.
  - Evidence: final audit reviews health-only daemon IPC, synthetic benchmark fixtures and equal isolated Syncthing identities. No production brain/identity or master-device behavior implemented.

- [x] Runtime data remains outside the source repository.
  - Evidence: both-OS native-interruption, changed-package and managed/upgrade reports retain fresh proof roots outside Git; source path guards reject in-repository fixture roots. Historical product-namespace runtimes are also outside Git.

- [x] Development runtime data uses Cryptex-Dev app-data roots.
  - Evidence: debug/pilot/transaction/Syncthing use Cryptex-Dev. `ad44e82` corrects ordinary release to Cryptex-Dev/runtime/p0-2; Windows 21/21 debug + release, Mac 14/14 debug + release, both CLI 4/4 PASS. Both-OS release-probe-root evidence records paths; historical product-root runs remain explicit and were not moved/deleted.

- [x] Canonical writers remain within the permitted daemon boundary for implemented M0 behavior.
  - Evidence: health-only IPC and Desktop supervision/snapshots; Golden uses explicit synthetic benchmark codec and transport fixtures are synthetic only. No production canonical write API or direct client event writer introduced; final audit records limits.

- [x] Relevant failure paths have tests or reproducible verification.
  - Evidence: final audit indexes both-OS protocol rejection/native rollback, actual pre-successor updater kill, injected I/O/claim/owner/process race regressions and sidecar negative tests. No machine power-loss or production migration failure claim.

- [x] Relevant security checks/review are complete.
  - Evidence: implementer corrections/regressions and independent security review in `evidence/m0/m0-independent-review-results-20261005.md`. Narrow M0 verdict PASS; production residuals explicitly retained. Human acceptance and completion-source CI reconciliation remain separate.

- [x] No real personal/Cortex data was used for M0 testing.
  - Evidence: P0-1 synthetic benchmark protocol plus both-OS Golden/native/managed Syncthing evidence explicitly restricts input to synthetic fixtures. No Cortex ingestion or personal memory API implemented; retained corpora not regenerated in this audit.

## G. Quality and completion workflow

- [x] Relevant formatting checks pass.
  - Evidence: both-OS native-interruption workspace checks and latest sidecar boundary checks; current documentation diff check. Later behavior correction must rerun relevant checks.

- [x] Relevant lint checks pass.
  - Evidence: both-OS strict workspace Clippy in native-interruption/changed-package evidence and latest strict Desktop Clippy in sidecar review. Final CI remains separate.

- [x] Relevant unit tests pass.
  - Evidence: dated workspace tests plus latest Windows Desktop 20/20, Mac 13/13 sidecar tests; full counts/sources in final audit. Incremental proof is not a latest-source full workspace claim.

- [x] Relevant integration tests pass.
  - Evidence: retained native pair/recovery and managed/upgrade runners plus candidate CI Windows Node 160/160 + 8/8 and Mac 162/162 + 8/8 at exact 5c07411. Completion-source reconciliation remains separate.

- [x] Required cross-platform tests pass.
  - Evidence: existing parity proofs PASS at exact sources; final path correction adds Mac 14/14 debug + release + 4/4 CLI at 7f5ab2b and Windows 21/21 debug + release + 4/4 CLI. Candidate integrated CI PASS; completion-source CI reconciliation remains unchecked.
  - Current `65ce7b9`: Windows 23 debug / 24 release / 4 CLI; Mac 16 debug / 17 release / 4 CLI; reported PASS, along with platform-specific format/lint/package checks. Fresh-source final CI is tracked separately below.

- [ ] Required CI checks are green at the M0 completion commit.
  - Evidence: the latest stable candidate `c7fa011672973101cb2a5519b5883b26c6909f90` passed all required jobs in run `37387323751`; full counts/jobs in `evidence/m0/m0-final-ci-attempt-20261005.md`. Keep this unchecked until a designated M0 completion commit is created and its required checks are verified.

- [x] Verifier reviewed M0 acceptance criteria against actual repository/runtime state.
  - Evidence: separate read-only verifier at 3de9d00 in `evidence/m0/m0-independent-review-results-20261005.md`, independently queried exact CI and recomputed Windows recovery report digest. Mac raw artifacts remain owner-supplied and are recorded separately; human/completion gates remain open.

- [x] Security review is complete for security-sensitive M0 work.
  - Evidence: independent security auditor at 3de9d00, recorded in `evidence/m0/m0-independent-review-results-20261005.md`; no new blocking synthetic M0 defect established. Five production residuals retained; not human approval or production certification.

- [x] Relevant foundation UI quality review is complete.
  - Evidence: `evidence/m0/m0-foundation-ui-review-20261005.md` records Windows native review; `evidence/m0/macos-embedded-frontend-visual-evidence-20261005.md` records Mac SCOPED PASS and hashes reported by the Mac operator. No broad theme/DPI review is claimed.

- [ ] Human diff/review is complete.
  - Evidence:

## H. Architecture and documentation gate

- [ ] All four P0 spikes are resolved with retained evidence.
  - Evidence:

- [ ] ADR-0012 is ACCEPTED.
  - Evidence:

- [x] ADR-0013 is ACCEPTED.
  - Evidence: `docs/adr/ADR-0013-physical-event-storage-layout.md`

- [x] ADR-0007 implementation addendum is complete.
  - Evidence: accepted `docs/adr/ADR-0007-local-ipc-implementation-addendum.md`; narrow health-only implementation/evidence and stated limits, no broader production API acceptance.

- [ ] ADR-0004 implementation addendum is complete.
  - Evidence:

- [ ] `docs/30_ADR_INDEX.md` reflects final M0 ADR status.
  - Evidence:

- [x] Any architecture change discovered through M0 evidence has been recorded through the ADR process.
  - Evidence: accepted ADR-0013/0007 plus explicitly PROPOSED ADR-0012/0004 revisions and owner boundary authorization; no unrecorded acceptance inferred.

- [ ] Master Spec has been updated if an accepted architecture decision changed.
  - Evidence: accepted sealed-pack layout already reflected in Master Spec section 8. No newly accepted architecture change in this audit; recheck upon final ADR acceptance.

- [x] No M1-or-later feature was implemented opportunistically.
  - Evidence: final audit reviews explicit synthetic Golden/Doctor/native/Syncthing scopes; no production identity/event store, replication semantics, secret store or shipped updater claim. Product obligations retained in revised proposals.

## Final gate

- [ ] Every applicable M0 checklist item above has verified evidence.
- [ ] `docs/milestones/M0_FOUNDATION.md` completion criteria are satisfied.
- [ ] `docs/21_DEFINITION_OF_DONE.md` is satisfied.
- [ ] M0 may be reported COMPLETE.
- [ ] M1 is authorized to begin.

Until all applicable final-gate items are verified, M0 remains IN PROGRESS.
