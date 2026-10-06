# Windows native package replacement — scoped PASS; rollback pending

Build source `5015ff1`; baseline proof recorded at `cb8e935`.
The operator ran the retained unsigned candidate NSIS installer from ordinary
Explorer. Its maintenance page detected an older Cryptex version. The instructed
procedure selected **Do not uninstall** and reused the isolated installed
directory. No Desktop or daemon was running before replacement. Installation
was completed by the operator; installer exit code was not independently captured.

## Installed identity

Actual-user uninstall registration returned version `0.0.1` and the same
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\native-package-proof-1003\installed` path.
Desktop and uninstaller file metadata also reported `0.0.1`.

- Installed Desktop SHA-256:
  `67F2519AA4EADF24777A109E41D391349DF33945F6C6E40EF0D39E8B9497E5AF`.
  This matches the candidate's independently computed NSIS bundle-marker
  adjustment documented in the baseline evidence.
- Installed daemon SHA-256:
  `A958E52813D5F59561E20E1F80A1D261F3028B9FEFC5FA3DF093FA1589493658`.
  The daemon is unchanged between the two compatible packages.

## Physical installed application

The operator launched the updated Desktop from Explorer: Desktop PID `9416`,
parent `22072`; daemon PID `28480`, parent `9416`. One Desktop and one daemon
were observed, both executing from the installed directory. Renderer resolved
the installed sibling daemon and the same product runtime path.

- Renderer status: `pid=28480`, `recorded_pid=28480`, `instance=held`,
  `health=active`, `protocol_version=1`.
- Renderer IPC: `health.pong`, daemon PID `28480`, protocol `1`.
- Login startup remained disabled in the renderer and actual-user startup
  enumeration returned no product `Cryptex` entry.
- Renderer **Quit clean** exited Desktop `9416` with code `0`; an observer
  attached to its exact process handle captured exit at
  `2026-10-04T02:44:13.5398235Z` (October 3 locally).
- Neither Desktop nor daemon remained after exit. No separate operator-context
  post-exit lock query was performed.

Raw candidate observation and exit records are retained outside Git under the
experiment root. No app-data deletion or P0-1 corpus operation was performed.
This does not establish canonical-event preservation: the probe exercised
derived runtime metadata, and ordinary-user versus agent runtime visibility
has known limitations. No GitHub workflow or redundant compile suite ran;
application source was unchanged.

## Next action and acceptance limits

With both processes stopped, run the retained baseline
`baseline\Cryptex_0.0.0_x64-setup.exe`, choose **Do not uninstall**, and reuse
the exact installed directory. Launch it from ordinary Explorer for version,
payload, instance and IPC checks. This is an operator-controlled rollback test.
Stop on security barriers or forced-shutdown prompts; preserve app data.

This run verifies a real compatible `0.0.0 -> 0.0.1` package replacement and
subsequent installed-app health. It does not verify automatic rollback,
interrupted-update atomicity, incompatible-pair preflight, signing, daemon
protocol upgrade or canonical-data safety. Rollback remains pending and
ADR-0012 remains unaccepted. This Windows run does not require repeating the
passed Mac lifecycle or lint tests.
