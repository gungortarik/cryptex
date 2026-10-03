# Windows M0 test results — consolidated evidence index

**Snapshot date:** 2026-10-03 UTC

**Branch:** `m0/p0-2-cursor`
**Purpose:** Repository-local index of the Windows physical-host and Windows CI
results already recorded in the detailed evidence files below.

This file is a navigation and status summary. The linked detailed evidence
files remain the source of the exact commands, timestamps, experiment IDs,
runtime paths and retained artifacts. A result is marked **VERIFIED** only for
the scope explicitly covered by its source evidence.

## Verified Windows results

| Area | Result | Environment / scope | Detailed evidence |
|---|---|---|---|
| Release foundation build | **VERIFIED** | Physical Windows host; Rust workspace and Tauri Desktop release build with embedded frontend | [`windows-release-build-20261002.md`](windows-release-build-20261002.md) |
| Current Tauri release executable | **VERIFIED** | Physical Windows host; Tauri CLI produced embedded renderer; release executable loaded and passed renderer IPC health/replacement flow | [`windows-release-build-20261003.md`](windows-release-build-20261003.md) |
| IPC candidate transport | **VERIFIED** | Physical Windows host; `interprocess` 2.4.4 named-pipe roles, protocol/capability rejection, malformed/oversized frames and reconnect after server restart | [`windows-ipc-spike-20261002.md`](p0-3/windows-ipc-spike-20261002.md) |
| Real daemon Desktop IPC client reconnect | **VERIFIED within harness scope** | Physical Windows host; shared Tauri Rust health client reconnected to real `cryptexd` after `DesktopSupervisor` restart; Tauri UI invocation was not exercised | [`windows-desktop-ipc-reconnect-20261002.md`](p0-2/windows-desktop-ipc-reconnect-20261002.md) |
| Tauri renderer IPC invocation | **VERIFIED within recorded flow** | Physical Windows host; rendered health and reconnect buttons invoked production Tauri commands against real daemon PIDs `29556 -> 1336` after clean stop/replacement | [`windows-tauri-renderer-ipc-reconnect-20261003.md`](p0-2/windows-tauri-renderer-ipc-reconnect-20261003.md) |
| IPC cross-account ACL | **VERIFIED** | Disposable GitHub-hosted Windows runner; authenticated distinct non-admin account denied, owner reconnect preserved | [`windows-cross-account-acl-20261002.md`](p0-3/windows-cross-account-acl-20261002.md) |
| Mode B lifecycle foundation | **VERIFIED** | Physical Windows host; duplicate rejection, crash detection, supervised restart, 32 ms restart latency, clean stop and runtime cleanup | [`windows-desktop-supervision-evidence.md`](p0-2/windows-desktop-supervision-evidence.md) |
| Tauri close/hide path | **VERIFIED within stated scope** | Physical Windows Tauri process using the automated `CloseRequested` path; Desktop and daemon survived hide and restored | [`windows-mode-b-gui-window-close-20261002.md`](p0-2/windows-mode-b-gui-window-close-20261002.md) |
| HKCU Run reboot/login survival | **VERIFIED for the measured run** | Physical Windows reboot/login; daemon started after login and remained alive for the retained 600-second observation | [`windows-reboot-login-survival-20261002.md`](p0-2/windows-reboot-login-survival-20261002.md) |
| Synthetic update/rollback safety | **VERIFIED within comparison harness** | Physical Windows host; compatible activation, bad-candidate rejection, rollback and unchanged synthetic marker | [`windows-update-rollback-evidence-20261002.md`](p0-2/windows-update-rollback-evidence-20261002.md) |

## Valid negative or limited results

| Area | Result | Interpretation | Detailed evidence |
|---|---|---|---|
| Task Scheduler Mode A candidate | **FAILED CANDIDATE / PERMISSION** | `schtasks /Create` returned `ERROR: Access is denied` in a normal interactive user session; no elevation or bypass was used | [`windows-mode-a-task-scheduler-permission-20261002.md`](p0-2/windows-mode-a-task-scheduler-permission-20261002.md) |
| HKCU Run harness-only probe | **PARTIAL** | Registration/run/cleanup passed, but that experiment did not perform interactive login or reboot | [`windows-mode-a-hkcu-run-20261002.md`](p0-2/windows-mode-a-hkcu-run-20261002.md) |
| Automatic restart while renderer waits | **OPEN** | The renderer invoked reconnect after a cleanly stopped daemon had been replaced; unexpected-crash restart while the command is waiting was not exercised | [`windows-tauri-renderer-ipc-reconnect-20261003.md`](p0-2/windows-tauri-renderer-ipc-reconnect-20261003.md) |

## Reproduction and integrity notes

- Runtime data and retained measurements were kept outside the Git checkout,
  primarily under `%LOCALAPPDATA%\Cryptex-Dev\...`.
- The Windows IPC candidate evidence includes the exact process-level
  reconnect result and the separate authenticated cross-account ACL proof; the
  latter intentionally runs only on a disposable Windows CI VM.
- Build hashes, CI run links and source commits are preserved in the detailed
  evidence files rather than duplicated here.
- These results do not select a supervision architecture, accept ADR-0012,
  or complete M0 by themselves.

## Still open

- Unexpected-crash automatic restart observed while the Tauri renderer's
  reconnect command is waiting; current physical UI evidence covers clean
  stop/replacement on both Windows and macOS.
- Final Mode A versus Mode B comparison and ADR-0012 acceptance.
- Remaining packaging, managed startup and other M0 acceptance gates.
