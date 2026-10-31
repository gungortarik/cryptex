# M5 Slice 2 macOS verification

**Date:** 2026-10-10
**Source:** Owner-supplied Mac-host report; not re-executed by this Windows
recording session. Raw terminal logs are not attached to this record.
**Branch:** `codex/m5-capture-timeline`
**Tested HEAD:** `44e686c829837bdfadc7f429a9ad331f01a8502f`
**Product fix:** `07005d93af58f7d2e9881bb62579ba69bb0e7e75`
**Host:** macOS 27.0.1 (26A434), arm64
**Cargo target:** `$HOME/Library/Caches/cryptex-cargo-target-m5`
**Working tree:** clean before and after

The report confirms the expected HEAD and product-fix ancestry.

## Reported checks

| Check | Exit | Result |
|---|---:|---|
| `cargo fmt --all -- --check` | 0 | PASS |
| `cargo check --locked -p cryptexd --lib` | 0 | PASS (non-test build) |
| `capture::` | 0 | 20/20 PASS |
| `capture_restart::` | 0 | 15/15 PASS |
| `secure_store::` | 0 | 14/14 PASS |
| `canonical_writer::` | 0 | 22/22 PASS |
| `projection::` | 0 | 17/17 PASS |
| `clippy -p cryptexd --all-targets -D warnings` | 0 | PASS |

Test filters and abbreviated Clippy invocation are recorded as supplied in
the result report; the preceding checklist specified locked Cargo commands,
`-- --test-threads=1` for tests, and `-- -D warnings` for Clippy.
Do not infer additional flags or execution from these abbreviations.
Windows SecureStore reported 17 tests; macOS reported 14. Platform counts
are retained separately rather than asserting identical test inventories.

## Restart inventory

- 1 worker helper: `xproc_worker_entry`.
- 9 error/restart scenarios: pending before append, append before tip,
  append before committed receipt, committed lost-response replay, partial
  staged receipt, corrupt receipt, concurrent matching IDs, distinct IDs with
  identical text, and durable capture without SQLite.
- 5 abrupt interruption scenarios: pending before append, append before tip,
  staged receipt before placement, committed before response, and append
  before committed receipt.

Total is 15 tests, including the helper; 14 are scenarios. Test-only handshake
and exit before normal capture cleanup establish process-restart behavior.
They are not power-loss or hardware-durability proofs.

## Disposition

Windows results at the product fix are in
`evidence/m5/m5-slice2-review-fix-windows-verification-20261010.md`.
Slice 2 focused Windows/macOS local verification is complete. This does not
close final M5 CI, security/DoD or owner acceptance.

No code changes, Actions, commit/push or merge were performed during the Mac
run. Subsequent evidence recording is documentation only. Production v2 stays
OFF. Desktop capture IPC/UI, timeline and projection wiring remain separately
authorized later slices; M5 is IN PROGRESS.
