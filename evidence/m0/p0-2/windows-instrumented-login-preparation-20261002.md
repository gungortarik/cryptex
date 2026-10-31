# P0-2 Windows instrumented login preparation

> **Historical status note (2026-10-05):** Status statements below reflect the state when this dated plan/result was recorded, before M0 closeout. Current M0 status is COMPLETE for the synthetic foundation scope; scoped ADR-0012/0004 are ACCEPTED. This note does not expand this artifact's evidence, satisfy production residuals, or authorize M1. See `evidence/m0/m0-closure-audit-20261005.md`.

Date: 2026-10-02. Implementation: `6910f7810e3349cf382f78de80fc182827c9689b`.
Scope: Windows development harness only. ADR-0012 remains unresolved.

## Result and limits

The observer and reversible preparation/cleanup controls are physically verified
on this Windows host. A new actual sign-out/sign-in is **PENDING**. The historical
post-login exit cause remains **OPEN**; no supervision candidate is selected.

The direct HKCU Run daemon command is retained as a direct executable launch.
The separate observer does not launch, restart, or stop it. Run entry ordering
is indeterminate, so a daemon that exits before attachment may still have an
unknown exit code. That case is recorded as missing attachment, never survival.

## Harness fixes

- HKCU Run now uses compact `lifecycle login-hold --runtime-root <path>`.
  It follows the existing hold lifecycle, deriving `ready` and `stop` under the
  explicit runtime root. The previous shape repeated long paths three times.
- The adapter rejects commands over 260 UTF-16 code units **before** clearing
  runtime control files or writing the Run value. Microsoft's documented
  [Run command limit and ordering](https://learn.microsoft.com/en-us/windows/win32/setupapi/run-and-runonce-registry-keys)
  justify this guard. The limit is not evidence explaining a process that had
  already published ready and subsequently disappeared.
- HKCU and native comparison flags share one parser. `leave_registered` is now
  read in Windows builds, removing the unused-field Clippy failure introduced
  by the persistent macOS proof option. Parser tests explicitly supply an
  existing executable instead of depending on a daemon beside Cargo's `deps/`
  test executable. macOS option semantics are retained; Mac execution is not
  claimed by this Windows run.

## Physical observer controls

Command:

```powershell
powershell.exe -NoProfile -NonInteractive -File tools/m0-lifecycle-compare/scripts/test-windows-login-observer.ps1
```

Retained runtime:

```text
C:\Users\Tarik\AppData\Local\Cryptex-Dev\p0-2\observer-controls-20261002T203153362Z
```

| Scenario | Observation | Exit code | Result |
|---|---|---|---|
| stop marker / clean shutdown | `process_exited` | `0` | PASS |
| deliberate termination of synthetic child | `process_exited` | `-1` (`0xFFFFFFFF`) | PASS |
| daemon still alive at observation deadline | `alive_at_observation_deadline` | absent | PASS |
| daemon never started | `not_attached_before_timeout` | absent | PASS |
| ready marker identifies unrelated process | `pid_identity_mismatch` | absent | PASS |

The observer retains an OS process handle and checks executable identity before
attachment. It preserves original daemon status rather than refreshing it via
`lifecycle status`. Reading held lock bytes fails on Windows; that snapshot
failure is explicitly logged while process observation continues. Each run
retains timestamps, heartbeats, control-file snapshots, filtered Code Integrity
events and a final JSON result. An exit code does not identify the terminating
actor.

## Preparation and cleanup controls

Two isolated fixtures were created outside the repository:

- `...\p0-2\wc-1002a`: register, inspect, remove; both Run values removed.
- `...\p0-2\wc-1002b`: register, launch pinned daemon and copied observer using
  their default startup arguments, wait for observer attachment, clean-stop
  through the cleanup script, remove both Run values. The observer captured
  exit `0`; the daemon and observer exited `0`. A repeated cleanup passed.

For `wc-1002b`, the daemon command was 182 code units and the observer command
207. Cleanup left `stop` absent and retained the experiment evidence. These
controls are not actual login evidence.

## Armed real-login experiment

Prepared at `2026-10-02T20:37:34.8186523Z` from a clean source tree at the
implementation commit above:

```text
session_root=C:\Users\Tarik\AppData\Local\Cryptex-Dev\p0-2\wl-1002a
runtime_root=C:\Users\Tarik\AppData\Local\Cryptex-Dev\p0-2\wl-1002a\mode-a-hkcu
daemon_registered=true
observer_registered=true
daemon_command_length=182
observer_command_length=207
ready_present=false
stop_present=false
source_dirty=false
binary_sha256=EDA29D5001CAFCA9091893696E8C46FC4436702152810A9CF764EDD9D0E18DB2
observer_sha256=49B77FCB4570D8013044D74A5C4B39F06F0AFDBA92A842C48A5EB591780AA10E
login_proof=PENDING_REAL_SIGN_OUT_AND_SIGN_IN
```

Both copied-file hashes matched the retained manifest. The daemon binary is
pinned outside Git so later builds cannot replace the registered executable.
Registry identities are `CryptexDevP02HkcuRun` and
`CryptexDevP02LoginObserver` under the current-user Run key. No machine-wide
registration, certificate installation, signing, elevation, execution-policy
override or security-policy change was performed.

The next required physical action is to save open work, sign out of Windows,
and sign in again. Lock/unlock is not a login-start proof. The observer runs for
up to 600 seconds and waits up to 180 seconds for attachment. Inspect its logs
under `runtime_root\observations` before querying status that rewrites daemon
metadata. Correlate with actual sign-in and fresh ready timestamps.

**Do not run the Rust registry tests while the experiment is armed.** They use
the same test-only daemon Run value and would overwrite/remove this preparation.
Use the Status/Cleanup commands in
`tools/m0-lifecycle-compare/scripts/README.md` after collecting the login evidence.

## Validation and remaining work

- `cargo fmt --all -- --check`: PASS.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: PASS.
- `cargo build --workspace --locked`: PASS.
- `cargo test --workspace --locked`: PASS, 52 tests.
- `pnpm --dir apps/desktop build`: PASS.
- Five observer controls, pinned-copy default invocation and idempotent
  preparation cleanup: PASS for the recorded physical control scope.

An earlier Desktop test launch was blocked with OS error 4551. The Code Integrity
log at `2026-10-02 20:12:05 UTC` records IDs 3033/3077 for
`cryptex_desktop-3370d38807265680.exe`, plus Smart App Control detail event 3118.
The later full workspace run executed that test binary successfully. No policy
was changed; the reason for the later allow decision is not established. This
does not prove signing caused the historical daemon disappearance or that the
policy can never block a future development build.

Full Desktop IPC reconnect remains **NOT YET VERIFIED**. Source inspection finds
only `query_daemon_status` launching a short-lived CLI process. No live named-pipe
client/server transport or reconnect handshake exists in the current slice.
That required proof needs the M0 P0-3 transport implementation/prototype; status
observation must not be relabeled IPC reconnect. No IPC implementation/library
or supervision architecture was selected by this work.
