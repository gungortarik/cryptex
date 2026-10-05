# Windows native handover OS gate — scoped PASS

Source: `80b727fdceab67ca4a9075371a07aca9cccf9654`.
Windows x64; pinned existing Rust toolchain and Node v24.19.0.
Approved isolated M0 pilot; ADR-0012 remains PROPOSED.

## Implementation

`hold-native-handover-gate --root PATH` validates a fresh owned pilot marker
(`m0-native-handover-pilot`) and ordinary outside-repository ancestors. It opens
the persistent `handover-native.lock` without truncation, takes fs2's exclusive
OS lock and reports its PID. Existing ordinary, linked and dangling lock entries
are checked before open. No lock file is removed.

The lock is held until the caller sends `release` over stdin. EOF/invalid control
is abnormal exit 2 and still releases the OS handle. SIGKILL also releases it.
The Node adapter validates acquisition PID, tracks helper closure/abort, requires
clean release acknowledgement and bounds startup/release/cleanup waits. The
operation wrapper retains the gate until its callback completes, including
exception cleanup. A lost lease fails `assertHeld`; cooperating callers must
check it before each phase and after awaits, and honor its abort signal.

This is a gate primitive, not automatic cancellation of arbitrary callbacks.
It does not gate existing Desktop/login processes or existing transaction entry
points until those flows are explicitly integrated. Native recovery uses its
separate existing lock and unchanged experiment marker.

## Verification and diagnostic history

- Initial format check found layout/module-order changes. Scoped formatter
  repair applied; final `cargo fmt --all -- --check` PASS on Windows. The old
  Application Control format block is historical, not observed in this run.
- Final locked Rust tests **14/14 PASS**, strict Clippy PASS, locked build PASS.
- Seven gate process tests PASS: competing startup/handover refusal throughout
  a callback, exception release, SIGKILL plus lost-lease refusal, wrong marker
  and existing/dangling junction refusal, input/repo refusal, persistent lock
  reuse and idempotent release, abnormal EOF release.
- Final combined Node suite **86/86 PASS** (51 + 4 + 6 + 18 + 7), no failure/skip,
  exit 0. Same full command repeated: **86/86 PASS**, no failure/skip, exit 0.
- Two new Node syntax checks and diff checks PASS.

An earlier combined run failed with helper subprocess spawn `UNKNOWN` (errno
-4094). Its root cause was not established. The final source was formatted,
rebuilt and tested successfully twice; no test was skipped or weakened and no
security policy was changed. Repeat output was retained outside Git at
`%TEMP%\cryptex-native-gate-repeat-20261005.log`.

Fixtures/owned helper children were cleaned by tests. No GUI, installed binary,
startup registration, retained corpus or canonical data was modified; no Actions
ran. These tests do not establish signed package identity, power-loss safety,
actual Desktop/daemon handover or adversarial same-user lock-path tamper defense.
Mac OS gate parity remains pending; complete-tree Mac parity is already recorded.
