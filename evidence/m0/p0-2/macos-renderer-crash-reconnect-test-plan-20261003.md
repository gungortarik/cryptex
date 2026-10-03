# macOS Tauri renderer reconnect during supervised crash — test plan

**Status:** NOT YET VERIFIED
**Scope:** M0 / P0-2 only  
**Required source:** branch `m0/p0-2-cursor` at or after commit `a66d78f`

## Purpose

Verify that the visible Tauri renderer's `Wait for IPC reconnect` command stays
pending while the Mode B comparison harness force-kills and restarts the daemon,
then displays the replacement daemon PID. This isolates the renderer path from
the Windows host's simultaneously active login daemon and its shared named-pipe
endpoint.

This does not verify that the production Desktop itself supervises or restarts
the daemon. The external Mode B harness owns that lifecycle in this test.

## Preconditions and safety

1. Read `AGENTS.md` and `docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md` first.
2. Use the existing `m0/p0-2-cursor` branch. If the Mac checkout is behind
   `a66d78f`, update only a clean checkout with a normal fast-forward; if it is
   dirty or on another branch, stop and report rather than switching or
   rewriting history.
3. Build locally; do not dispatch GitHub Actions for this test.
4. Keep all runtime data outside the repository at:
   `~/Library/Application Support/Cryptex-Dev/p0-2/renderer-crash-wait-20261003`.
5. Inspect for another `cryptexd` using the production IPC endpoint. Do not
   stop or reconfigure an unrelated daemon or LaunchAgent. If it prevents an
   isolated run, record the conflict and stop.
6. Use only synthetic P0-2 state. Do not touch P0-1 Syncthing data.

## Procedure

1. Build the release daemon and comparison harness with the repository's
   pinned Rust toolchain, then build the Tauri Desktop renderer:

   ```sh
   cargo build --locked --release -p cryptexd -p m0-lifecycle-compare
   pnpm --filter @cryptex/desktop tauri build --no-bundle
   ```

   Launch the resulting visible release Desktop executable from
   `target/release/cryptex-desktop`. Use the renderer status text as the
   observation; do not substitute a command-line IPC ping.
2. Open Cryptex Desktop and leave it running without starting its own probe
   daemon.
3. Start the Mode B harness with a 15-second coordination window:

   ```sh
   target/release/m0-lifecycle-compare run-mode-b \
     --experiment-id mac-renderer-crash-wait-20261003 \
     --runtime-root "$HOME/Library/Application Support/Cryptex-Dev/p0-2/renderer-crash-wait-20261003" \
     --cryptexd "$PWD/target/release/cryptexd" \
     --pre-crash-hold-ms 15000
   ```

4. When the harness prints `pre_crash_hold_started_ms=15000`, immediately use
   the visible renderer to click `IPC health ping`, then `Wait for IPC
   reconnect` while the original daemon is still alive. Record the initial PID
   shown in the UI and verify it matches
   `.../mode-b/ready` before the harness removes that control file.
5. Wait for the harness and renderer to finish. Record the renderer's exact
   `IPC reconnected after restart · pid OLD → NEW` text. Compare `NEW` against
   both `restart_success` and the replacement PID in `desktop_reconnect_result`
   in the retained measurement. Require harness exit code 0 and exact PID
   agreement for a PASS.
6. Click `Quit clean` in the renderer and confirm Desktop exits. Preserve the
   measurement file; remove only this test's disposable runtime control files.

## Classification and report

- **PASS:** Renderer command was started before the simulated crash, displayed
  the replacement PID within its 90-second timeout, and that PID exactly matches
  the Mode B measurement. The harness also completed successfully.
- **NOT YET VERIFIED:** No renderer reconnect status appeared within 90 seconds,
  the IDs disagree, an endpoint conflict prevents isolation, or the harness
  failed. Preserve the exact UI status/error, harness output, measurement, OS
  version, architecture, source commit, and both PIDs. Do not relabel a shared
  Rust-client reconnect as a renderer pass.

After running the test, append the measured result to
`macos-tauri-renderer-ipc-reconnect-20261003.md`, update the implementation
handoff and Windows/macOS result index as applicable, and make a small normal
commit and push. Do not accept ADR-0012 or claim P0-2 complete from this test.

## Run record — 2026-10-03 UTC

The release build and visible renderer were run on macOS 27.0.1 / aarch64 at
source `7615b08`. The coordinated Mode B harness completed successfully for
experiment `mac-renderer-crash-wait-20261003-r7`:

- `pre_crash_hold_ms=15000`
- replacement daemon PID: `23330`
- `restart_latency_ms=30`
- shared Desktop IPC measurement: `pid=23314 -> 23330`
- harness exit code: `0`

The renderer was invoked with `IPC health ping` and `Wait for IPC reconnect`
inside the coordination window, but the final visible status remained
`IPC health.pong · daemon pid=23314 · protocol=1`; it did not display the
required reconnect text or replacement PID. This is **NOT YET VERIFIED** for
the renderer crash/restart path. The shared Rust-client reconnect line in the
harness measurement is not counted as renderer evidence.
