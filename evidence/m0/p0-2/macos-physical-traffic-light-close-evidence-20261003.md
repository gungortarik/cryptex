# macOS physical traffic-light close and Dock restore evidence

**Status:** PASS / VERIFIED for the Mode B test build

**Date:** 2026-10-03

**Source:** `aed973dc7a58bdedb7becf01738b7d743887a739`

**Branch:** `m0/p0-2-cursor`

**Host:** macOS 27.0.1 / arm64

## Procedure and result

`cargo build -p cryptexd --locked` passed. Desktop was launched with
`CRYPTEXD_BIN` pointing to the repository-built debug daemon through
`pnpm --filter @cryptex/desktop tauri dev`. The renderer's `Start probe daemon`
reported `daemon started pid=27600`; Desktop PID was `27505`.

The operator was instructed to physically click the red traffic-light close
control, confirm the window disappeared with its Dock icon remaining, and
click the Dock icon to reopen. The operator confirmed completion with
`actim`. Neither physical action was synthesized by UI automation.

After reopening, the visible renderer's `Daemon status` returned:

```text
pid=27600
recorded_pid=27600
instance=held
health=active
note=observed without acquiring instance lock
```

Both Desktop `27505` and daemon `27600` were independently observed alive.
The daemon PID was unchanged from before the physical close.

## Runtime evidence

Retained root, outside Git:
`/Users/tarikgungor/Library/Application Support/Cryptex-Dev/p0-2/compare/mode-b-desktop`

`window-close.event` after the operator's close:

```text
event=window_close
source=tauri_close_requested
supervisor_alive=true
daemon_expected_alive=true
daemon_pid=27600
daemon_pid_observation=live
desktop_pid=27505
```

`desktop-supervisor.state` after Dock reopening:

```text
supervisor_pid=27505
state=window_restored
daemon_pid=27600
supervisor_alive=true
```

## Clean exit

The renderer's `Quit clean` was invoked after verification. The Tauri dev
command exited 0. Neither test Desktop nor test daemon remained in the process
list. A fresh `cryptexd lifecycle status` observation returned `pid=none`,
`recorded_pid=27600`, `instance=stale`: the retained lock file was no longer
held. Supervisor state was:

```text
supervisor_pid=27505
state=clean_quit
daemon_pid=none
supervisor_alive=false
```

No start, lifecycle or cleanup errors occurred. An initial automation lookup
ran before the renderer loaded; retrying after load successfully started the
daemon. UI automation was used only for probe buttons/status and clean quit;
the operator performed the physical close and Dock restore.

This closes the literal macOS traffic-light close/hide/restore and same-daemon
survival evidence gate for this test build. Production login integration,
tray UX, supervision ownership and ADR-0012 remain open. No application code,
login registration or P0-1 runtime data was changed.
