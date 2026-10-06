# Windows reboot/login survival repeat

Date: 2026-10-03 EDT. Experiment: `wl-1003h`.
Status: **PASS** for test-only HKCU Run startup and 600-second survival after a
second physical Windows reboot/login. The test registrations were then removed;
the pinned runtime and observation artifacts remain outside the repository.

## Why a terminal appeared

The user-provided screenshot shows `cryptexd.exe lifecycle login-hold` output
for PID `26576` and runtime
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\wl-1003h\mode-a-hkcu`. The matching manifest
and actual user's HKCU Run command show that `CryptexDevP02HkcuRun` launches
the console daemon executable directly. Windows therefore displayed its
console at sign-in. This was the Mode A comparison harness, not the Cryptex
Desktop application or its proposed Mode B login flow.

The hidden `CryptexDevP02LoginObserver` Run value launched the independent
observer. Both Run values and command strings matched `login-experiment.json`
under the actual user SID. The pinned daemon's SHA-256 matched the manifest:
`EDA29D5001CAFCA9091893696E8C46FC4436702152810A9CF764EDD9D0E18DB2`.

## Physical login and survival

Windows reported boot at `2026-10-03 11:25:40 EDT`; the interactive console
session logged in at `11:26 AM EDT`. The daemon was created at
`11:26:36.275 EDT` as PID `26576`, with Explorer PID `13424` as parent. Its
executable and full command line matched the pinned `wl-1003h` manifest. The
observer was PID `26356`. Shell-Core event 9708 records Run execution completion
for both processes at `11:26:37`/`11:26:38 EDT`; that event is corroborating
launch evidence, not evidence of process exit.

Observer result:

- Observation: `observations/20261003T152637877Z-2222a2db/result.json`
- Started: `2026-10-03T15:26:37.8776824Z`
- Ended: `2026-10-03T15:36:39.1288109Z` (about 601 seconds)
- Outcome: `alive_at_observation_deadline`; `exit_code=null`
- Final heartbeat: attached to the same process; no stop marker
- Code Integrity matches: 0
- Ready PID remained `26576`; the lock-file snapshot was correctly reported
  unavailable while held

Together, the operator-confirmed reboot, fresh interactive logon, exact Run
commands, process identity/ancestry, Shell-Core records, and attached observer
establish test-only login startup and measured ten-minute survival. This does
not demonstrate crash restart, Desktop IPC, production startup, or indefinite
uptime. The observer's `login_proof` field correctly remains
`NOT_ESTABLISHED_BY_OBSERVER_ALONE`.

## Cleanup after the observation

Both Run values were re-read through `StdRegProv` under the actual user's SID
and compared byte-for-byte with the manifest before removal. Their exact values
were saved to
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\wl-1003h\registration-cleanup\run-values-before-cleanup.json`.
No unrelated startup registration was changed.

A cooperative `stop` marker was written for PID `26576`. A subsequent fresh
lifecycle status reported `pid=none`; WMI found no matching daemon process, and
`Win32_StartupCommand` showed neither test Run value. The exact stop marker was
removed after confirming the process was gone. Lifecycle status reports
`instance=stale` because the retained `daemon.lock` file still records the old
PID; no process holds the instance lock. The post-observation stop exit code was
not captured. The comparison cleanup path also retains runtime artifacts.
Evidence, binary, manifest and backup remain under the LocalAppData experiment
root; no runtime data was added to the repo.

The PowerShell cleanup script's .NET `HKCU` view in the Codex process did not
match the actual user's Run view, so it was not used to mutate startup state.
The actual SID-scoped registry provider was used after exact ownership checks.
No process was force-killed. GitHub Actions was not run.
