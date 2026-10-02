# Windows login observation (M0 P0-2)

These test-only scripts investigate daemon survival after an actual Windows
sign-out/sign-in. They neither choose supervision for ADR-0012 nor restart the
daemon. All runtime files stay under `%LOCALAPPDATA%\Cryptex-Dev\p0-2`.

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

An observer result alone does not establish that startup came from login. Correlate
the fresh ready timestamp, observer/start time, registered command, and actual
sign-in. An exit code may narrow the cause but does not identify who terminated
the process. These controls do not prove full Desktop IPC reconnect: the current
Desktop reads status via a short-lived CLI process; the P0-3 IPC transport is not
implemented yet.
