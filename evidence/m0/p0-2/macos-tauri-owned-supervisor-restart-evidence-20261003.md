# macOS Desktop-owned lifecycle validation

**Status:** PARTIAL / NOT YET VERIFIED overall; tray Open/Quit remain unverified; cleanup complete

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
