# M8 Windows transport-path batch staging

**Status:** Implemented and locally verified; staging only.

## Change

`ImportStagingSession::stage_batch_from_transport_paths` accepts a bounded
caller-provided list of relative incoming paths, decodes the DeviceId from the
canonical lowercase-hex directory name and the segment index from the exact
20-digit `.seg` filename, sorts candidates deterministically by DeviceId bytes
and index, rejects malformed paths and duplicate identities, then delegates
each snapshot to the existing bounded no-follow staging implementation.

The method requires an empty, open transaction. Path validation for the whole
batch happens before copying begins. During copying, the existing single-flight,
size, file identity, alias, link-count, snapshot-stability, digest, and
no-replace checks remain in force. A staging error uses the existing whole
unprepared-transaction rollback behavior.

## Verification

- `cargo test -p cryptexd --lib import_staging::tests`: **44 passed**.
- Added tests for canonical path decoding, rejection of uppercase hex,
  malformed/overflow indexes, traversal/extra components, and deterministic
  multi-device batch ordering, pre-parse file-count limits, and duplicate
  identity rejection before copying.
- Full `cargo test -p cryptexd`: library **444 passed, 0 failed, 4 ignored**;
  binary **12/12**, lifecycle **9/9**, supervisor **3/3**, rollback **2/2**.
  One initial full run had a transient existing child-process cleanup test
  failure; that test passed alone, and the complete package retry passed.
- `cargo clippy -p cryptexd --lib --tests --release -- -D warnings`, fmt check,
  and `git diff --check`: PASS.

## Limits

This is not a transport controller or directory scanner. No Syncthing event or
poller invokes the new method. Staged candidates remain untrusted: semantic
verification, local trust/cutoff and `RetrievalSafetyState` gates are not wired
to publication. No canonical destination, committed marker, or transport
activation is enabled. The method does not change ADR-0019 proposal status or
the owner-approved Windows §5.1.1 scope.

## Rollback regression follow-up

Added a synthetic regression test where the second sorted transport candidate
is missing after the first candidate has already been copied. The test confirms
that the batch is marked failed, the earlier staged transaction directory is
removed, and the original transport source remains available for retry.

- `cargo test -p cryptexd --lib import_staging::tests::transport_batch_failure_rolls_back_earlier_staged_candidates`: **1/1 PASS**.
- `cargo test -p cryptexd --lib import_staging::tests`: **45/45 PASS**.
- `cargo clippy -p cryptexd --lib --tests --release -- -D warnings`: PASS.
- `cargo fmt --all -- --check`: PASS after formatting.
- `git diff --check`: PASS.

The full daemon package suite was not rerun after this test-only addition. The
staging API remains unconnected to a production Syncthing caller; no trust,
publication, marker completion, or transport activation is enabled.

## Full daemon suite refresh (2026-10-29)

After the rollback regression test was added, `cargo test -p cryptexd` passed on
Windows:

- Library: **445 passed, 0 failed, 4 ignored**.
- Binary: **12/12**.
- Lifecycle: **9/9**.
- Supervisor: **3/3**.
- Update rollback integration: **2/2**.

This verifies the current daemon package tree. It does not close the absent
production transport caller, canonical publication/replay, trust/cutoff,
`RetrievalSafetyState`, approval IPC, macOS parity, or independent protocol
review gates.
