# Windows background-command visual check — PASS

Date: 2026-10-03 (America/New_York). Checkout: `73263ca`.
The existing locally built release contains the Windows command adapter from
`3a67f39`. Later source changes were Mac-only helper gating/tests and evidence;
Windows production command code was unchanged. Desktop SHA-256:
`B13A6085C1306B64FB1126BE39E6415FD05447706BE711A198656C9F9BB87108`.
No rebuild, login/reboot or GitHub workflow was needed for this focused run.

The operator explicitly reported ready to watch the screen before launch.
Desktop `14768` automatically started daemon `8968`. Computer Use invoked
renderer **Daemon status**, which displayed `pid=8968`, `instance=held`,
`health=active` and protocol `1`. Renderer **IPC health ping** then displayed
`IPC health.pong · daemon pid=8968 · protocol=1`.

After launch and both queries, the operator confirmed:
**“Hiç terminal/parlama görünmedi”** (no terminal window or flash appeared).
This closes the previously inconclusive visual check for the observed
foreground release launch and status/IPC actions. It does not claim a second
physical login run with the new binary. The earlier real login proof remains
in `windows-desktop-login-result-20261003.md`.

Computer Use invoked renderer **Quit clean** with the daemon active.
The retained process observer captured Desktop exit **0** at
`2026-10-04T02:19:17.5900046Z` (October 3 locally). Both PIDs were absent.
Fresh lifecycle status reported `pid=none`, `recorded_pid=8968`,
`instance=stale`: retained lock file, no held instance lock. The exact
one-shot `stop\n` marker was removed after exit. No product Cryptex startup
entry remained; login was not enabled during this run.

Raw process, artifact-hash and exit records are outside Git at
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\windows-console-visual-1003`.
The adapter subprocess tests, full 11-test Windows Desktop suite, strict
Clippy and local release build passed previously. Mac format/check/strict
Clippy plus its two runtime tests passed at `808bb1c`. Those suites were not
repeated for this visual check.

Result: console-free observed launch/query behavior, new-release renderer
status/IPC and clean shutdown PASS. Real installer delivery, compatible
Desktop+daemon package update/rollback, canonical-data preservation and
ADR-0012 acceptance remain open.
