# Windows Tauri renderer IPC reconnect proof

**Date:** 2026-10-03 UTC

**Host:** Windows 11 Pro / x86_64

**Branch:** `m0/p0-2-cursor`

**Tauri release source:** `11c65a8ba56696b7d6d200bf0fde56608299fadb`

**Earlier development-mode run:** source `ff179da`; daemon binary from `6cd5b3b`
(intervening source changes were documentation only).

**Classification:** AUTOMATED physical Windows Tauri UI evidence

## Method

Two physical Windows renderer runs were performed. The development-mode run
used `pnpm tauri dev`; the release-mode run used the repository's Tauri CLI
build (`pnpm --filter @cryptex/desktop tauri build --no-bundle`) and loaded
embedded assets at `http://tauri.localhost/`. In both runs, WebView2 DevTools
Protocol drove the rendered button elements and read their visible status
text. The buttons invoked the real Tauri commands through the renderer IPC
bridge. The selected daemon was the repository-built release `cryptexd.exe`.

Runtime data remained outside the repository at:

```text
%LOCALAPPDATA%\Cryptex-Dev\p0-2\compare\mode-b-desktop
```

## Result

| Renderer action | Result |
|---|---|
| Development run `Start probe daemon` | PASS; renderer reported PID `29556` |
| Development run `IPC health ping` | PASS; `IPC health.pong`, PID `29556`, protocol `1` |
| Development run `Wait for IPC reconnect` | PASS; renderer reported `29556 → 1336` after clean stop/replacement |
| Release run embedded frontend | PASS; actual WebView2 URL `http://tauri.localhost/` |
| Release run `Start probe daemon` / `IPC health ping` | PASS; `IPC health.pong`, PID `18376`, protocol `1` |
| Release run `Wait for IPC reconnect` | PASS; renderer reported `18376 → 28936` after clean stop/replacement |
| Final stop and `Quit clean` | PASS; daemon lock released; Desktop and debug endpoint exited |

After cleanup, `lifecycle.status` recorded `pid=none`, `instance=stale`,
`health=attention`, and `note=clean shutdown released instance lock`.

The development-mode host printed a WebView2 window-class unregister
diagnostic (`Chrome_WidgetWin_0`, error `1412`) during shutdown; that process
exited with code `0`, the endpoint closed, and the daemon lock was released.
The release-mode run completed cleanup without a remaining Desktop/daemon
process. Final retained status has `pid=none` and
`note=clean shutdown released instance lock`.

## Scope and limits

This verifies that the physical Windows Tauri renderer can invoke the health
and reconnect commands in both development and release executables and reach
the real named-pipe daemon before and after a cleanly stopped/replaced daemon
process. The UI's `Wait for IPC reconnect` command was invoked after each
replacement was already running. This does not verify automatic restart after
an unexpected crash, an installer, literal Windows traffic-light interaction,
or ADR-0012 acceptance.

## Supervised crash while the renderer waits — NOT YET VERIFIED

A coordinated physical Windows attempt used the release renderer and the
Mode B comparison harness with its optional `--pre-crash-hold-ms 8000` window.
The renderer's initial health ping reached the harness-owned daemon at PID
`25988`. The harness then force-killed it, restarted the daemon as PID `35808`
in `34 ms`, and its shared Rust IPC client reconnected. The retained measurement
is under
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\renderer-wait-20261003d\mode-b\measurements\renderer-wait-1791000931925.kv`.

The renderer was told to wait for the restart, but its visible status remained
at the initial health response throughout the controller's 12-second
observation. The renderer command itself has a 90-second timeout, so this is
**NOT VERIFIED**, not a demonstrated product timeout or a reconnect pass. A
separate login daemon (PID `31512`) remained active on this host and uses the
same named-pipe endpoint; endpoint contention is a possible test confounder,
not an established cause. That daemon and its registration were left untouched.

The Mode B crash/restart and shared Rust client reconnect passed independently.
The missing proof is specifically the visible Tauri renderer receiving the
supervisor's replacement PID while its reconnect command is pending.
