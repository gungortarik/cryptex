# Windows release Desktop-owned supervisor check

Date: 2026-10-03 EDT. Runtime: `%LOCALAPPDATA%\Cryptex\runtime\p0-2`.
Status: **PARTIAL**. The local release Desktop started and supervised one
daemon, rejected a duplicate Desktop, restarted its daemon after a controlled
crash, and passed the automated Tauri close/restore path. Literal tray
Open/Quit, release login startup, and a current package-smoke rebuild remain
unverified.

## Build provenance and local rebuild block

The release executable used was already present at
`target/release/cryptex-desktop.exe`, SHA-256
`638E469DEE192C94FB5185FE64496BE43F495336CA462FD361E22C8EBD92E709`.
Its source file `apps/desktop/src-tauri/src/main.rs` is byte-identical to the
file at source commit `c77479f`; the executable timestamp is
`2026-10-03T06:33:12Z`, just after that source change. Its sibling
`target/release/cryptexd.exe` SHA-256 is
`A958E52813D5F59561E20E1F80A1D261F3028B9FEFC5FA3DF093FA1589493658`.

The current command `pnpm --filter @cryptex/desktop tauri:package-smoke`
successfully prepared the Windows sidecar and built the frontend, then failed
while compiling the Tauri Rust crate with `E0463: can't find crate for
serde_repr`. A direct `cargo check -p cryptex-desktop --release --locked`
reproduced it. Windows Code Integrity events 3033 and 3077 at 11:51:41–42 EDT
show that policy `0283ac0f-fff1-49ae-ada1-8a933130cad6` blocked `rustc.exe`
from loading the locally generated
`target/release/deps/serde_repr-6db8b116c5f0678c.dll` because it did not meet
the signing-level requirements. The policy was not changed or bypassed. The
existing release executable was used only for the isolated physical checks
below. No GitHub Actions workflow ran; the repository's current CI workflow
does not produce a Desktop release artifact.

## Isolated release run

The product runtime directory did not exist before launch. The run used only
this newly created empty runtime; it did not enable login start or add a
product startup registration.

- Desktop PID `13264` started from the release executable. It launched daemon
  PID `20204` from the sibling `cryptexd.exe`; fresh lifecycle status reported
  `pid=20204`, `instance=held`, `health=active`.
- Second Desktop PID `23592` exited with code `0`. The original Desktop PID
  remained the only Desktop, and one daemon continued to hold the runtime.
- Before crash injection, PID `20204` was checked against the exact executable,
  command line, parent Desktop PID, and empty synthetic runtime. It was then
  force-terminated as the controlled crash input. A first rapid sample caught
  an intermediate child candidate before it held the lock; the following poll
  showed supervisor state `daemon_restarted`, replacement PID `6452`, and
  fresh lifecycle status `instance=held`, `health=active`. Exactly one matching
  daemon remained under Desktop PID `13264`. This proves eventual supervised
  replacement; the first intermediate sample is retained here so the initial
  412 ms candidate observation is not misreported as lock-ready latency.
- The scoped `close.request` test hook entered Tauri's `CloseRequested` handler.
  Snapshot: `window_visible=false`, Desktop `13264`, daemon `6452`,
  `daemon_pid_observation=live`.
- The `show.request` test hook restored the window. Snapshot:
  `window_visible=true`, with the same live daemon PID `6452`.
- The `quit.request` cleanup hook exited the Desktop; no exit code was captured
  by the process wrapper. WMI then found no release Desktop or runtime daemon.
  Fresh status reports `pid=none`, `recorded_pid=6452`, `instance=stale`; the
  stale ready/lock records are retained, but no process holds the lock. The
  exact one-shot `stop` marker was removed. The runtime files remain outside
  the repository for inspection.

The close/show requests are automated hooks, not literal title-bar clicks or
tray menu actions. The cleanup request is not tray Quit. Do not count this
partial run as proof of login startup, IPC command invocation after this
Desktop-owned crash restart, or the tray menu. The earlier Windows renderer
IPC crash/reconnect proof remains separate and valid for its external Mode B
controller.
