# M5 Slice 1 -- review-fix Windows verification

**Date:** 2026-10-10
**Branch:** `codex/m5-capture-timeline`
**Contract:** `evidence/m5/m5-slice1-capture-reconciliation-contract-20261010.md`
**CARGO_TARGET_DIR:** `%LOCALAPPDATA%\cryptex-cargo-target`
**Product commit:** `d206ea4dc213f56908807a7700c1f6542e14787a`

## Review findings addressed

1. Pack selection uses verified enrolled frame scan pack paths (no raw
   `event_id` substring match). Regression:
   `pack_lookup_ignores_uuid_in_earlier_note_body`.
2. Capture/resolve validate existing signing identity via
   `load_existing_event_verifying_key` and fail closed on missing/partial/
   mismatched seed or public metadata without SecureStore put or metadata
   mint. Explicit enrollment/writer `ensure` paths unchanged.
3. Receipt durability confirms directory ancestry (including pre-created /
   lock-created dirs), does not treat visible paths or `COMMITTED` state as
   durable without sync, stages PENDING exclusively with cleanup errors
   preserved, and keeps typed sync failures.

## Commands executed (Windows)

```text
cargo fmt -- --check
cargo test -p cryptexd --lib capture::
cargo test -p cryptexd --lib fake::
cargo test -p cryptexd --lib canonical_writer::
cargo test -p cryptexd --lib projection:: -- --test-threads=1
cargo clippy -p cryptexd --lib --tests -- -D warnings
cargo clippy -p cryptexd --all-targets -- -D warnings
```

## Results

| Check | Result |
|---|---|
| `cargo fmt -- --check` | PASS |
| `capture::` lib tests | **20/20 PASS** |
| `fake::` lib tests | **6/6 PASS** |
| `canonical_writer::` lib tests | **22/22 PASS** |
| `projection::` lib tests | **17/17 PASS** |
| `clippy -p cryptexd --lib --tests -- -D warnings` | PASS |
| `clippy -p cryptexd --all-targets -- -D warnings` | PASS |

Verified product SHA: `d206ea4dc213f56908807a7700c1f6542e14787a`.

## macOS verification (historical prepared checklist; subsequently reported PASS)

Owner-reported macOS verification at exact HEAD `5d580a654d23f851e6853f0b297a8474ad87b747`
is recorded in `evidence/m5/m5-slice1-macos-verification-20261010.md`.
The checklist below is retained as originally prepared; it was not run on this
Windows host. Its pending language is superseded by that separate Mac record.

Do **not** claim macOS PASS without running these on a Mac host at this SHA:

```bash
git fetch origin codex/m5-capture-timeline
git checkout codex/m5-capture-timeline
git pull --ff-only origin codex/m5-capture-timeline
git merge-base --is-ancestor d206ea4dc213f56908807a7700c1f6542e14787a HEAD
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/Library/Caches/cryptex-cargo-target}"
cargo fmt --all -- --check
cargo test -p cryptexd --lib capture:: -- --test-threads=1
cargo test -p cryptexd --lib canonical_writer:: -- --test-threads=1
cargo test -p cryptexd --lib projection:: -- --test-threads=1
cargo clippy -p cryptexd --all-targets -- -D warnings
```

## Scope confirmation

Slice 1 review fixes only. No Desktop capture IPC/UI, timeline APIs,
projection-as-saved wiring, extraction, Actions, merge, or later slices.
M5 is not complete. Slice 1 Windows/macOS focused local verification is complete
after the separate Mac report; final M5 CI, DoD and owner acceptance remain open.
