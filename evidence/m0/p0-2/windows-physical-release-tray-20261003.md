# Windows release title-bar close and physical tray — PASS

Date: 2026-10-03 (America/New_York). This follows the partial release
supervisor run and only closes its title-bar/tray actions.

The existing `c77479f` release executable and sibling daemon were reused.
Desktop source blob `fa7c747f27818c419ff3568163f41fd48a220d5c` matched the
current checkout exactly. Artifact hashes remained those recorded in
`windows-release-desktop-owned-supervisor-20261003.md`; no new build or
crash/restart suite was needed for this check.

Desktop `8140` started daemon `22828` under
`%LOCALAPPDATA%\Cryptex\runtime\p0-2`. Computer Use invoked renderer IPC health;
the rendered result was `health.pong`, daemon `22828`, protocol `1`.

Computer Use clicked the native title-bar Close button. After asynchronous
completion, the window was hidden and the snapshot recorded
`note=window_closed_hidden`, Desktop `8140`, live daemon `22828` and
`window_visible=false`. Fresh lifecycle status still showed a held lock and
active daemon. No `close.request` hook was used.

The operator physically selected **Open Cryptex** from the Windows tray menu
and confirmed that the window opened. Independent snapshot and event records
showed `note=window_restored_via_tray_menu`, `source=tray_menu`, visible window
and the same live daemon `22828`. Renderer IPC health was invoked again and
displayed protocol `1` and that PID.

The operator then selected **Quit Cryptex** while the daemon was active,
without invoking Stop first. The retained foreground launch observer captured
Desktop exit code **0** at `2026-10-04T01:38:28.9593847Z` (October 3 locally).
Supervisor and snapshot recorded `clean_quit_from_tray`, no daemon and
`supervisor_alive=false`. Both PIDs were absent. Fresh lifecycle status showed
`pid=none`, `instance=stale`: a retained lock file, no held instance lock.
The exact `stop\n` marker was removed after exit; other runtime evidence was
preserved.

Raw launch/exit JSON and close/Open/Quit records are retained outside Git at
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\windows-tray-1003`. Product login startup was
not enabled during this tray run. The unrelated historical Run-dispatch
experiment is outside this scope.

Windows title-bar close, physical tray Open and active-daemon tray Quit are
VERIFIED for this release executable. Actual Desktop login startup, installer
installation, signing, real package update/rollback and ADR-0012 acceptance
remain open. No additional GitHub workflow was run for this tray proof.
