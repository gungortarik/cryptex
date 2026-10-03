# macOS Tauri-owned daemon restart test — 2026-10-03

**Status:** REQUESTED; not yet run
**Scope:** P0-2; test-only Cryptex-Dev runtime and synthetic lifecycle daemon

## Purpose

Verify the current Tauri Desktop process notices an unexpected exit of the
daemon it started, waits for the single-instance lock to release, starts one
replacement, and can reach it through the renderer IPC health command. Then
verify the explicit Stop action is not mistaken for a crash and does not cause
an automatic restart.

This does not prove packaged sidecar resolution, login startup, tray behavior,
or production update/rollback. Do not use real canonical/personal data.

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
   `~/Library/Application Support/Cryptex-Dev/p0-2/compare/mode-b-desktop`.
   Confirm the daemon path is this checkout's `target/debug/cryptexd`. If either
   path differs or another Cryptex test is using this runtime, stop and report
   that before proceeding.
2. Confirm the daemon path field resolves to the sibling
   `target/release/cryptexd` without setting `CRYPTEXD_BIN`. Click **Start probe
   daemon**, then **IPC health ping**. Record the daemon PID
   shown by both controls and the Desktop PID.
3. In Terminal, verify that exact daemon PID is the Cryptex daemon running for
   the test runtime. Only then terminate that PID:

   ```sh
   ps -p <DAEMON_PID> -o pid=,command=
   kill -KILL <DAEMON_PID>
   ```

4. Wait at most 20 seconds. Click **Daemon status**, then **IPC health ping**.
   Record the replacement PID and elapsed time. The PID must differ from the
   killed PID; IPC must answer with protocol 1. Confirm exactly one daemon for
   this runtime owns the lock.
5. Click **Stop daemon**. For 10 seconds, verify no replacement starts. Click
   **Daemon status** and confirm `pid=none` / lock not held. Click **Quit clean**.

## Pass criteria

- The targeted `mode_b_supervisor` integration tests pass.
- The target-aware staging tests pass, and `tauri:package-smoke` builds the
  release Desktop with its `cryptexd` sidecar beside the executable.
- Both Desktop sidecar-resolution unit tests pass on macOS.
- After the intentional process kill, the Tauri-owned supervisor starts one
  replacement within 20 seconds and the renderer's IPC ping returns its new PID.
- A duplicate daemon probe is rejected while the replacement holds the lock.
- Explicit Stop leaves the runtime stopped for 10 seconds and releases the lock.
- Desktop exits 0 after **Quit clean**; no GitHub Actions workflow is needed.

If any check fails, preserve the runtime root and logs, report exact output, and
do not classify the gate as PASS. Commit the evidence and handoff update on the
Mac Codex branch; do not modify application code during this evidence run.
