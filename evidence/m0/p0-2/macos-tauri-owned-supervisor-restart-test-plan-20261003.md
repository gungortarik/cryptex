# macOS Tauri-owned daemon restart test — 2026-10-03

**Status:** PARTIAL / NOT YET VERIFIED overall; login, IPC, Stop and cleanup passed; actual tray Open/Quit remain unverified

See `macos-tauri-owned-supervisor-restart-evidence-20261003.md` for measured
results and continuation.

**Scope:** P0-2; isolated lifecycle runtime and synthetic daemon

## Purpose

Verify the current Tauri Desktop process notices an unexpected exit of the
daemon it started, waits for the single-instance lock to release, starts one
replacement, and can reach it through the renderer IPC health command. Then
verify the explicit Stop action is not mistaken for a crash and does not cause
an automatic restart.

This verifies the release-mode sidecar, login-start adapter, tray behavior and
Desktop-owned restart path. It does not prove signed installer delivery or
production update/rollback. Do not use real canonical/personal data.

## Exact run steps

From the Mac checkout, on branch `m0/p0-2-cursor`:

```sh
git pull --ff-only origin m0/p0-2-cursor
pnpm install --frozen-lockfile
cargo build -p cryptexd --locked
cargo test -p cryptexd --test mode_b_supervisor --locked
pnpm --dir apps/desktop test:sidecar-build-script
pnpm --dir apps/desktop tauri:package-smoke
cargo test -p cryptex-desktop --bin cryptex-desktop --locked sidecar_resolution_tests
./target/release/cryptex-desktop
```

In Cryptex Desktop:

1. Confirm the displayed runtime path is under
   `~/Library/Application Support/Cryptex/runtime/p0-2`. Confirm the daemon
   path field resolves to the sibling `target/release/cryptexd` without setting
   `CRYPTEXD_BIN`. If the path differs or another Cryptex test is using this
   runtime, stop and report that before proceeding.
2. The release Desktop must start or attach to one daemon automatically. Click
   **Daemon status**, then **IPC health ping**. Record the daemon PID shown by
   both controls and the Desktop PID.
3. Launch a second copy of `./target/release/cryptex-desktop`. It must focus the
   existing Desktop, then exit; there must still be one Desktop process, one
   menu-bar icon and one daemon holding the runtime lock.
4. In Terminal, verify that exact daemon PID is the Cryptex daemon running for
   the test runtime. Only then terminate that PID:

   ```sh
   ps -p <DAEMON_PID> -o pid=,command=
   kill -KILL <DAEMON_PID>
   ```

5. Wait at most 20 seconds. Click **Daemon status**, then **IPC health ping**.
   Record the replacement PID and elapsed time. The PID must differ from the
   killed PID; IPC must answer with protocol 1. Confirm exactly one daemon for
   this runtime owns the lock.
6. Check the login-start status. If disabled, click **Start Desktop at login**;
   if already enabled, leave it enabled. Confirm the UI reports enabled. Sign
   out of the macOS account and sign back in. Cryptex must start hidden with a
   menu-bar icon and one active daemon. Use the tray menu's **Open Cryptex**
   item, then confirm **Daemon status** and **IPC health ping** work after login.
7. Disable Desktop login start and confirm the UI reports disabled. Click
   **Stop daemon**. For 10 seconds, verify no replacement starts. Click
   **Daemon status** and confirm `pid=none` / lock not held. Use the tray menu's
   **Quit Cryptex** action and confirm the Desktop exits 0.

## Pass criteria

- The targeted `mode_b_supervisor` integration tests pass.
- The target-aware staging tests pass, and `tauri:package-smoke` builds the
  release Desktop with its `cryptexd` sidecar beside the executable.
- Both Desktop sidecar-resolution unit tests pass on macOS.
- A second Desktop launch focuses the existing process and does not add a
  Desktop process, tray icon or daemon instance.
- After the intentional process kill, the Tauri-owned supervisor starts one
  replacement within 20 seconds and the renderer's IPC ping returns its new PID.
- A duplicate daemon probe is rejected while the replacement holds the lock.
- Physical sign-out/sign-in starts the Desktop hidden, leaves a usable menu-bar
  item, and starts exactly one daemon which answers renderer IPC.
- Disabling login start removes the per-user entry. Explicit Stop leaves the
  runtime stopped for 10 seconds and releases the lock.
- Tray **Quit Cryptex** exits Desktop 0; no GitHub Actions workflow is needed.

If any check fails, preserve the runtime root and logs, report exact output, and
do not classify the gate as PASS. Commit the evidence and handoff update on the
Mac Codex branch; do not modify application code during this evidence run.

## Recorded result for this run

The local checks, second-instance handling, release daemon startup and restart,
duplicate-daemon rejection, real sign-out/sign-in, hidden login launch, Dock
reopen, renderer IPC, explicit Stop with a 10-second no-restart observation,
login-start removal and process cleanup passed. The operator used Dock reopen
to reach the renderer after login; this does not count as tray **Open Cryptex**.

The menu-bar item was present, but its Open/Quit menu could not be exposed by
the available UI controls. Tray **Open Cryptex** and **Quit Cryptex**, including
Quit exit code 0, remain **NOT YET VERIFIED**. The overall test remains partial;
see the evidence file for observations and cleanup details.

## Remaining tray-only follow-up

The previous run removed the login-start registration and shut down all test
processes. Keep login start disabled and run this focused check from the Mac
checkout; no sign-out or reboot is needed:

1. Confirm no Cryptex Desktop or lifecycle daemon process is running. Start
   `./target/release/cryptex-desktop` in a foreground Terminal and keep that
   Terminal open so it can report the Desktop exit code.
2. Confirm one daemon starts under
   `~/Library/Application Support/Cryptex/runtime/p0-2`; record its PID and
   verify **Daemon status** plus **IPC health ping**.
3. Close the main window with its traffic-light close button. Confirm the
   window hides and the same daemon PID remains active.
4. Open the Cryptex menu-bar item's context menu and select **Open Cryptex**.
   Confirm the main window returns, then verify status and IPC still report the
   same daemon PID. This is the tray Open proof; Dock restore does not count.
5. Open the menu-bar item's context menu again and select **Quit Cryptex**.
   Confirm the foreground command exits with code 0, Desktop and daemon
   processes are gone, and fresh lifecycle status reports no live PID/lock.
6. If either menu action is still inaccessible, capture the physical Mac
   display and exact pointer/menu behavior, clean up through renderer **Quit
   clean**, verify no processes remain, and leave that tray gate NOT YET
   VERIFIED. Do not recreate the login-start registration.
