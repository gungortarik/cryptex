# macOS tray input diagnosis — PASS

The operator's physical left/right input and menu selection verified tray
restore, Open and Quit at checkout `dfbaded`; daemon PID `41851` was preserved
through restore/Open, and active-daemon tray Quit exited 0. See the dated
physical-input section in `macos-tauri-owned-supervisor-restart-evidence-20261003.md`.

Scope: the two remaining P0-2 tray actions. This request follows `e26bc0b`;
it does not repeat login, crash/restart, Stop or build suites already passed.
Keep login startup disabled. Do not change application code during this run.

## Source inspection

The pinned `tauri` 2.12.0 / `tray-icon` 0.25.1 source was inspected on Windows:

- Cryptex attaches a two-item menu and disables its opening on left click.
  Its left-button-up handler restores the main window instead.
- `tray-icon` enables right-click menus by default. Its macOS
  `rightMouseDown:` handler opens the attached menu using `performClick`.
- Tauri registers a clone of the tray object in its resource table. Returning
  from `build_tray` does not by itself destroy that tray object.

These observations establish the intended input mapping, not a physical Mac
pass. The earlier synthetic left/right pointer attempts exposed neither menu
nor restored window. Their accessible item's identity and input delivery to
Cryptex are not independently established. Root cause remains open.

## Request for Mac Codex and the physical operator

1. Verify the checkout contains `e26bc0b`. Reuse the validated release
   executable only if Desktop/daemon source and artifact hashes still match
   the preceding evidence. Otherwise report the mismatch before using it.
   Capture the Desktop command's exit code with a retained foreground process.
2. With no Cryptex test processes running, capture the menu bar before launch.
   Launch the release Desktop, capture it again, and identify the newly added
   status icon by the before/after difference. Record display, scale and icon
   location; an accessible item name alone does not establish ownership.
3. Record Desktop/daemon PIDs and renderer status plus IPC health. Close the
   window using the red close button and confirm the same daemon remains.
4. Ask the human at the Mac to use the physical mouse/trackpad to **right-click
   that identified icon**. Synthetic event permission is insufficient to claim
   a physical click reached it. Capture the resulting menu/display. Select
   **Open Cryptex** if the menu appears; verify the window, status and IPC show
   the same daemon PID, and retain `gui-proof.snapshot` with
   `note=window_restored_via_tray_menu`.
5. If right click does not open the menu, physically left-click the same icon.
   Expected result: the window returns, with snapshot
   `note=window_restored_via_tray_left_click`. Record this independently;
   left-click restore does not satisfy the menu's Open action. Do not repeat
   coordinate automation as a substitute for the unavailable human input.
6. If the menu is accessible, select **Quit Cryptex** with the daemon active.
   Capture Desktop exit code, process absence, lock release and
   `state=clean_quit_from_tray`. Require exit code 0. This also verifies the
   tray action actually invokes shutdown rather than quitting an already
   stopped daemon.
7. If the menu remains inaccessible, restore through the existing
   single-instance focus fallback and use renderer **Quit clean**. Capture
   its exit code and verify processes/held lock are gone and login remains
   disabled. Keep tray Open/Quit **NOT YET VERIFIED**. If no human input is
   available, stop with that limitation rather than repeat the same test.

## Evidence to return

Append a dated section to the existing Mac supervisor evidence and update the
handoff. Include source commit, artifact hashes, PID pair, icon ownership
comparison, whether input was physical or synthetic, the separate right/left
results, snapshot/state notes, captured exit code and cleanup. Keep full desktop
screenshots outside Git; describe relevant observations without private screen
content. Commit/push evidence only. No GitHub Actions run is required.

If physical input still fails, the next implementation step is scoped native
tray event instrumentation to distinguish input delivery from action handling;
do not infer an application root cause or accept ADR-0012 from this diagnosis.
