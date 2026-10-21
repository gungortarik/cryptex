# M5 Slice 2 -- review-fix Windows verification (cfg gate + abrupt interrupt)

**Date:** 2026-10-10
**Branch:** `codex/m5-capture-timeline`
**CARGO_TARGET_DIR:** `%LOCALAPPDATA%\cryptex-cargo-target`

## Findings addressed

1. `FileBackedSecureStore` module and re-export are `#[cfg(test)]` only.
   Non-test `cargo check -p cryptexd --lib` succeeds; production storage remains
   OS SecureStore.
2. Abrupt-interruption coverage added at durability boundaries via test-only
   handshake + `process::exit(97)` before normal capture cleanup returns.
   Existing typed error/restart cross-process tests are preserved.
   These are **not** power-loss proofs.

## `capture_restart` inventory

| Kind | Count | Notes |
|---|---:|---|
| Worker helper | 1 | `xproc_worker_entry` (child entrypoint only) |
| Error/restart scenarios | 9 | Typed Uncertain/Rejected then fresh-process reconcile |
| Abrupt interruption scenarios | 5 | Handshake then exit at boundary; fresh-process reopen |
| **Module total** | **15** | Helper counted separately from scenarios |

### Abrupt interruption boundaries

- `after_pending_before_append`
- `after_append_before_tip`
- `after_append_before_committed_receipt`
- `after_receipt_stage_before_place`
- `after_committed_before_response`

Fault/interrupt mechanisms remain `cfg(test)` / test env only
(`CRYPTEX_TEST_CAPTURE_FAULT`, `CRYPTEX_TEST_CAPTURE_INTERRUPT`).

## Commands executed (Windows)

```text
cargo fmt --all -- --check
cargo check --locked -p cryptexd --lib
cargo test --locked -p cryptexd --lib capture:: -- --test-threads=1
cargo test --locked -p cryptexd --lib capture_restart:: -- --test-threads=1
cargo test --locked -p cryptexd --lib secure_store:: -- --test-threads=1
cargo test --locked -p cryptexd --lib canonical_writer:: -- --test-threads=1
cargo clippy --locked -p cryptexd --all-targets -- -D warnings
```

## Results

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo check --locked -p cryptexd --lib` (non-test) | PASS |
| `capture::` | **20/20 PASS** |
| `capture_restart::` | **15/15 PASS** (1 helper + 9 error/restart + 5 abrupt) |
| `secure_store::` | **17/17 PASS** |
| `canonical_writer::` | **22/22 PASS** |
| `clippy --locked -p cryptexd --all-targets -- -D warnings` | PASS |

Verified product SHA: 07005d93af58f7d2e9881bb62579ba69bb0e7e75.

## macOS verification (historical prepared checklist)

Owner-reported Mac PASS at exact HEAD `44e686c829837bdfadc7f429a9ad331f01a8502f`
is recorded in `evidence/m5/m5-slice2-macos-verification-20261010.md`.
The checklist below is retained as originally prepared; it was not run on
this Windows host. Slice 2 local platform verification is now complete.

```bash
git fetch origin codex/m5-capture-timeline
git checkout codex/m5-capture-timeline
git pull --ff-only origin codex/m5-capture-timeline
git merge-base --is-ancestor 07005d93af58f7d2e9881bb62579ba69bb0e7e75 HEAD
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/Library/Caches/cryptex-cargo-target}"
cargo fmt --all -- --check
cargo check --locked -p cryptexd --lib
cargo test --locked -p cryptexd --lib capture:: -- --test-threads=1
cargo test --locked -p cryptexd --lib capture_restart:: -- --test-threads=1
cargo test --locked -p cryptexd --lib secure_store:: -- --test-threads=1
cargo test --locked -p cryptexd --lib canonical_writer:: -- --test-threads=1
cargo clippy --locked -p cryptexd --all-targets -- -D warnings
```

## Scope

No Actions, merge, Desktop capture IPC/UI, timeline, or later slices.
M5 is not complete. Process interruption tests are not power-loss proof.
