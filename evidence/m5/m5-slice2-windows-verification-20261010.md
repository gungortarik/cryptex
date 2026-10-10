# M5 Slice 2 -- Retry/Restart Safety Windows verification

**Date:** 2026-10-10
**Branch:** `codex/m5-capture-timeline`
**Coverage map:** `evidence/m5/m5-slice2-retry-restart-coverage-map-20261010.md`
**CARGO_TARGET_DIR:** `%LOCALAPPDATA%\cryptex-cargo-target`
**Product tip:** `a1645e7a1d7655d346f7df4aa3519b315db93147`

## What was verified

Cross-process restart proofs in `capture_restart` (child test binary, file-backed
SecureStore, env fault points). Process termination is **not** claimed as
power-loss proof. No Desktop IPC/UI, timeline, projection wiring, Actions, or merge.

## Commands executed (Windows)

```text
cargo fmt --all -- --check
cargo test --locked -p cryptexd --lib capture:: -- --test-threads=1
cargo test --locked -p cryptexd --lib capture_restart:: -- --test-threads=1
cargo test --locked -p cryptexd --lib secure_store:: -- --test-threads=1
cargo test --locked -p cryptexd --lib canonical_writer:: -- --test-threads=1
cargo test --locked -p cryptexd --lib projection:: -- --test-threads=1
cargo clippy --locked -p cryptexd --all-targets -- -D warnings
```

## Results

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `capture::` | **20/20 PASS** |
| `capture_restart::` | **10/10 PASS** |
| `secure_store::` | **17/17 PASS** |
| `canonical_writer::` | **22/22 PASS** |
| `projection::` | **17/17 PASS** |
| `clippy --locked -p cryptexd --all-targets -- -D warnings` | PASS |

Verified product SHA: `a1645e7a1d7655d346f7df4aa3519b315db93147`.

## macOS verification (prepared; not executed here)

```bash
git fetch origin codex/m5-capture-timeline
git checkout codex/m5-capture-timeline
git pull --ff-only origin codex/m5-capture-timeline
git merge-base --is-ancestor a1645e7a1d7655d346f7df4aa3519b315db93147 HEAD
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/Library/Caches/cryptex-cargo-target}"
cargo fmt --all -- --check
cargo test --locked -p cryptexd --lib capture:: -- --test-threads=1
cargo test --locked -p cryptexd --lib capture_restart:: -- --test-threads=1
cargo test --locked -p cryptexd --lib secure_store:: -- --test-threads=1
cargo test --locked -p cryptexd --lib canonical_writer:: -- --test-threads=1
cargo test --locked -p cryptexd --lib projection:: -- --test-threads=1
cargo clippy --locked -p cryptexd --all-targets -- -D warnings
```

## Scope / remaining blockers

- Slice 2 local Windows gate recorded; macOS verification still required before
  treating Slice 2 as platform-complete.
- Kickoff items still closed for this slice: Desktop capture IPC/UI, timeline,
  derived catch-up/projection-as-saved wiring, extraction, later slices.
- M5 is not complete.
