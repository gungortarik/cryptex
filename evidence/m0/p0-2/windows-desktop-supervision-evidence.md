# P0-2 Mode B — Windows Desktop/sidecar supervision evidence

**Date (UTC):** 2026-10-02T05:04:08Z  
**Host:** Windows physical machine / x86_64  
**Branch:** `m0/p0-2-cursor`  
**Commit:** `c8c86dc5bc93438309c5fd0df3ef7991342c28c0`  
**Experiment ID:** `windows-mode-b-r2-20261002T050408Z`  
**Architecture:** x86_64  
**Mode:** `desktop_supervisor` (Mode B process harness)  
**Status:** VERIFIED for AUTOMATED process-supervisor behaviors listed below only

## Relation to earlier Windows attempt

The first Windows Mode B attempt is recorded in
`windows-mode-b-invalid-harness-20261002.md` as **FAILED / INVALID HARNESS EVIDENCE**
(observe lock-byte read / os error 33). That run is **not** a lifecycle verdict.

This r2 run is the first **valid** Windows Mode B physical foundation evidence after the
observe-without-locked-byte-read fix.

## Methodology

### AUTOMATED process supervisor (`DesktopSupervisor`)

1. Parent supervisor process spawns `cryptexd lifecycle hold`
2. Duplicate probe must exit 1
3. Record process-level window-close event while supervisor+daemon remain alive
4. Force-kill child; observe lock release
5. Supervisor restart path respawns daemon
6. Clean stop via stop-file
7. Remove control files; keep measurement KV

### Still not executed on Windows

- PHYSICAL_MANUAL Tauri GUI window-close
- login / reboot startup registration
- full Desktop IPC reconnect after supervised restart

These remain OPEN / NOT YET VERIFIED. They are not blocked by P0-1 (P0-1 is complete);
absence of those proofs is simply unfinished P0-2 evidence.

## Measurements

| Field | Result | Class |
|---|---|---|
| time_to_daemon_ready_ms | 573 | AUTOMATED / VERIFIED |
| duplicate_launch_result | rejected exit=1 | AUTOMATED / VERIFIED |
| main_window_close_result | process-level event recorded | AUTOMATED / VERIFIED (physical Tauri GUI close NOT YET VERIFIED) |
| daemon_alive_after_window_close | true | AUTOMATED / VERIFIED (process-level) |
| crash_detected | supervisor observed child/lock release | AUTOMATED / VERIFIED |
| restart_attempted | DesktopSupervisor::restart_daemon | AUTOMATED / VERIFIED |
| restart_success | restarted pid=24368 | AUTOMATED / VERIFIED |
| restart_latency_ms | 32 | AUTOMATED / VERIFIED |
| stale_lock_recovery | waited for lock release before respawn | AUTOMATED / VERIFIED |
| clean_shutdown_result | stop-file clean shutdown | AUTOMATED / VERIFIED |
| runtime_cleanup_result | control files removed; measurement retained | AUTOMATED / VERIFIED |
| desktop_reconnect_result | status observe after supervised restart succeeded | AUTOMATED / PARTIAL (full Desktop IPC reconnect NOT YET VERIFIED) |
| errors | (empty) | AUTOMATED / VERIFIED |
| physical Tauri GUI window-close | not performed | NOT YET VERIFIED |
| login / reboot startup | not performed | NOT YET VERIFIED |
| full Desktop IPC reconnect | not performed | NOT YET VERIFIED |

## Security / isolation implications

- Supervisor is a normal user process (no Windows startup registration in this path)
- Runtime confined to Cryptex-Dev/p0-2 compare Mode B roots (outside the repository)
- Exclusive instance lock remained authoritative; duplicate launch rejected
- No Windows login-item / startup entry was installed

## Recovery / update implications

- Process-level crash detect + restart worked on Windows physical hardware
- Fast restart latency does not prove package-update atomicity
- Orphaning vs stopping child on Desktop quit remains an OPEN product decision for Mode B
- Windows startup registration is intentionally not implemented yet

## Decision note

No architecture winner. ADR-0012 remains unresolved. Mode A and Mode B are both still under
comparison; this evidence does **not** select Mode B and does **not** accept ADR-0012.