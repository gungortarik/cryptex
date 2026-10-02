# M0 P0-2 — Background Runtime / Atomic Update Lifecycle Evidence

**Status:** IN PROGRESS  
**ADR-0012:** UNRESOLVED (do not accept yet)

This directory records reproducible evidence for M0 P0-2 only. It does not
select or freeze the production supervision/update architecture.

## Authority

- `docs/11_DAEMON_IPC_RUNTIME.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/26_VERSIONING_MIGRATIONS_AND_UPDATES.md`
- `docs/milestones/M0_FOUNDATION.md`
- `checklists/M0_FOUNDATION_GATE.md`

If evidence conflicts with an accepted architectural decision, stop and use the
ADR/specification process.

## Options under comparison (still OPEN)

1. native per-user background registration (macOS LaunchAgent + Windows per-user startup)
2. Tauri/tray supervisor spawning `cryptexd` sidecar
3. hybrid only if evidence justifies it

No option has been selected.

## Safety and isolation rules

- Use only synthetic/runtime test state.
- Store runtime under `Cryptex-Dev/p0-2`, never inside the source repository.
- Do not touch P0-1 Syncthing paths (`Cryptex-Dev/syncthing-p0-1*`).
- Do not install LaunchAgents / Windows startup entries until comparison evidence exists.
- Do not mark P0-2 complete and do not accept ADR-0012 from foundation-slice work alone.
- Login/reboot startup proofs remain OPEN; historical harness notes that deferred reboot
  "while P0-1 active" are stale now that P0-1 is complete and must not be treated as a
  current blocker.

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

- `macos-foundation-slice.md`
- `macos-supervision-comparison-protocol.md`
- `macos-native-supervision-evidence.md`
- `macos-desktop-supervision-evidence.md`
- `macos-mode-b-gui-window-close.md`
- `macos-update-rollback-protocol.md`
- `macos-update-rollback-evidence.md`
- `windows-desktop-supervision-evidence.md` (Windows Mode B AUTOMATED — VERIFIED for listed fields only; GUI close / login-reboot / full Desktop IPC reconnect remain NOT YET VERIFIED)
- `windows-mode-b-invalid-harness-20261002.md` (historical FAILED/INVALID HARNESS EVIDENCE; superseded by r2)
- `windows-supervision-comparison-protocol.md` (Windows Mode A comparison; Task Scheduler permission failure recorded; HKCU Run `native_hkcu_run` login-start probe harness; ADR-0012 still open)
- `windows-mode-a-task-scheduler-permission-20261002.md` (Mode A schtasks FAILED CANDIDATE / PERMISSION)
- `windows-mode-a-hkcu-run-20261002.md` (Windows physical-host AUTOMATED HKCU Run register/run/cleanup for this experiment; separate reboot evidence is in the post-login audit)
- `windows-hkcu-post-login-audit-20261002.md` (records handoff's physical login-start success / post-login daemon exit and read-only source audit; root cause remains OPEN)
- `windows-mode-b-gui-window-close-20261002.md` (Windows physical Tauri CloseRequested/hide/restore proof; verified for this flow only)
- `windows-update-rollback-evidence-20261002.md` (Windows physical synthetic compatible update/reject/rollback marker safety; verified for harness behavior only)
- `windows-parent-exit-control-20261002.md` (isolated Windows child-process survival after launcher-shell exit; does not reproduce HKCU Run or explain post-login exit)
- `windows-hkcu-stop-marker-cleanup-20261002.md` (isolated physical Windows regression proof that `--leave-registered` removes its one-shot stop marker while retaining then cleaning the test registry value)
