# M0 P0-2 — Background Runtime / Atomic Update Lifecycle Evidence

**Status:** IN PROGRESS  
**ADR-0012:** UNRESOLVED (do not accept yet)

This directory records reproducible evidence for M0 P0-2 only. It does not
select or freeze the production supervision/update architecture.

The current cross-platform evidence comparison is in
`supervision-candidate-comparison-20261003.md`. It is not an ADR decision.

## Authority

Approved scoped pilot: `native-package-handover-plan-20261005.md`.
First slice: `windows-native-package-tree-evidence-20261005.md`.
Runnable Mac parity plan: `macos-native-package-tree-test-plan-20261005.md`.
Mac tree parity result: `macos-native-package-tree-evidence-20261005.md` (79/79).
Native gate primitive: `windows-native-handover-gate-evidence-20261005.md`
(Rust 14/14, Node 86/86); parity request:
`macos-native-handover-gate-test-plan-20261005.md`.

- `docs/11_DAEMON_IPC_RUNTIME.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/26_VERSIONING_MIGRATIONS_AND_UPDATES.md`
- `docs/milestones/M0_FOUNDATION.md`
- `checklists/M0_FOUNDATION_GATE.md`

If evidence conflicts with an accepted architectural decision, stop and use the
ADR/specification process.

## Options under comparison (final decision still OPEN)

Current Mac Desktop-owned run: `macos-tauri-owned-supervisor-restart-evidence-20261003.md`
(local checks, release crash/restart, physical login, IPC, Stop and cleanup passed; physical tray Open and active-daemon Quit exit 0 also verified at `dfbaded`).

The project owner approved Mode B with per-user login startup as the
implementation direction. The cross-platform comparison and ADR-0012 acceptance
remain open; this approval does not close the P0-2 gate.

1. native per-user background registration (macOS LaunchAgent + Windows per-user startup)
2. Tauri/tray supervisor spawning `cryptexd` sidecar
3. hybrid only if evidence justifies it

The implementation direction is selected for the current slice; no final
architecture decision has been recorded or accepted in ADR-0012.

## Safety and isolation rules

- Use only synthetic/runtime test state.
- Keep all runtime data outside the source repository. Comparison-harness
  experiments use `Cryptex-Dev/p0-2`; the release Desktop lifecycle test uses
  the isolated app-data runtime path in its test plan and only synthetic state.
- Do not touch P0-1 Syncthing paths (`Cryptex-Dev/syncthing-p0-1*`).
- Create per-user test startup entries only through the reversible procedure in
  the active test plan, and verify their removal during cleanup. Do not treat a
  test registration as approval to ship production startup integration.
- Do not mark P0-2 complete and do not accept ADR-0012 from foundation-slice work alone.
- Test-only native login/reboot startup proofs are verified on both platforms:
  `windows-reboot-login-survival-20261002.md` and
  `macos-native-supervision-evidence.md`. The separate Mac release Desktop run
  also has partial physical evidence for Desktop login startup, supervision,
  renderer IPC and Stop behavior; physical tray Open/Quit passed in the focused
  diagnosis at `dfbaded`. Windows
  release-mode physical lifecycle proof, signed packaging and production
  update/canonical-data gates remain open. Renderer reconnect during external
  Mode B crash/restart is verified on both platforms.
- Windows' second physical reboot/login and ten-minute survival repeat is
  recorded in `windows-reboot-login-survival-repeat-20261003.md`. It explains
  the visible console as the direct Mode A test-daemon launch and records exact
  cleanup of both temporary Run registrations; no matching test process remains.
- Windows release Desktop-owned supervision is partially verified in
  `windows-release-desktop-owned-supervisor-20261003.md`: single instance,
  child restart and automated close/restore passed. Literal tray/login-start
  checks remain open; Code Integrity currently blocks a fresh local package
  build.
  Historical notes that deferred reboot "while P0-1 active" are stale.

## Current foundation slice

Implemented and test-backed in `services/cryptexd`:

- single-instance lock probe
- structured lifecycle status output
- crash/restart harness commands
- protocol-compat + synthetic canonical-marker safety fixtures
- test-only binary stage/activate/rollback harness (`lifecycle update-proof`)
- platform registration adapters left as `not_configured`

Comparison harness added:

- `tools/m0-lifecycle-compare` Mode A/B runners
- `run-update-rollback` synthetic update/rollback runner
- measurement contract + KV capture
- reversible macOS LaunchAgent test harness (Mode A)
- DesktopSupervisor process harness (Mode B)
- Windows Mode B AUTOMATED physical foundation evidence (`windows-desktop-supervision-evidence.md`)
- temporary Desktop hide-on-close developer hooks
- interactive Mode B GUI CloseRequested proof (`macos-mode-b-gui-window-close.md`)

See:

- `windows-package-native-recovery-evidence-20261004.md` (Rust 13/13 and Node 61/61 functional PASS; native stale-claim cleanup; formatting blocked locally and pending on Mac)
- `macos-package-native-recovery-test-plan-20261004.md` (focused native helper build/test and formatting request; no GUI or Actions)
- `macos-package-native-recovery-evidence-20261004.md` (Rust 13/13 and Node 61/61 scoped PASS at 7011fc7; formatter-only repair and Mac native cleanup parity)

- `windows-package-recovery-claim-evidence-20261004.md` (55/55 PASS; exclusive recovery claim, concurrent/late updater refusal and interrupted-claim retention)
- `macos-package-recovery-claim-test-plan-20261004.md` (focused claim parity request; no GUI or Actions)
- `macos-package-recovery-claim-evidence-20261004.md` (reported Mac PASS at `0ce9c93`: 55/55, three claim cases and clean fixture/child cleanup; automatic interrupted-recovery repair remains open)

- `windows-package-live-owner-evidence-20261004.md` (52/52 PASS; live-owner recovery refusal, post-exit rollback, journal ownership checks)
- `macos-package-live-owner-test-plan-20261004.md` (schema-2 owner guard parity request; no GUI or Actions)
- `macos-package-live-owner-evidence-20261004.md` (reported Mac PASS at `b05be5e`: 52/52, conservative owner guard and clean fixture/child cleanup)

- `windows-package-process-race-evidence-20261004.md` (45/45 tests PASS; four actual separate-process transaction races, controlled IPC barriers)
- `macos-package-process-race-test-plan-20261004.md` (focused parity request for process races; no GUI or Actions)
- `macos-package-process-race-evidence-20261004.md` (reported Mac PASS at `6b8776c`: 45/45 tests; four separate-process races and clean child/fixture cleanup)

- `windows-package-recovery-io-evidence-20261003.md` (37/37 transaction + 4/4 health adapter tests PASS; six injected recovery faults, retry and explicit failure diagnostics)
- `macos-package-recovery-io-test-plan-20261003.md` (focused parity request; no GUI, Rust rebuild or Actions)
- `macos-package-recovery-io-evidence-20261004.md` (reported Mac PASS at `9d9159c`: 41/41 tests, six injected fault cases and retry checks; physical disk/ACL/durability gates remain open)

- `windows-package-real-health-20261003.md` (3 actual daemon/shared Desktop IPC scenarios PASS, Rust 10/10 and Node 4/4; installed Desktop updater integration remains open)
- `macos-package-real-health-test-plan-20261003.md` (executed; no repeated filesystem/GUI suites or Actions)
- `macos-package-real-health-evidence-20261003.md` (3/3 real daemon IPC health/recovery scenarios PASS using identical retained Mac release pair; full report retained outside Git; installed updater/canonical-data gates remain open)
- `windows-package-concurrent-transition-evidence-20261003.md` (31/31 filesystem candidate tests PASS; two deterministic stale/concurrent-selection races fail closed; Mac parity requested, no native process lock or production integration claimed)
- `macos-package-concurrent-transition-test-plan-20261003.md` (focused cross-platform parity request for the two new transaction race regressions; no GUI or workflow)
- `macos-package-concurrent-transition-evidence-20261003.md` (reported Mac PASS at `ad37989`: 31/31 tests, two controlled races and five SIGKILL boundaries; native locking and recovery-I/O gates remain open)

- `windows-package-fixture-path-fix-20261003.md` (29/29 Windows tests PASS; real temporary-root allocation and early cleanup; Mac default TMPDIR repeat pending, engine guards unchanged)

- `windows-native-package-replacement-plan-20261003.md` (local unsigned baseline/candidate installer builds PASS; ordinary-session install/replace/operator rollback pending, automatic update safety open)
- `windows-native-package-baseline-20261003.md` (installed 0.0.0 identity/status/IPC and clean exit PASS; NSIS bundle marker hash difference explained; candidate replacement/rollback pending)
- `windows-native-package-update-20261003.md` (real 0.0.0 to 0.0.1 installed replacement, payload/status/IPC and clean exit PASS; operator rollback pending, atomic update acceptance open)
- `windows-native-package-rollback-20261003.md` (real 0.0.1 to 0.0.0 operator rollback PASS; original payload restored, status/IPC and clean exit verified; automatic rollback and canonical-data safety open)
- `windows-package-transaction-candidate-20261003.md` (28 Windows filesystem candidate tests and 8 retained-binary-copy cases PASS; actual owned-worker kills, fixture preservation; installed-app integration and production acceptance open)
- `macos-package-transaction-candidate-test-plan-20261003.md` (follow-up complete at `fc31489`; no GUI, lifecycle repeats or Actions)
- `macos-package-transaction-candidate-evidence-20261003.md` (29/29 PASS with default TMPDIR, no override; alias regression, five SIGKILL boundaries and POSIX fsync passed; earlier 28-test runs retained, production gates remain open)
- `windows-console-visual-result-20261003.md` (PASS: operator-observed console-free release launch/query, renderer status/IPC and clean exit 0)
- `windows-desktop-login-result-20261003.md` (scoped PASS: automatic hidden login, tray Open, IPC, registration removal and clean exit; daemon console allocation found and fixed in source, rebuilt physical proof pending)
- `macos-background-command-check-plan-20261003.md` (revalidated at `808bb1c`; no repeated Mac physical tests)
- `macos-background-command-check-evidence-20261003.md` (PASS at `808bb1c`: format/check/strict Clippy and 2/2 runtime tests; Mac lint gate closed, initial failure retained)
- `windows-physical-release-tray-20261003.md` (PASS: native title-bar close, physical tray Open and active-daemon Quit exit 0; Desktop login remains open)
- `windows-ci-package-build-20261003.md` (one manual Windows package/artifact job to unblock fresh build production; signing and physical-host acceptance remain open)
- `macos-tray-input-diagnostic-plan-20261003.md` (completed physical-input diagnosis: tray left restore, Open and active-daemon Quit exit 0 verified at `dfbaded`)
- `macos-foundation-slice.md`
- `macos-supervision-comparison-protocol.md`
- `macos-native-supervision-evidence.md`
- `macos-desktop-supervision-evidence.md`
- `macos-mode-b-gui-window-close.md`
- `macos-update-rollback-protocol.md`
- `macos-update-rollback-evidence.md`
- `windows-desktop-supervision-evidence.md` (historical Windows Mode B process-supervisor run; later Windows daemon IPC reconnect proof is recorded separately; physical Tauri GUI close remains open)
- `windows-desktop-ipc-reconnect-20261002.md` (physical Windows shared Desktop Rust client reconnect after supervised daemon restart; Tauri renderer invocation remains open)
- `windows-tauri-renderer-ipc-reconnect-20261003.md` (physical Windows rendered health/reconnect path after clean replacement and during external Mode B crash/restart; production Desktop supervision and packaging remain open)
- `windows-release-desktop-owned-supervisor-20261003.md` (partial Windows release Desktop physical run; Code Integrity build block, child restart, close/restore, duplicate and cleanup results)
- `../windows-release-build-20261003.md` (current Tauri CLI release executable loaded its embedded renderer and passed the Windows renderer IPC flow)
- `macos-desktop-ipc-reconnect-20261003.md` (physical macOS shared Desktop Rust client reconnect after supervised daemon restart; Tauri renderer invocation remains open)
- `macos-tauri-renderer-ipc-reconnect-20261003.md` (physical macOS Tauri renderer health and reconnect command invocation; production packaging remains open)
- `macos-renderer-crash-reconnect-test-plan-20261003.md` (PASS at `dc0ae10`: visible Mac renderer reconnected during the external Mode B supervised crash/restart; PID matched the harness)
- `windows-mode-b-invalid-harness-20261002.md` (historical FAILED/INVALID HARNESS EVIDENCE; superseded by r2)
- `windows-supervision-comparison-protocol.md` (Windows Mode A comparison; Task Scheduler permission failure recorded; HKCU Run `native_hkcu_run` login-start probe harness; ADR-0012 still open)
- `supervision-candidate-comparison-20261003.md` (current cross-platform Mode A/Mode B evidence comparison and remaining P0-2 gates; no architecture selected)
- `windows-mode-a-task-scheduler-permission-20261002.md` (Mode A schtasks FAILED CANDIDATE / PERMISSION)
- `windows-mode-a-hkcu-run-20261002.md` (Windows physical-host AUTOMATED HKCU Run register/run/cleanup for this experiment; separate reboot evidence is in the post-login audit)
- `windows-hkcu-post-login-audit-20261002.md` (records handoff's physical login-start success / post-login daemon exit and read-only source audit; root cause remains OPEN)
- `windows-mode-b-gui-window-close-20261002.md` (Windows physical Tauri CloseRequested/hide/restore proof; verified for this flow only)
- `windows-update-rollback-evidence-20261002.md` (Windows physical synthetic compatible update/reject/rollback marker safety; verified for harness behavior only)
- `macos-update-compat-test-plan-20261003.md` (completed Mac validation procedure for the synthetic Desktop/daemon protocol-pair rejection and update/rollback repeat)
- `macos-update-compat-evidence-20261003.md` (PASS at `46169ee`: 33 local tests and all synthetic protocol-pair/update/rollback acceptance fields passed on macOS)
- `macos-physical-traffic-light-close-test-plan-20261003.md` (completed Mac operator procedure for physical traffic-light close and Dock restore)
- `macos-physical-traffic-light-close-evidence-20261003.md` (PASS at evidence commit `52e9dc4`, using test build source `aed973d`: operator red close/Dock restore preserved daemon PID `27600`; clean quit released the lock)
- `windows-parent-exit-control-20261002.md` (isolated Windows child-process survival after launcher-shell exit; does not reproduce HKCU Run or explain post-login exit)
- `windows-hkcu-stop-marker-cleanup-20261002.md` (isolated physical Windows regression proof that `--leave-registered` removes its one-shot stop marker while retaining then cleaning the test registry value)
- `windows-instrumented-login-preparation-20261002.md` (verified observer/preparation controls, compact Run command limit guard, and armed `wl-1002a` experiment; actual next login and exit cause remain pending)
- `windows-instrumented-login-result-20261002.md` (regular Windows logon confirmed, but neither Cryptex Run value produced an observed start in 11.4 minutes; observer output absent; values cleaned; daemon exit cause remains open)
- `windows-reboot-login-survival-repeat-20261003.md` (second physical reboot/login, 601-second daemon survival, visible test console cause, and exact registration/process cleanup)
