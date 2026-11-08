# Committed import cleanup interruption audit

Date: 2026-11-07. Base revision: `e2dc285`.

## Scope

Test-only interruption hooks were added after staging deletion/parent sync,
prepared-manifest deletion/parent sync, and committed-marker deletion/parent
sync. Production recovery behavior is unchanged. The parent test starts a real
child for each boundary, verifies the named handshake and abrupt exit code 97,
then invokes recovery twice and compares exact canonical bytes and remaining
manifest/marker bytes with the originals.

The fixture is synthetic committed bookkeeping, not a signed native peer stream.
It intentionally isolates final-marker cleanup, whose protocol branch verifies
the prior commit evidence rather than re-admitting under current trust policy.

| Interruption boundary | Fresh recovery result | Canonical bytes |
|---|---|---|
| Staging removed, manifest and marker remain | Typed RecoveryConflict on both attempts | Unchanged |
| Manifest removed, marker remains | Typed RecoveryConflict on both attempts | Unchanged |
| Marker removed, cleanup complete | Success on both attempts | Unchanged |

The first two are expected holds under the current normative decision table;
they are **not a completed durability gate**. No data corruption was observed,
but startup cannot finish cleanup automatically. See the
[proposed receipt amendment](../../docs/proposals/m8-import-cleanup-receipt.md).

## Commands and results

`cargo test --locked -p cryptexd --lib import_staging::tests::xproc_committed_cleanup_interruptions_preserve_events_and_hold_missing_companions -- --exact --test-threads=1`

Passed: **1 test**, containing three abrupt child-process exits and six recovery
attempts. Run on the Windows development host.

`cargo clippy --locked -p cryptexd --lib --bin cryptexd --tests -- -D warnings`

Passed.

`cargo fmt --all -- --check` and `git diff --check` also passed.

The subsequent full import-staging suite command compiled successfully but
**never executed**: Windows Application Control blocked the test executable
with OS error 4551. No protection settings were changed. The earlier 57-test
pass belongs to `e2dc285`, not this change. Do not report the new suite as passed.

These tests do not establish power-loss durability, macOS behavior, native
peer-import functionality or full M8 completion. Startup/publication activation
remains gated pending the protocol decision and remaining integration checks.
