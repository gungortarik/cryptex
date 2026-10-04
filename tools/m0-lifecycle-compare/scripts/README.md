# Windows login observation (M0 P0-2)

## Headless package transaction candidate

For actual copied-daemon health (without launching Desktop), build the lifecycle
tool locally and use `run-package-real-health-proof.mjs` with a fresh outside-repo
`--root`, absolute `--health-helper` and the four baseline/candidate component
path flags. The helper runs `probe-package-health` through the shared Desktop IPC
client. Active/ambiguous endpoints are refused. Three scenarios exercise actual
health, invalid binary rollback and actual protocol mismatch rollback. This is
still a candidate experiment; installed updater/startup integration is absent.
See the real-health evidence and Mac plan in `evidence/m0/p0-2/`.

`package-transaction.mjs` is a separate, test-only serialized filesystem candidate.
It never touches installed Desktop, startup registration or canonical storage.
Run its 29 focused tests with:

```sh
node --test tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs
```

`run-package-transaction-proof.mjs` retains eight scenarios with supplied binary
copies; it requires a fresh outside-repository `--root` and absolute
`--baseline-desktop`, `--baseline-daemon`, `--candidate-desktop`,
`--candidate-daemon` paths. Health is injected, not actual Desktop/daemon execution.
It declares protocol 1 for the positive experiment; incompatibility is a deliberate
metadata negative control. Real runtime negotiation remains a separate gate.

See `docs/adr/ADR-0012-update-orchestration-candidate.md` for the exact recovery
contract and limits and the Mac candidate test plan in `evidence/m0/p0-2/`.
Do not use this engine for concurrent or production installations. Power-loss
durability, signed bundle resources and installed-app handover are not verified.

These test-only scripts investigate daemon survival after an actual Windows
sign-out/sign-in. They neither choose supervision for ADR-0012 nor restart the
daemon. All runtime files stay under `%LOCALAPPDATA%\Cryptex-Dev\p0-2`.

The user's latest external inspection identified the legacy comparison Run
command, not `wl-1003g`, as the occupied registration. To replace that exact
confirmed command and prepare a fresh instrumented experiment from ordinary
Windows PowerShell:

```powershell
powershell.exe -NoProfile -NonInteractive -File 'D:\github_repos\cryptex\tools\m0-lifecycle-compare\scripts\replace-legacy-windows-login.ps1' -Apply -SessionRoot "$env:LOCALAPPDATA\Cryptex-Dev\p0-2\wl-1003h"
```

The helper preserves a JSON backup outside the repository before removing
only the exact legacy daemon command. It requires an empty observer slot and
refuses a live legacy ready PID. On preparation failure it restores the old
command only when both slots are vacant. It retains the old stop/control files.
Without `-Apply` it only validates; a mismatched command is always refused.

## Isolated Explorer Run dispatch check

The 2026-10-02 `wl-1002a` attempt had a confirmed interactive logon but no
Shell-Core launch event or runtime artifact for either Cryptex Run value. Before
repeating the daemon test, isolate Explorer's dispatch using one harmless
PowerShell marker. The marker process writes only its PID, session ID, and UTC
launch time under the supplied runtime directory. It does not start `cryptexd`
or touch canonical data.

After completing the relevant build/tests below, choose a fresh runtime directory
and prepare. Preparation self-checks the marker script without using the registry
and records any existing StartupApproved bytes without modifying that key.

```powershell
$dispatch = Join-Path $env:LOCALAPPDATA 'Cryptex-Dev\p0-2\wd-1003a'
powershell.exe -NoProfile -NonInteractive -File tools/m0-lifecycle-compare/scripts/prepare-windows-run-dispatch.ps1 -Action Prepare -SessionRoot $dispatch
```

Sign out of Windows and sign in normally. Do not use lock/unlock as a substitute.
After sign-in, inspect the marker and Run registration:

```powershell
powershell.exe -NoProfile -NonInteractive -File tools/m0-lifecycle-compare/scripts/prepare-windows-run-dispatch.ps1 -Action Status -SessionRoot $dispatch
```

Status reports the observation time, process bitness, user SID and Windows
session ID. `run_value_present` and `run_value_matches_manifest` distinguish a
missing registry value from a changed command; `run_registered` remains the
manifest-match result for compatibility.

For the next login trial, capture the combined diagnostic evidence instead of
only copying Status. From any PowerShell working directory:

```powershell
powershell.exe -NoProfile -NonInteractive -File 'D:\github_repos\cryptex\tools\m0-lifecycle-compare\scripts\collect-windows-run-dispatch.ps1' -SessionRoot "$env:LOCALAPPDATA\Cryptex-Dev\p0-2\wd-1003a"
```

The collector preserves uniquely named JSON observations outside the repository.
It reads both registry views, this value's StartupApproved bytes, Run policy
values, the current session's Explorer process, execution policy, marker/script
hash and filtered Shell-Core events using explicit UTC boundaries. Unrelated
application launch events are excluded. Collection errors are recorded explicitly.
Marker presence still requires correlation with the real login and timestamps.
The collector neither registers startup nor launches the marker/daemon.

The second real login showed different registry visibility between the user's
ordinary PowerShell and Codex-hosted commands. The collector now records package
identity and process ancestry, and prints `run_matches_both_views`. Prepare the
next fresh `wd-1003b` experiment in the user's ordinary PowerShell; collect its
pre-login state there and require matching registration before signing out.
Do not treat Codex-context visibility as proof that Explorer sees the same value.
Preserve `wd-1003a`; package virtualization is being investigated, not established
as a root cause.

Synthetic controls (no startup registration changes):

```powershell
powershell.exe -NoProfile -NonInteractive -File tools/m0-lifecycle-compare/scripts/test-windows-run-dispatch-collector.ps1
```

If the marker is present, Explorer dispatched this benign Run command, narrowing
the prior failure to the Cryptex entries or their command forms. If absent, collect
the corresponding Shell-Core and process-start evidence before cleanup; do not
infer why dispatch was skipped. Then remove only the matching test value:

```powershell
powershell.exe -NoProfile -NonInteractive -File tools/m0-lifecycle-compare/scripts/prepare-windows-run-dispatch.ps1 -Action Cleanup -SessionRoot $dispatch
```

The manifest and marker evidence remain outside the repository. This probe is
diagnostic only and is not supervision or daemon-survival evidence.

Build and test **before preparing a real login experiment**. The existing Rust
registry integration test uses/removes `CryptexDevP02HkcuRun`; do not run it while
an experiment is armed.

```powershell
cargo build -p cryptexd -p m0-lifecycle-compare --locked
powershell.exe -NoProfile -NonInteractive -File tools/m0-lifecycle-compare/scripts/test-windows-login-observer.ps1
$session = Join-Path $env:LOCALAPPDATA 'Cryptex-Dev\p0-2\wl-1002a'
powershell.exe -NoProfile -NonInteractive -File tools/m0-lifecycle-compare/scripts/prepare-windows-login.ps1 -Action Prepare -SessionRoot $session
```

Use a fresh, short session directory. Preparation refuses existing directories
or occupied test Run values. It copies a fixed daemon binary and observer script
outside the repository and records their SHA-256 hashes plus the source commit
in `login-experiment.json`. A binary copy does not change its signature or trust.

Preparation registers two reversible current-user Run values:

- `CryptexDevP02HkcuRun`: launches `cryptexd lifecycle login-hold` **directly**;
  ready and stop paths are derived from the explicit runtime root.
- `CryptexDevP02LoginObserver`: launches the independent observer, hidden, using
  Windows PowerShell's normal policy. No execution-policy override is used.

The direct daemon entry runs the console `cryptexd.exe`, so Windows may show a
terminal at sign-in while this test registration is armed. That window is
expected test-harness behavior; it is not Cryptex Desktop startup. Run the
scoped Cleanup action after collecting the observation to remove both temporary
Run values and stop the test daemon.

The compact daemon command avoids repeating app-data paths. Registration rejects
commands longer than 260 UTF-16 code units before touching runtime files or the
registry. This follows Microsoft's documented
[Run command limit](https://learn.microsoft.com/en-us/windows/win32/setupapi/run-and-runonce-registry-keys).
Microsoft also documents indeterminate Run entry order and possible startup
delay; separate registration cannot guarantee observer attachment before a very
early daemon exit.

Save open work, sign out of Windows, and sign back in. Merely locking/unlocking
Windows does not establish a login-start proof. Do not launch the daemon manually
in the armed runtime. Leave the observer up to ten minutes; it waits up to three
minutes for attachment and samples daemon liveness every five seconds. If the
daemon exits after attachment, its retained OS process handle records the exit
code even when the process no longer exists.

```powershell
powershell.exe -NoProfile -NonInteractive -File tools/m0-lifecycle-compare/scripts/prepare-windows-login.ps1 -Action Status -SessionRoot $session
```

Inspect `mode-a-hkcu/observations/<id>/observer.jsonl`, `result.json`, original
control-file snapshots, and filtered `code-integrity.json` before running a
status command that rewrites `lifecycle.status`. Expected outcomes distinguish
an observed exit, a daemon alive at the deadline, missing attachment, PID identity
mismatch, and observer error. Failed/missing attachment supplies **no exit code**
and is never labeled survival. Windows denies reads of held lock bytes; that
snapshot error is recorded explicitly and does not stop observation.

The five regression controls cover clean stop, forced termination, alive at
deadline, no process, and an unrelated ready PID. They use isolated synthetic
processes and do not establish a real login/reboot result.

```powershell
powershell.exe -NoProfile -NonInteractive -File tools/m0-lifecycle-compare/scripts/prepare-windows-login.ps1 -Action Cleanup -SessionRoot $session
```

Cleanup checks ownership of both Run values, verifies the ready PID's executable
before stopping it, requests a clean stop, and removes only matching test values.
It leaves evidence files and binary snapshots intact. Changed ownership or failed
clean stop requires inspection rather than deleting a replacement value or
killing an unrelated process. Cleanup is idempotent.

Preparation now retains `preparation.jsonl` with step names, UTC times and
failure diagnostics. Hashes use the framework SHA-256 API rather than depending
on `Get-FileHash` module autoload when launched through Explorer's shell.
The `wl-1003g` attempt was dispatched with `Shell.Application` from Codex.
Its child still read Codex's registry view: an Explorer process in the ancestry
does not prove an unvirtualized caller. No daemon/observer artifacts appeared
after that login, so `wl-1003g` is NOT an armed/valid experiment. Prepare a fresh
experiment directly in the user's ordinary Windows PowerShell and require the
printed manifest before signing out. Retained incomplete attempts are never
overwritten or reused.

An observer result alone does not establish that startup came from login. Correlate
the fresh ready timestamp, observer/start time, registered command, and actual
sign-in. An exit code may narrow the cause but does not identify who terminated
the process. These login controls do not prove Desktop IPC reconnect. The shared
production Desktop Rust IPC client is integrated and has physically reconnected
across supervised daemon restarts on Windows and macOS. Tauri renderer health
and reconnect command invocations have also been physically exercised on both
platforms after clean daemon stop/replacement; unexpected-crash restart while
the renderer waits remains a separate physical check.
