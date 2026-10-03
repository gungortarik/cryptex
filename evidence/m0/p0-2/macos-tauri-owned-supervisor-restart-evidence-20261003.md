# macOS Desktop-owned lifecycle validation

**Status:** PARTIAL; physical login and final Stop/tray Quit pending

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

## Physical login continuation

The renderer initially showed `Start Desktop at login` (disabled). The test
enabled it through that control; renderer reported `Desktop login start enabled`.
`/Users/tarikgungor/Library/LaunchAgents/Cryptex.plist` contains:

```text
Label=Cryptex
ProgramArguments[0]=/Users/tarikgungor/github_repos/cryptex/target/release/cryptex-desktop
ProgramArguments[1]=--cryptex-background
RunAtLoad=true
```

The operator must sign out and sign back in. Do not manually launch Desktop
after login: first observe its hidden window, one tray icon and one active
daemon. Then use tray `Open Cryptex` and verify renderer status/IPC. Finally
disable login start, verify removal of the per-user entry, Stop and observe no
restart for 10 seconds, then use tray `Quit Cryptex` and verify cleanup.

The current runtime and registration are deliberately retained for this
pending physical step. This record does not yet classify the entire test as
PASS. No application code or GitHub Actions workflow was changed/run.
ADR-0012 and P0-2 acceptance remain open.
