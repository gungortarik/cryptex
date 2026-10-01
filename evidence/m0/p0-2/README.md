# M0 P0-2 — Background Runtime / Atomic Update Lifecycle Evidence

**Status:** IN PROGRESS (update/rollback synthetic proof VERIFIED; ADR still open)  
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

## Current foundation slice

Implemented and test-backed in `services/cryptexd`:

- single-instance lock probe
- structured lifecycle status output
- crash/restart harness commands
- protocol-compat + synthetic canonical-marker safety fixtures
- platform registration adapters left as `not_configured`

Comparison harness added:

- `tools/m0-lifecycle-compare` Mode A/B runners
- measurement contract + KV capture
- reversible macOS LaunchAgent test harness (Mode A)
- DesktopSupervisor process harness (Mode B)
- temporary Desktop hide-on-close developer hooks
- interactive Mode B GUI CloseRequested proof (`macos-mode-b-gui-window-close.md`)
- test-only staged binary update / failed update / rollback proof (`macos-update-rollback-evidence.md`)

See:

- `macos-foundation-slice.md`
- `macos-supervision-comparison-protocol.md`
- `macos-native-supervision-evidence.md`
- `macos-desktop-supervision-evidence.md`
- `macos-mode-b-gui-window-close.md`
- `macos-update-rollback-protocol.md`
- `macos-update-rollback-evidence.md`
