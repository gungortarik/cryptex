# Windows Tauri renderer IPC reconnect proof

**Date:** 2026-10-03 UTC

**Host:** Windows 11 Pro / x86_64

**Branch:** `m0/p0-2-cursor`

**Source:** `ff179dabff0b01854a4332bcfcafbd26e3b5f5e0`

**Daemon binary:** release build from `6cd5b3b`; daemon/Desktop Rust sources are
unchanged through `ff179da` (intervening commits update evidence and docs).

**Classification:** AUTOMATED physical Windows Tauri UI evidence

## Method

The Tauri Desktop was launched with `pnpm tauri dev`; its Vite frontend and
visible WebView2 window were live. WebView2 DevTools Protocol drove the
rendered button elements and read their visible status text. The buttons still
invoked the production Tauri commands through the renderer IPC bridge. The
daemon binary was the repository-built release `cryptexd.exe`.

Runtime data remained outside the repository at:

```text
%LOCALAPPDATA%\Cryptex-Dev\p0-2\compare\mode-b-desktop
```

## Result

| Renderer action | Result |
|---|---|
| `Start probe daemon` | PASS; renderer reported PID `29556` |
| `IPC health ping` | PASS; `IPC health.pong`, daemon PID `29556`, protocol `1` |
| `Stop daemon` | PASS; clean stop requested through renderer command |
| Replacement `Start probe daemon` | PASS; new daemon PID `1336` |
| `Wait for IPC reconnect` | PASS; renderer reported `IPC reconnected after restart · pid 29556 → 1336` |
| Final stop and `Quit clean` | PASS; daemon lock released, Desktop exited with code `0` |

After cleanup, `lifecycle.status` recorded `pid=none`, `instance=stale`,
`health=attention`, and `note=clean shutdown released instance lock`.

The Tauri dev host printed a WebView2 window-class unregister diagnostic
(`Chrome_WidgetWin_0`, error `1412`) during shutdown; the app process exited
with code `0`, the endpoint closed, and the daemon lock was released.

## Scope and limits

This verifies that the physical Windows Tauri renderer can invoke the health
and reconnect commands and reach the real named-pipe daemon before and after a
cleanly stopped/replaced daemon process. The UI's `Wait for IPC reconnect`
command was invoked after the replacement was already running. This does not
verify automatic restart after an unexpected crash, production packaging,
literal Windows traffic-light interaction, or ADR-0012 acceptance.
