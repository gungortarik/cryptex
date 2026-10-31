# P0-2 Mode B — interactive Tauri GUI window-close evidence

**Date (UTC):** 2026-09-30T23:58:00Z  
**Host:** macOS 27.0.1 / aarch64  
**Branch:** `m0/p0-2-cursor`  
**Runtime root:** `~/Library/Application Support/Cryptex-Dev/p0-2/compare/mode-b-desktop`

## GUI environment

| Field | Value |
|---|---|
| Desktop command | `cargo build -p cryptex-desktop` then run `target/.../cryptex-desktop` with Vite `pnpm --filter @cryptex/desktop dev` on `127.0.0.1:1420` |
| Alternative repo command | `pnpm --filter @cryptex/desktop tauri dev` (also works; used earlier for launch smoke) |
| CRYPTEXD_BIN | workspace `debug/cryptexd` |
| UI elements enabled (TCC) | `false` (agent cannot click traffic-light) |

P0-1 Syncthing (`syncthing-p0-1`) remained running and untouched.

## Close-path methodology (important)

1. **Attempted PHYSICAL_MANUAL traffic-light / Cmd+W via AppleScript:** blocked — `System Events` reports `UI elements enabled=false` and keystroke permission denied.
2. **Verified CloseRequested path:** `close.request` calls `WebviewWindow::close()`, which emits the same `WindowEvent::CloseRequested` handled by the Mode B hook (`prevent_close` + hide + record event).

Therefore:

- CloseRequested hide-on-close behavior is **VERIFIED**
- Literal traffic-light mouse click remains **NOT YET VERIFIED** (Accessibility TCC blocker)

This is not treated as an architecture discrepancy: observed CloseRequested behavior matches the coded hook.

## Measurements

| Field | Value | Class |
|---|---|---|
| desktop_pid_before_close | 19202 | VERIFIED |
| daemon_pid_before_close | 19214 | VERIFIED |
| desktop_alive_after_close | true | VERIFIED |
| daemon_alive_after_close | true | VERIFIED |
| daemon_pid_after_close | 19214 (unchanged) | VERIFIED |
| duplicate_instance_count | 1 before/after close; 1 after dup attempt | VERIFIED |
| window_restore_success | true (`show.request`) | VERIFIED |
| desktop_alive_after_restore | true | VERIFIED |
| daemon_alive_after_restore | true | VERIFIED |
| duplicate_start_rejected | true (`start.error` + probe exit 1) | VERIFIED |
| clean_shutdown_result | Desktop+daemon exited via `quit.request`; status `instance=stale` | VERIFIED |
| traffic-light physical click | blocked by Accessibility | NOT YET VERIFIED |
| full Desktop IPC reconnect | not tested | NOT YET VERIFIED |
| login/reboot | not tested | NOT YET VERIFIED |

### Raw event excerpt (after close)

```
event=window_close
source=tauri_close_requested
supervisor_alive=true
daemon_expected_alive=true
daemon_pid=19214
desktop_pid=19202
```

## Answers to the proof questions

1. **Does Tauri Desktop launch?** Yes — `gui-proof.snapshot` `note=desktop_launched`, `window_visible=true`.
2. **Does Mode B supervise cryptexd?** Yes — Desktop PID 19202 supervised daemon 19214 (`state=active`).
3. **On main-window close (CloseRequested path):** window is hidden (not destroyed); Desktop process remains alive; cryptexd remains alive with same PID.
4. **Restore/reopen without second daemon?** Yes — `show.request` restored window; daemon PID unchanged; instance count stayed 1.
5. **Single-instance still effective?** Yes — second `start.request` wrote supervision error; `lifecycle probe` exited 1.
6. **Clean termination?** Yes — `quit.request` stopped owned daemon and exited Desktop only; P0-1 Syncthing untouched.
7. **Runtime truthful/outside repo?** Yes — under Cryptex-Dev/p0-2/compare/mode-b-desktop; no repo runtime residue.

## Developer-only support added for this proof

- `p0_2_show_main_window`, `p0_2_write_snapshot`, `p0_2_quit_clean`
- macOS Dock `RunEvent::Reopen` restore
- control files: `start.request`, `close.request`, `show.request`, `quit.request`
- refuse second supervised start inside the same Desktop process

No product tray UX. ADR-0012 not selected.
