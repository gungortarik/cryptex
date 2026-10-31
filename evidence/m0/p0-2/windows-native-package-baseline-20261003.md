# Windows native NSIS baseline — scoped PASS

Build source: `5015ff1`; installation prepared at `438c978`.
The operator completed the retained unsigned `0.0.0` installer from ordinary
Windows Explorer and launched the application. No GitHub Actions ran.

## Installed identity

Actual-user WMI registry reads returned DisplayVersion `0.0.0` and InstallLocation
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\native-package-proof-1003\installed`.
Both executable version metadata and process paths matched this installation.
The retained installer still hashes to
`7198BAC5639A4D8AE1E4F3AA76B89CFE219A9A9E986CB6C85BFFEBFC9758F425`.

Installed Desktop SHA-256:
`2B91AF7C6D497CA91399F9DD831FFD0759BBFF4F7491476B738260A90F8002A7`.
This differs from the retained loose build, but exhaustive byte comparison found
exactly three differences at zero-based offsets `6882802..6882804`: `UNK` became
`NSS` in `__TAURI_BUNDLE_TYPE_VAR_UNK`. Local dependency source
`tauri-utils-2.10.0/src/platform.rs:350` documents build-time binary patching;
its bundle-type mapping identifies `NSS` as NSIS. Substituting only those three
bytes in memory reproduces the installed hash exactly. The original retained
loose executable and installer were not modified. A separate copy of the actual
installed NSIS Desktop is retained outside Git for rollback comparison.

Installed daemon SHA-256 matches the original manifest:
`A958E52813D5F59561E20E1F80A1D261F3028B9FEFC5FA3DF093FA1589493658`.
The manifest's loose Desktop hashes must not be treated as literal NSIS payload
hashes. Candidate `0.0.1` is expected to hash to
`67F2519AA4EADF24777A109E41D391349DF33945F6C6E40EF0D39E8B9497E5AF`
after the same unique marker substitution; actual installation is still pending.

## Physical installed application

- Desktop PID `26124`, daemon PID `9412`, parent PID `26124`.
- One Desktop and one daemon observed; both execute from the isolated installed
  directory. Renderer sidecar path identifies its sibling `cryptexd.exe`.
- Renderer status: `pid=9412`, `recorded_pid=9412`, `instance=held`,
  `health=active`, `protocol_version=1`.
- Renderer IPC: `health.pong`, daemon PID `9412`, protocol `1`.
- Product runtime: `%LOCALAPPDATA%\Cryptex\runtime\p0-2`.
- Login control remained disabled; actual-user startup enumeration had no
  product `Cryptex` entry. Historical probe registrations were not modified.
- Renderer **Quit clean** exited Desktop with code `0`, captured by an observer
  attached to the exact process handle. Neither executable remained afterward.
  Exit captured at `2026-10-04T02:37:28.9198097Z` (October 3 locally).

Raw exit, observer-context runtime inventory and payload comparison records are
retained under the experiment root outside Git. Because prior ordinary-user
and agent runtime views differed, the agent inventory is not proof of the
operator's current runtime contents or canonical-data preservation. A separate
post-exit lock query in the operator's context was not performed. Process absence
and exit code are verified; do not substitute stale runtime snapshots for that
missing observation.

## Continuation and limits

Run the retained `candidate\Cryptex_0.0.1_x64-setup.exe` from ordinary Explorer
into exactly the same installed directory while no test processes are active.
Then verify installed version, NSIS-adjusted payload hashes, one daemon and
renderer IPC. Quit clean before operator rollback with the retained baseline.
Do not accept forced process shutdown or delete app data.

Candidate replacement and rollback remain pending. This baseline does not prove
atomic updates, incompatible-pair preflight, automatic rollback, signing or
canonical-data safety. ADR-0012 remains unaccepted; no new Mac test is required
by this Windows-only baseline observation.
