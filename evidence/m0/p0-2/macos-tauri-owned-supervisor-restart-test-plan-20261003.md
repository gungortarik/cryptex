# macOS Tauri-owned daemon restart test — 2026-10-03

**Status:** PARTIAL; local checks and release crash/restart passed; physical login and final Stop/tray Quit pending

See `macos-tauri-owned-supervisor-restart-evidence-20261003.md` for measured results and continuation.
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
