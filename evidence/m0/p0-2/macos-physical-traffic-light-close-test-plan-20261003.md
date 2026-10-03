# macOS physical traffic-light close test — test request

**Status:** REQUESTED; not yet run  
**Scope:** Verify that the actual macOS main-window close control enters the
existing P0-2 `CloseRequested` hide path while Desktop and its test daemon stay
alive. This is evidence for the Mode B candidate only; it does not select
ADR-0012 or prove production tray UX.

## Run on the Mac host

Use the current `m0/p0-2-cursor` branch after pulling the latest origin. Do not
touch any `syncthing-p0-1*` paths or existing login registrations.

```bash
set -euo pipefail
cd "$HOME/github_repos/cryptex"
git fetch origin
git checkout m0/p0-2-cursor
git pull --ff-only origin m0/p0-2-cursor
cargo build -p cryptexd --locked
CRYPTEXD_BIN="$PWD/target/debug/cryptexd" pnpm --filter @cryptex/desktop tauri dev
```

## Interactive steps

1. Wait for the Cryptex window. In the app, click **Start probe daemon** and
   record the reported PID (`daemon_pid_before_close`). If the app reports a
   start error or a pre-existing daemon, stop and report it; do not kill or
   clean up an unknown process/registration.
2. The Mac operator physically clicks the red macOS traffic-light close
   control. Do not use AppleScript or UI automation to synthesize the click.
   Accessibility automation was unavailable in the earlier Mac run.
3. Confirm the main window disappears while the Cryptex Dock icon remains.
   The Mac operator clicks the Dock icon to reopen the window.
4. In the reopened window, click **Daemon status**. Record the PID and confirm
   it equals `daemon_pid_before_close`.
5. Click **Quit clean**. Confirm the app exits and its test daemon releases the
   instance lock.

## Evidence collection and expected result

Runtime root:

```text
~/Library/Application Support/Cryptex-Dev/p0-2/compare/mode-b-desktop
```

After the click, inspect `window-close.event` and `desktop-supervisor.state`.
The close event must identify `source=tauri_close_requested`; supervisor and
daemon must remain alive, and the daemon PID must be unchanged after reopening.
After **Quit clean**, verify the daemon lock is released and no test process is
left running. Record macOS version/architecture, source commit, exact observed
PIDs, event/state excerpts, exit result, and any errors in a new evidence file.

If these observations pass, classify only the physical traffic-light close,
hide, restore, and daemon-survival behavior as VERIFIED for this test build.
Keep production login registration, tray UX, supervision ownership, and
ADR-0012 status open. The runtime remains under `Cryptex-Dev/p0-2`, outside the
repository.
