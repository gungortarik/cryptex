# Windows owned Desktop session pilot — runtime scoped PASS

Implementation: `dd5021bb04ccc698b66de51f739ab60053997534`.
Windows x64; Node v24.19.0; existing pinned Rust toolchain.
ADR-0012 remains PROPOSED. This is same-package baseline reopen, not an update.

## Implementation and local checks

Desktop accepts `--cryptex-pilot-runtime` only for a fresh outside-repository
ordinary directory containing exactly the fixed versioned pilot marker. It
validates before Tauri/daemon startup, uses a separate single-instance identifier,
sets initial windows invisible, and resolves the bundled sidecar without the
environment override. Pilot login controls cannot alter product autostart.
Pilot clean quit follows the existing supervised stop path and rejects a
non-successful owned daemon exit. Ordinary product runtime defaults are retained.

The runner holds the native handover gate through both sessions. Each session
uses a new runtime, requires the completed `desktop_launched` snapshot, checks
Desktop/daemon PID plus OS parent/executable identity, performs real shared IPC
health, and stops through `pilot-quit.request`. Clean exit 0, daemon PID absence,
lock release and absent IPC endpoint must all pass before reopening.
No installed application or existing login registration is changed.

- Combined Node tests **97/97 PASS** (51 + 4 + 6 + 18 + 7 + 7 + 4), zero failures/
  skips, exit 0. Four CLI tests use the current-source local release Desktop and
  reject invalid arguments, wrong/reused markers, actual junction roots and
  repository/relative roots before Tauri startup.
- Format, strict Desktop Clippy, frontend build, locked release Desktop+daemon
  build, five new Node syntax checks and diff checks PASS.
- Initial Desktop test compilation needed an explicit Tauri context type;
  repaired. The local debug unit-test executable then could not execute:
  Application Control error **4551**. No local Rust Desktop test PASS is claimed
  and no policy/blocked executable was modified or bypassed.
- Required independent Windows release unit checks/package build were dispatched
  once with existing `windows_desktop_build_only=true`: run **37308202823**, source
  `dd5021b`. Other workflow jobs were skipped. Result is pending until appended;
  a CI unit result does not replace the physical-host session evidence below.

## Real Windows session proof

Fresh source copies:
`C:\Users\Tarik\AppData\Local\Cryptex-Dev\p0-2\desktop-session-input-20261005-3efd2eb3a25c42179ddcb6d878e1cdc8`.
These are newly built loose release executables with embedded frontend resources;
not a new NSIS installation and not a signed publisher package.

Retained passing proof:
`C:\Users\Tarik\AppData\Local\Cryptex-Dev\p0-2\desktop-session-proof-20261005-08ded4de10b04fdabb73dfc44501ea1c`.

Report `desktop-session-report.json` SHA-256:
`f4ac336ca0dfe8676d1e89eb84e944590d8b24c6aac762974340aea368ebec72`.

| Session | Desktop PID | Daemon PID | Daemon OS parent | IPC | Cleanup |
| --- | --- | --- | --- | --- | --- |
| Baseline first | 33552 | 3364 | 33552 | health.pong, protocol 1, lock held | exit 0, PID gone, lock released, endpoint absent |
| Same baseline reopen | 460 | 10924 | 460 | health.pong, protocol 1, lock held | exit 0, PID gone, lock released, endpoint absent |

Both Desktop OS parents were the owned Node runner PID 1440. Executable identities
matched the staged package paths, not the installed application. Final startup
snapshots showed `window_visible=false`. This is snapshot/configuration evidence,
not an operator's physical no-flash observation. No focus/GUI clicks were used.

Desktop SHA-256:
`26556b6c9255911a81d0faf2481a83138efdc05c18698b07a86eecd700609edd`.
Daemon SHA-256:
`0abb57a0e88a3659e181df9988027cf4c7fe040e399da316c7bbd8a62f44c72c`.
Source/staged package hashes were verified; only owned runtimes contain new data.
No Desktop/daemon process remained after the proof.

## Diagnostic history and limits

Two initial runtime attempts rejected the intermediate daemon-start snapshot;
one diagnostic recorded matching PIDs but `window_visible=true` before the final
Desktop launch snapshot. The harness incorrectly treated any snapshot as final
readiness. It now waits for a complete `desktop_launched` event, still requiring
hidden-window/PID/OS-identity/IPC checks. A new regression rejects intermediate
and partial snapshot writes. Retained attempts:
`desktop-session-proof-20261005-48ace11e4af74123b4c8a76d5ec8a503` and
`desktop-session-proof-20261005-24af568c93b44f76a3ed31dc3dcf901d` under the same
outside-repository P0-2 base. Each owned session was cleaned; no failure was
reclassified as PASS.

No candidate slot was activated and no transaction journal or rollback was
integrated. Real version-changing update, candidate failure/baseline restart,
interruption recovery during handover, signatures, canonical data and power-loss
durability remain open. Pilot validation currently requires the compiling source
checkout to be present; this test tool is not a relocatable shipped launcher.
Mac actual `.app` session parity remains pending.
