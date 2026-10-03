# P0-2 Mode B — Desktop/sidecar supervision evidence

**Date (UTC):** 2026-09-30T23:41:00Z  
**Host:** macOS 27.0.1 / aarch64  
**Branch:** `m0/p0-2-cursor`  
**Experiment ID:** `macos-mode-b-20260930T2340Z`  
**Runtime root:** `~/Library/Application Support/Cryptex-Dev/p0-2/compare/mode-b`

## Methodology

### AUTOMATED process supervisor (`DesktopSupervisor`)

1. Parent supervisor process spawns `cryptexd lifecycle hold`
2. Duplicate probe must exit 1
3. Record process-level window-close event while supervisor+daemon remain alive
4. Force-kill child; observe lock release
5. Supervisor restart path respawns daemon
6. Clean stop via stop-file
7. Remove control files; keep measurement KV

### PHYSICAL_MANUAL Tauri GUI

Desktop foundation now includes temporary developer hooks:

- spawn/status/stop probe daemon commands
- `CloseRequested` → prevent close, hide window, write `window-close.event`

Interactive Tauri window-close was **not** executed in this slice to avoid GUI session disruption while P0-1 remains active. Hooks are present for a later safe PHYSICAL_MANUAL run.

## Measurements

| Field | Result | Class |
|---|---|---|
| time_to_daemon_ready_ms | 29 | AUTOMATED / VERIFIED |
| duplicate_launch_result | rejected exit=1 | AUTOMATED / VERIFIED |
| main_window_close_result | process-level event recorded | AUTOMATED / VERIFIED (GUI close NOT YET VERIFIED) |
| daemon_alive_after_window_close | true | AUTOMATED / VERIFIED (process-level) |
| crash_detected | child/lock release observed | AUTOMATED / VERIFIED |
| restart_success | restarted with new pid | AUTOMATED / VERIFIED |
| restart_latency_ms | 28 | AUTOMATED / VERIFIED |
| clean_shutdown_result | stop-file clean shutdown | AUTOMATED / VERIFIED |
| runtime_cleanup_result | control files removed | AUTOMATED / VERIFIED |
| login_startup_result | not performed | NOT YET VERIFIED |
| desktop_reconnect_result | shared Rust client reconnected after supervised restart; pid=17128 -> 17130 | AUTOMATED / VERIFIED (Tauri renderer invocation remains open) |
| Tauri interactive window-close | hooks ready, not run | NOT YET VERIFIED |

## Latest production IPC reconnect proof

The matching physical macOS runtime proof is recorded in
[`macos-desktop-ipc-reconnect-20261003.md`](macos-desktop-ipc-reconnect-20261003.md).
It verifies the shared Desktop Rust client against the real daemon transport
before and after a supervised restart. It does not verify a visible Tauri
renderer invocation.

## Security / isolation implications

- Supervisor is a normal user process (no LaunchAgent in Mode B automated path)
- Runtime confined to Cryptex-Dev/p0-2/compare/mode-b
- Renderer still must not gain arbitrary shell/fs access beyond narrow commands
- Temporary developer UI controls are not product UX

## Recovery / update implications

- If Desktop process is the supervisor, Desktop crash/quit policy must be defined before ADR-0012
- Fast restart latency in-process does not prove package-update atomicity
- Orphaning vs stopping child on app quit remains an OPEN product decision for Mode B

## Interactive GUI window-close

See `macos-mode-b-gui-window-close.md`.

CloseRequested hide-on-close with daemon survival is **VERIFIED**. Literal traffic-light click remains **NOT YET VERIFIED** while macOS Accessibility UI elements are disabled for the agent session.

## Decision note

No architecture winner. ADR-0012 remains unresolved. Hybrid is not selected; further evidence may still be needed after GUI close + Windows + login proofs.
