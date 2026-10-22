# M8 owned-process foundation — Windows verification

**Date:** 2026-10-20  
**Source:** uncommitted `codex/m8-device-trust`, base
`74bc3b14127962579ae03838bafea9336612a033`. No new product SHA claimed.  
**Scope:** Windows owned process tree and explicit synthetic native transport proof.

## Implementation

Added `owned_process/mod.rs`, the isolated Win32 adapter and synthetic worker
tests. Atomic process/job association uses the existing windows-sys 0.61.2 pin
with JobObjects enabled. Explicit handle inheritance permits NUL stdio only.
Cleanup targets retained owned job/process handles, not arbitrary PIDs or names.

Role preparation has fixed Syncthing flags, a minimal environment and bounded
UTF-16 quoting. Polling does not call a root-only exit a completed tree; timeout
does not imply cancellation; forced cleanup cannot become clean success.
Tests cover unrelated-job preservation, remaining descendants, owner Drop,
owner `exit(97)` without destructors and external termination of the owner alone.

Added the test-only PowerShell native fixture preparer. It establishes a
protected current-user-only DACL on its fresh app-data fixture before native
generation, prohibits external XML resolution and prepares loopback-only
configuration without shared folders, discovery/relay/NAT/upgrade/usage reporting.
This script is NOT the product profile or OS credential provisioner.

Contract and limits: `m8-owned-process-boundary-contract-20261020.md`.

## Final checks

Windows host, pinned Rust 1.98.1; target:
`%LOCALAPPDATA%\Cryptex-Dev\build\m8-device-trust-target`.

| Check | Observed result |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo test --locked -p cryptexd -p cryptex-events` | exit 0 |
| Event library | 61/61 PASS, golden/strict-schema cases retained |
| Event schema-generator target | executed successfully; 0 tests |
| Daemon library | 321 PASS, 0 FAIL, 1 explicit native-fixture test excluded by default |
| Daemon CLI | 8/8 PASS |
| Lifecycle integration | 9/9 PASS |
| Mode B integration | 3/3 PASS |
| Update/rollback integration | 2/2 PASS |
| Doc tests | both targets executed successfully; 0 tests |
| `cargo clippy --locked -p cryptexd --all-targets -- -D warnings` | exit 0 |
| `cargo build --locked -p cryptexd --bin cryptexd` | exit 0 |
| Explicit `owned_process::tests::native_syncthing_lifecycle_fixture --ignored --exact --test-threads=1` | 1/1 PASS, two native startup/shutdown cycles |
| PowerShell preparer syntax and actual fixture preparation | PASS |
| `git diff --check` | PASS |

The new ordinary owned-process entries are 16/16, including a worker entry.
The native integration is reported separately: it was actually selected and
executed with its explicit protected fixture, not merely counted as an ignored
test in the general run.

Final daemon SHA-256:
`4b0c0ff217bc383418b382d5b6c166df62f5c91f892b2f3877267749fdda4186`.

Full final logs, copied native result and the 12-file LF-normalized source
manifest are under:
`%LOCALAPPDATA%\Cryptex-Dev\m8-verification\owned-process-verified-20261020-9e474d7126d74046b1cc1441cfcd2810`.
Repository manifest: `m8-owned-process-source-sha256-20261020.txt`.

## Actual native observations

Retained native input SHA-256:
`36a0f7bc372f64fa7cc4f5654fa324c0dd9f7fef2e07565e00c6e1cf73f50344`,
matching the previously recorded unmodified Windows Syncthing 2.1.5 M0 package.
No new native binary was downloaded or published.

Private synthetic fixture:
`%LOCALAPPDATA%\Cryptex-Dev\m8-verification\m8-owned-native-acc91dac2e3b43088ed3548c7523f78c`.

| Cycle | Owned native PID | Exit | Forced | Incorrect API key |
|---|---:|---:|---|---:|
| 1 | 36984 | 0 | false | rejected 403 |
| Retained-profile restart | 24028 | 0 | false | rejected 403 |

Both cycles verified expected version/native identity through the bounded REST
adapter and inspected live configuration: no shared folders, loopback-only API/
peer listener, discovery/relay/NAT disabled, automatic upgrade disabled, reporting
disabled and insecure admin access disabled. Both shutdowns completed with no
active job members; no native database-stop failure was logged. API keys did not
appear in generation or native logs. A post-run snapshot found no Syncthing
process. No user-global profile or personal canonical memory was used. This
does not claim a before/after digest audit of every user-global configuration.

The native key used FakeSecureStore within this synthetic test. This is not
product Windows Credential Manager provisioning or publisher authenticity.
Native private certificate/config files remain only in the protected fixture.

## Failures preserved and resolved

- First focused process run: 14 PASS / 1 FAIL. The test worker incorrectly
  attempted to parse the literal `exit-with-tree` mode as a numeric exit code,
  so the intended scenario never ran. Its dispatcher was corrected; the final
  suite exercises the original assertion without weakening it.
- Strict lint caught the intentionally surviving fixture descendant's early
  return. The scenario now uses an explicit root `exit(0)` to model the actual
  root-exited/descendant-alive condition. No lint allow or assertion was removed.
- A parallel package run exposed the prior REST fixture's Windows accepted
  socket retaining nonblocking mode. The fixture explicitly selects bounded
  blocking header reads; production client policy is unchanged. Both isolated
  proxy and full-package tests now pass. PID/ready fixture publication is also
  atomic so readers cannot observe partially written PIDs under parallel load.
- Intermediate generated test launches were refused by Application Control
  4551. The final normal Cargo invocation executed both the current daemon
  and previously blocked schema-generator target successfully. No security
  setting, renamed/copied executable or alternate loader was used. This closes
  that exact candidate's observed launcher check, not Application Control in
  general. No inference is made about why its decision differed between runs.

## Remaining M8 scope

This is a tested ownership foundation, not the integrated product controller.
No daemon startup/IPC path launches it by default. Package/profile/permissions
validation, production secure-store provisioning, endpoint ownership binding,
authenticated lifecycle control and controller integration remain required.
Non-Windows spawn is explicitly unsupported until its consolidated Mac stage;
no cross-platform supervisor completion is claimed.

Pairing/bootstrap, versioned durable security events, issuer-authority review,
revocation/cutoff integration, incoming validation and offline convergence remain
open under ADR-0019 §8. Current diagnostic protection/controller fields remain
unconfigured/unimplemented. M7 write-back remains disabled. No Actions, commit,
push, merge or M8-complete claim.
