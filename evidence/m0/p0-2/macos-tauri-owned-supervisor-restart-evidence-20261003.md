# macOS Desktop-owned lifecycle validation

**Status:** PASS for the combined Mac release lifecycle test slices; physical tray Open/Quit verified; cleanup complete

**Source:** `c77479ffb82b2fc697a2f4d16fb3e3a6bd19f4d7`

**Host:** macOS 27.0.1 / arm64, 2026-10-03

## Completed local checks

- `pnpm install --frozen-lockfile`: exit 0.
- `cargo build -p cryptexd --locked`: exit 0.
- `cargo test -p cryptexd --test mode_b_supervisor --locked`: 3 passed, exit 0.
- `pnpm --dir apps/desktop test:sidecar-build-script`: 3 passed, exit 0.
- `pnpm --dir apps/desktop tauri:package-smoke`: exit 0; frontend build passed,
  target `aarch64-apple-darwin` sidecar staged, release Desktop built.
- `cargo test -p cryptex-desktop --bin cryptex-desktop --locked sidecar_resolution_tests`:
  2 passed, 1 filtered out, exit 0.

## Release physical-host checks

The runtime root did not exist before this run. No earlier Cryptex test
Desktop/daemon was active. Release Desktop started without `CRYPTEXD_BIN`.
The visible renderer resolved the sidecar to
`/Users/tarikgungor/github_repos/cryptex/target/release/cryptexd` and the runtime
to `/Users/tarikgungor/Library/Application Support/Cryptex/runtime/p0-2`.

Desktop PID `31422` automatically started daemon `31562`. Renderer status
reported `pid=31562`, `instance=held`, `health=active`; IPC returned
`IPC health.pong · daemon pid=31562 · protocol=1`.

A second release Desktop launch exited 0. Only Desktop `31422` remained,
with one tray menu-bar item and one daemon. The retained snapshot recorded
`note=window_restored_via_single_instance_launch`, `desktop_pid=31422`,
`daemon_pid=31562`, `daemon_pid_observation=live`, `window_visible=true`.

After verifying PID `31562` was the exact repository release daemon with the
specified runtime, the test sent SIGKILL only to that process. The real Desktop
supervisor restarted it; polling observed replacement within 500 ms
(250 ms polling interval, not a precise restart-latency measurement).

```text
supervisor_pid=31422
state=daemon_restarted
daemon_pid=32006
supervisor_alive=true
```

The renderer status reported `pid=32006`, `instance=held`, `health=active`,
and IPC returned `IPC health.pong · daemon pid=32006 · protocol=1`.
The duplicate `lifecycle probe` returned
`cryptexd: daemon instance already running (pid 32006)` with exit 1.

## Login-start registration before sign-in

The renderer initially showed `Start Desktop at login` (disabled). The test
enabled it through that control; renderer reported `Desktop login start enabled`.
`/Users/tarikgungor/Library/LaunchAgents/Cryptex.plist` contains:

```text
Label=Cryptex
ProgramArguments[0]=/Users/tarikgungor/github_repos/cryptex/target/release/cryptex-desktop
ProgramArguments[1]=--cryptex-background
RunAtLoad=true
```

This registration was used for the physical sign-out/sign-in step below. The
registration has since been disabled and removed as part of scoped cleanup.
The run used synthetic lifecycle state; no application code or GitHub Actions
workflow was changed/run. ADR-0012 and P0-2 acceptance remain open.

## Actual sign-out/sign-in observation

The operator confirmed a real sign-out/sign-in. Before any manual Desktop
launch or Open action, live process inspection observed exactly one release
Desktop PID `32654` with `--cryptex-background` and daemon PID `32891` using
the expected runtime. Supervisor state reported `state=active`,
`daemon_pid=32891`, `supervisor_alive=true`. The app had no visible accessible
window and had exactly one menu-bar item in its tray menu bar. The LaunchAgent
still pointed to the release Desktop with the background argument.

The retained `desktop_launched` snapshot recorded `window_visible=true` before
the startup code hides the window; it is not a post-hide visibility snapshot.
The live no-window observation establishes the hidden state after login.

The hidden launch was observed before any manual launch. The native UI tool
could not bind the unbundled release executable by process name or path. The
operator reopened the app through Dock; this confirms Dock reopen, not tray
Open. Renderer IPC was verified after that Dock reopen below.

## Post-login renderer and explicit Stop

The operator reopened via Dock and clicked renderer status/IPC. Live status
showed `pid=32891`, `instance=held`, `health=active`. The renderer accessibility
tree showed `IPC health.pong · daemon pid=32891 · protocol=1`. The retained
snapshot recorded `note=window_restored_via_dock_reopen`; tray Open is therefore
still a separate unverified action.

The operator then clicked `Stop daemon`. Fresh status showed `pid=none`,
`recorded_pid=32891`, `instance=stale`; supervisor state was
`state=daemon_stop_requested`, `daemon_pid=none`, `supervisor_alive=true`.
After an additional measured 10-second wait, the status and state were unchanged
and no release daemon hold process remained. Explicit Stop did not restart the
daemon. Desktop `32654` remained alive.

The Stop observation was completed before login-start removal. The macOS
Dock's Options/Open at Login menu is distinct from Cryptex's own tray menu and
the renderer login-start control; it is not evidence for either Cryptex tray
action.

## Login disable and final cleanup

The agent invoked `Disable Desktop login start` through the renderer.
The UI reported `Desktop login start disabled` and
`/Users/tarikgungor/Library/LaunchAgents/Cryptex.plist` was confirmed absent.
No unrelated login registration was modified.

The agent attempted the actual menu-bar item's accessibility press,
control-click and coordinate right-click. A single menu-bar item remained
accessible, but no `Open Cryptex` / `Quit Cryptex` menu was exposed. The generic
native UI tool also could not bind this unbundled executable. These attempts
do not prove either tray action. The suspected hidden menu-bar position is not
an established application root cause.

To complete cleanup, the renderer's `Quit clean` was invoked instead. Fresh
inspection confirmed Desktop `32654` and daemon `32891` absent, daemon status
`pid=none` / `instance=stale`, and supervisor state:

```text
supervisor_pid=32654
state=clean_quit
daemon_pid=none
supervisor_alive=false
```

The post-login Desktop exit code was not directly captured because it was
launched by the login agent rather than the retained shell session. The normal
Quit state and process disappearance are verified; tray Quit exit 0 remains
unverified. The successful physical login, renderer IPC, login-disable cleanup
and 10-second no-restart-after-Stop checks are retained as separate passes.
The overall test remains **PARTIAL / NOT YET VERIFIED**: actual tray Open and
Quit were not demonstrated. Login startup, renderer IPC after Dock reopen,
Stop-without-restart, login-registration removal and process cleanup are
separate verified results recorded above.

## Focused tray-only follow-up — NOT YET VERIFIED

On 2026-10-03, checkout `924cc139bbba0abd322f1aef6bc174df5864261d`
ran the previously built release artifacts from the validated `c77479f` code.
There were no intervening Desktop/daemon source changes and no build, login,
restart or Stop regression suites were rerun. Login registration remained
absent throughout this follow-up.

Artifact SHA-256:

```text
Desktop: 0bfd52594a4a30bfcd6e37678835bb7d833cfe6be867f44996e45e18de36f1a5
daemon:  bfbfa26c6df1934312ba0d483f5351ac759c8f3f96458f8753d5b4a70157feb4
```

Foreground Desktop `38558` automatically started daemon `38569` under the
expected runtime. Renderer status reported `pid=38569`, `instance=held`,
`health=active`; renderer IPC showed
`IPC health.pong · daemon pid=38569 · protocol=1`.

The native red close control was accessibility-pressed for this follow-up.
The window hid, the same daemon remained active, and `window-close.event`
recorded `source=tauri_close_requested`, `daemon_pid=38569`,
`daemon_pid_observation=live`, `desktop_pid=38558`. The snapshot recorded
`note=window_closed_hidden`, `window_visible=false`.

The enabled tray status item was located at logical display position
`1003,5`, size `36,24`. Native right mouse down/up events were sent at its
center `1021,17`; event-post access was confirmed true. The display bounds
were `1710x1107` logical pixels. No Open/Quit context menu appeared in the
physical display capture or accessibility tree. A subsequent native left
click also left the snapshot `window_closed_hidden` and the window absent.
This establishes failed observation of tray interaction, not a proven cause.
Tray Open and tray Quit remain NOT YET VERIFIED.

Physical display captures were retained locally outside Git:
`/tmp/cryptex-tray-only-before.png` and
`/tmp/cryptex-tray-only-rightclick.png`. They include desktop context and are
not committed. The first shows the visible renderer/IPC; the second shows
the hidden Cryptex window and no tray context menu after the right-click.

Cleanup followed the plan's fallback: a second release launch focused the
existing window through single-instance handling, then renderer `Quit clean`
was invoked. The foreground original Desktop command exited **0**. Fresh
status showed `pid=none`, `recorded_pid=38569`, `instance=stale`; both test
PIDs were absent. Supervisor state recorded `state=clean_quit`,
`daemon_pid=none`, `supervisor_alive=false`. `Cryptex.plist` remained absent.
This exit 0 is renderer cleanup evidence and does not count as tray Quit.

## Physical tray-input diagnosis — PASS, 2026-10-03

Checkout `dfbadedf53d72cde39fd4f4782a061cbc2333980` contains `e26bc0b`.
Desktop/daemon sources were unchanged from the validated `c77479f` build.
Both artifact hashes matched the preceding follow-up exactly:

```text
Desktop: 0bfd52594a4a30bfcd6e37678835bb7d833cfe6be867f44996e45e18de36f1a5
daemon:  bfbfa26c6df1934312ba0d483f5351ac759c8f3f96458f8753d5b4a70157feb4
```

No test processes existed before launch, and login startup remained disabled.
The original Desktop command was retained in a foreground shell to capture
its exit code. Desktop `41780` started daemon `41851`. Renderer status showed
`pid=41851`, `instance=held`, `health=active`; IPC showed
`IPC health.pong · daemon pid=41851 · protocol=1`.

### Icon ownership and physical input

Before/after launch display captures were retained outside Git:
`/tmp/cryptex-tray-diagnostic-before.png` and
`/tmp/cryptex-tray-diagnostic-after.png`. Initially the newly registered
accessible status item at `968,5` (size `36x24` logical points) was not visibly
distinguishable in the menu bar. This comparison alone did not establish a
click target. The display was `1710x1107` logical points, with captures at
`3420x2214` pixels (2x capture scale).

The operator was guided to reveal the menu-bar area, then independently found
the black rounded-square Cryptex icon immediately left of the ChatGPT icon.
The operator supplied display captures of the icon, restored window and actual
two-item menu. Full captures remain outside Git; no private screen content is
included in this record. After the operator's physical input, the accessible
tray item was at `1057,5`, size `36x24` logical points. Its screen position had
changed from the initial observation.

The native close control hid the main window with daemon `41851` unchanged.
An initial accessibility lookup ran before the renderer tree was available;
the initial status/IPC checks were completed after single-instance focus and
the window was closed again. Neither that fallback nor automated close is
counted as a physical tray input result.

The operator physically left-clicked the identified icon and the window
returned. Snapshot:

```text
note=window_restored_via_tray_left_click
desktop_pid=41780
daemon_pid=41851
daemon_pid_observation=live
window_visible=true
```

The operator then physically right-clicked the same icon and selected
**Open Cryptex**. The supplied capture showed the actual Open/Quit menu;
the retained snapshot independently recorded:

```text
note=window_restored_via_tray_menu
desktop_pid=41780
daemon_pid=41851
daemon_pid_observation=live
window_visible=true
```

Fresh lifecycle status still reported PID `41851`, held lock and active
health. The visible renderer retained protocol-1 IPC health for that PID.
No synthetic left/right pointer event was substituted for the operator's
physical input during this diagnostic run. These physical results establish
tray input delivery and successful Open action; the earlier synthetic failures
do not establish an application tray defect.

### Tray Quit with an active daemon

With daemon `41851` still active, the operator selected **Quit Cryptex** from
the actual tray menu and confirmed closure. The retained original foreground
Desktop command exited **0**. Supervisor state recorded:

```text
supervisor_pid=41780
state=clean_quit_from_tray
daemon_pid=none
supervisor_alive=false
```

The snapshot likewise recorded `note=clean_quit_from_tray`,
`daemon_pid_observation=not_running`, `window_visible=false`.
Fresh lifecycle status returned `pid=none`, `recorded_pid=41851`,
`instance=stale` (retained lock file, no held instance lock). Both Desktop and
daemon PIDs were absent. `Cryptex.plist` remained absent.

Physical tray left-click restore, right-click Open, and active-daemon tray
Quit with exit 0 are now VERIFIED. Combined with the preceding release tests,
this closes the Mac lifecycle plan's remaining tray actions. Signed installer,
real Desktop+daemon update/rollback and ADR-0012 acceptance remain separate
gates. No application code, login setting, build suite or GitHub Actions run
was changed/repeated during this focused diagnosis.
