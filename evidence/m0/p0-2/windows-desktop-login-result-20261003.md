# Windows Desktop login startup — scoped PASS; console defect found

The operator completed real sign-out/sign-in on 2026-10-03, after the
ordinary-session preparation recorded in `windows-ci-package-build-20261003.md`.
The CI source was `0867b9f`, with the previously verified artifact hashes.

Explorer `22072` started at 21:49:11 EDT in session 2. It automatically launched
Desktop `29872` at 21:49:49 with `--cryptex-background`; Desktop started one
sibling daemon `26588` at 21:49:51. The previous Desktop `12084` and daemon
`8052` were replaced by this new login's processes. The exact per-user Cryptex
startup value was still present at initial inspection.

Computer Use exposed no visible Cryptex window before Open. The operator
confirmed that the main window did not appear at login, then physically selected
tray **Open Cryptex**. Computer Use then found its window. Renderer **Daemon
status** reported PID `26588`, `instance=held`, `health=active`; renderer
**IPC health ping** returned `health.pong`, that PID and protocol `1`.

The renderer disabled Desktop login startup. Windows `Win32_StartupCommand`
then showed zero product `Cryptex` entries, and the renderer reported disabled.
Renderer **Quit clean** stopped Desktop and daemon. An observer with a retained
handle to the exact Desktop process captured exit **0** at
`2026-10-04T01:54:05.4090101Z` (October 3 locally). Both PIDs disappeared.
Raw process and exit records are outside Git at
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\windows-desktop-login-1003`.

The agent's filesystem read showed stale snapshot/state records from Desktop
`2684`, predating this login. Those records were not used to claim visibility,
PID, shutdown or lock release for this run. Lock-held status was obtained from
the actual renderer; post-Quit lock release was not independently queried in
the ordinary-session runtime view. Process exit and daemon absence are verified.
The separately recorded physical tray run retains its lock-release proof.

## Console defect and follow-up

The operator reported terminal windows opening/closing at login and one remaining
open. A `conhost.exe` process `13736` had daemon `26588` as parent and started
alongside it. It disappeared when that daemon was cleanly stopped. Source
inspection found all three Desktop daemon command sites (status, preflight,
hold) lacking Windows `CREATE_NO_WINDOW`; stdio redirection alone does not
prevent a console allocation. This establishes the daemon's console host; the
individual transient windows were not captured and attributed separately.

A scoped platform command adapter now applies `CREATE_NO_WINDOW` to all three
sites on Windows and preserves ordinary commands on macOS. An isolated Rust
test harness uses the same module and self-spawns a console test executable;
the child queries `GetConsoleWindow` and verifies no console exists while
captured output remains intact. A separate spawn test uses discarded stdout/
stderr, matching the long-lived daemon command. The full Desktop suite passed
11 tests, including both subprocess checks and their shared probe fixture.
Desktop `cargo check`, full Desktop all-target Clippy with warnings denied,
Rust formatting and diff checks passed. Five existing same-file Clippy warnings
(needless borrows and a collapsible condition) were corrected without changing
the lifecycle decision logic.

Local `tauri:package-smoke` also passed: sidecar staging, frontend build and
release Desktop compilation. The prior build block did not recur in this
attempt; no security setting was changed. The old CI package does not contain
the console fix.

The new release launched as Desktop `19640` / daemon `18856`. Desktop SHA-256:
`B13A6085C1306B64FB1126BE39E6415FD05447706BE711A198656C9F9BB87108`.
The visible renderer loaded. A daemon-owned console host was still observable;
console-host presence alone does not prove that a terminal window is visible.
The operator was not watching continuously and could not conclusively verify
absence of a window/flash. UI status invocation also encountered geometry
errors (`coordinate input geometry is unavailable`, then `window bounds changed`)
and was not counted as a successful new-release status/IPC proof.

Cleanup used a verified-owner stop marker, waited for daemon exit, then used
the Desktop `quit.request` test hook; exit 0 was captured. Fresh status showed
no PID/held lock. This is test cleanup, not an additional physical Quit pass.
Both processes are gone and the exact one-shot stop marker was removed. Raw
hash/process/exit data remains outside Git under
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\windows-no-console-1003`.
No workflow was dispatched for this login test or adapter validation.

Automatic Desktop login, hidden main window, tray Open, renderer IPC,
registration removal and clean exit are VERIFIED for the recorded artifact.
Console-free production startup, real installer/update/rollback and ADR-0012
acceptance remain open.
