# P0-2 Mode A — native macOS LaunchAgent supervision evidence

**Date (UTC):** 2026-09-30T23:41:00Z  
**Host:** macOS 27.0.1 / aarch64  
**Branch:** `m0/p0-2-cursor`  
**Experiment ID:** `macos-mode-a-20260930T2340Z`  
**Runtime root:** `~/Library/Application Support/Cryptex-Dev/p0-2/compare/mode-a`  
**Label:** `com.cryptex.dev.p0-2.compare` (test-only, reversible)

## Methodology

1. Install plist under Cryptex-Dev compare runtime (not `~/Library/LaunchAgents` default pollution path for persistence; loaded via `launchctl bootstrap`)
2. `KeepAlive.SuccessfulExit=false` so crash restarts but clean exit does not loop
3. Wait for ready file / single-instance lock
4. Duplicate `cryptexd lifecycle probe` must exit 1
5. `kill -9` daemon PID; observe lock release and launchd restart
6. Observe status after restart
7. stop-file + `bootout` + plist remove
8. Confirm label unloaded

No reboot/logout was performed (P0-1 active).

## Measurements (VERIFIED unless noted)

| Field | Result | Class |
|---|---|---|
| time_to_daemon_ready_ms | 174 | AUTOMATED |
| duplicate_launch_result | rejected exit=1 | AUTOMATED / VERIFIED |
| main_window_close_result | N/A (no Desktop window supervisor) | AUTOMATED |
| daemon_alive_after_window_close | N/A | AUTOMATED |
| crash_detected | lock released after SIGKILL | AUTOMATED / VERIFIED |
| restart_success | new pid after KeepAlive | AUTOMATED / VERIFIED |
| restart_latency_ms | 9992 | AUTOMATED / VERIFIED |
| clean_shutdown_result | stop-file + bootout + remove | AUTOMATED / VERIFIED |
| runtime_cleanup_result | plist removed, label unloaded | AUTOMATED / VERIFIED |
| login_startup_result | not performed | NOT YET VERIFIED |
| desktop_reconnect_result | status observe after restart only | PARTIAL (full Desktop IPC reconnect NOT YET VERIFIED) |

Raw KV retained outside git under the Cryptex-Dev runtime measurements directory.

## Security / isolation implications

- Per-user `gui/$UID` domain only
- Test label namespaced `com.cryptex.dev.p0-2.*`
- Teardown verified; default LaunchAgents dir had no Cryptex leftovers
- P0-1 Syncthing homes untouched

## Recovery / update implications

- Native supervision can restart daemon without Desktop process
- Update orchestration still unresolved: replacing binary while launchd holds KeepAlive needs a later controlled experiment
- Synthetic marker update/rollback fixtures remain separate foundation evidence

## Limitations

- Does not prove login item persistence across reboot
- Does not prove Desktop reconnect protocol (P0-3 overlap)
- LaunchAgent plist content is spike-only, not production packaging

## Decision note

No architecture winner. ADR-0012 remains unresolved.
