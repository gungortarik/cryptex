# Windows native package operator rollback — scoped PASS

> **Historical status note (2026-10-05):** Status statements below reflect the state when this dated plan/result was recorded, before M0 closeout. Current M0 status is COMPLETE for the synthetic foundation scope; scoped ADR-0012/0004 are ACCEPTED. This note does not expand this artifact's evidence, satisfy production residuals, or authorize M1. See `evidence/m0/m0-closure-audit-20261005.md`.

Build source `5015ff1`; prior installed baseline and update results are recorded
at `cb8e935` and `1e2be11`. The operator completed retained baseline `0.0.0`
reinstallation after the candidate's clean exit. The instructed maintenance
option was **Do not uninstall**, into the same isolated install directory.
The operator supplied a running application screenshot after installation.
Installer exit code and radio-button selection were not independently captured.

## Restored payload and registration

Actual-user WMI uninstall registration again reports DisplayVersion `0.0.0`
and InstallLocation
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\native-package-proof-1003\installed`.
Desktop and uninstaller version metadata also report `0.0.0`.

- Desktop SHA-256:
  `2B91AF7C6D497CA91399F9DD831FFD0759BBFF4F7491476B738260A90F8002A7`.
  Exact match with the actual NSIS payload from the first baseline installation.
- Daemon SHA-256:
  `A958E52813D5F59561E20E1F80A1D261F3028B9FEFC5FA3DF093FA1589493658`.
  Exact match across baseline, candidate and rollback.
- Uninstaller SHA-256:
  `FBF7289095CE78700D88C8B327A667672CF0E99AB34C63981BD67E3265997EAD`.
  Exact match with the first baseline installation.

## Physical application check

Desktop PID `25436`, daemon PID `15340` with parent `25436`; both process
executable paths are in the installed directory. One Desktop and one daemon
were observed. The renderer resolves the installed sibling daemon and uses
`%LOCALAPPDATA%\Cryptex\runtime\p0-2`.

- Renderer IPC: `health.pong`, daemon PID `15340`, protocol `1`.
- Renderer status: `pid=15340`, `recorded_pid=15340`, `instance=held`,
  `health=active`, `protocol_version=1`.
- Login control remained **Start Desktop at login** and actual-user startup
  enumeration contained zero product `Cryptex` entries.
- Renderer **Quit clean** exited Desktop with code `0`, captured by an observer
  attached to the exact process handle at `2026-10-04T02:47:33.0414730Z`
  (October 3 locally). Neither executable remained afterward.

Raw rollback observation and exact-process exit records remain outside Git under
the experiment root. Retained package artifacts and runtime observations were
preserved. No app-data deletion, P0-1 corpus operation or GitHub workflow ran.
Application source is unchanged; no redundant compile suite was run.

## Result and remaining gates

The real installed sequence `0.0.0 -> 0.0.1 -> 0.0.0` passes package identity,
installed-app health and clean process exit at each stage. This is a compatible
Desktop version replacement with the same protocol-1 daemon bundled in both
packages, followed by operator-controlled rollback. The final installed version
is `0.0.0`, stopped, with product login startup disabled.

Automatic rollback, interrupted replacement safety, incompatible-pair package
preflight, signing and actual canonical-event preservation remain open. No
operator-context post-exit lock query was captured. Known runtime-view separation
means the agent's inventory cannot establish canonical-data safety. P0-2 and
ADR-0012 are not complete. Next work is a scoped update-orchestration proposal
with explicit failure-path and data-preservation acceptance criteria before
implementing package transaction behavior. Do not repeat the passed Mac
login/tray/lint suites in response to this Windows-only result.
