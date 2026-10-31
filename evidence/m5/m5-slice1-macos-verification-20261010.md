# M5 Slice 1 macOS verification

**Date:** 2026-10-10
**Source:** Owner-supplied Mac-host verification report; not executed again by
the recording Windows session. Raw terminal output is not attached to this record.
**Branch:** `codex/m5-capture-timeline`
**Tested HEAD:** `5d580a654d23f851e6853f0b297a8474ad87b747`
**Product fix:** `d206ea4dc213f56908807a7700c1f6542e14787a`
**Host:** macOS 27.0.1 (26A434), arm64
**Working tree:** clean before and after
**Cargo target:** `$HOME/Library/Caches/cryptex-cargo-target-m5`

The report confirms the expected HEAD and product-fix ancestry
(`git merge-base --is-ancestor` exit 0).

## Reported results

| Command | Exit | Result |
|---|---:|---|
| `cargo fmt --all -- --check` | 0 | PASS |
| `cargo test --locked -p cryptexd --lib capture::` | 0 | 20/20 PASS |
| `cargo test --locked -p cryptexd --lib secure_store::fake::` | 0 | 6/6 PASS |
| `cargo test --locked -p cryptexd --lib canonical_writer::` | 0 | 22/22 PASS |
| `cargo test --locked -p cryptexd --lib projection::` | 0 | 17/17 PASS |
| `cargo clippy --locked -p cryptexd --all-targets -- -D warnings` | 0 | PASS |

Commands are recorded as supplied in the result report. The preceding checklist
specified `-- --test-threads=1` for the test commands; the result table did not
repeat that argument. Counts and exit codes above are the reported results.

## Slice disposition

Windows focused verification at the product fix is recorded in
`evidence/m5/m5-slice1-review-fix-windows-verification-20261010.md`.
Together these records close the Slice 1 Windows/macOS local-verification gate.
They do not constitute final M5 CI or milestone acceptance.

No code changes, app launch, commit, push or Actions were performed during the
reported Mac run. The subsequent evidence recording is documentation only.
Receipt-loss idempotency remains explicitly bounded; canonical notes remain
recoverable without receipts. Strict v1 and production-v2 OFF are preserved.

M5 remains IN PROGRESS. Later slices still need separate authorization; no
Desktop capture IPC/UI, timeline, projection wiring, merge or final completion
is authorized by this record. Classification UX default and capture transport
bounds remain later-slice review items.
