# P0-2 Mode B — Windows Tauri GUI close/hide proof

> **Historical status note (2026-10-05):** Status statements below reflect the state when this dated plan/result was recorded, before M0 closeout. Current M0 status is COMPLETE for the synthetic foundation scope; scoped ADR-0012/0004 are ACCEPTED. This note does not expand this artifact's evidence, satisfy production residuals, or authorize M1. See `evidence/m0/m0-closure-audit-20261005.md`.

**Date (local):** 2026-10-02 15:11–15:12 America/New_York  
**Host:** Windows physical machine / x86_64  
**Branch:** `m0/p0-2-cursor`  
**Mode:** Tauri Desktop supervising `cryptexd`  
**Status:** VERIFIED for the automated physical Tauri `CloseRequested` event path only

## Method

Started the Windows Tauri development application and its bundled Vite frontend.
With the app's P0-2 control-file hook, started the built `cryptexd.exe`, requested
window close (which calls Tauri `Window::close()` and enters the same
`CloseRequested` handler), then requested window restore. The runtime was kept
outside the Git repository at:

```text
%LOCALAPPDATA%\Cryptex-Dev\p0-2\compare\mode-b-desktop
```

## Observed results

| Observation | Result |
|---|---|
| Desktop process | PID 31872 (`cryptex-desktop`) remained alive while hidden |
| Daemon process | PID 33584 (`cryptexd`) remained alive while window was hidden |
| Close event | `source=tauri_close_requested`; supervisor and daemon expected alive |
| Hidden snapshot | `note=window_closed_hidden`, `window_visible=false`, daemon PID 33584 |
| Lifecycle while hidden | `instance=held`, `health=active`, PID 33584 |
| Restore event | `source=show.request`; snapshot `window_visible=true` |
| Cleanup | stop file released daemon lock; lifecycle became `instance=stale`, `health=attention`; app quit request exited Desktop |

The daemon's final lifecycle record reported `pid=none`, `recorded_pid=33584`,
and `note=clean shutdown released instance lock`.

## Stale-ready PID regression proof

Repeated the physical Windows Tauri close flow after changing the runtime
`ready` file to the deliberately invalid historical PID `99999` (Desktop PID
30248):

- While the daemon lock was held, the snapshot recorded live lock-holder PID
  `35252`, not the stale `ready` value, and marked
  `daemon_pid_observation=live`.
- After cleanly stopping the daemon while leaving `ready=99999`, lifecycle
  status reported `pid=none`, `recorded_pid=35252`, `instance=stale`.
- A subsequent Tauri `CloseRequested` snapshot and event both recorded
  `daemon_pid=none` and `daemon_pid_observation=not_running` despite the
  unchanged stale ready file.

This confirms these Desktop proof artifacts now query a fresh daemon lifecycle
observation instead of treating the startup marker as a current PID. The
snapshot records `unavailable` plus an error when lifecycle status cannot be
queried or parsed, rather than silently classifying that case as stopped.

## Limits

This is a physical Windows host run through the application's automated
control-file hook; it is not a manually clicked title-bar close test. It verifies
the Tauri `CloseRequested` handling and the window-hide/restore lifecycle. It
does not verify full Desktop IPC reconnect after daemon restart, login/reboot
startup, or crash recovery, and it does not select a supervision architecture.
ADR-0012 remains unresolved.
