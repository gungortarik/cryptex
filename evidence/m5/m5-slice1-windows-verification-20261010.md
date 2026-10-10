# M5 Slice 1 â€” Windows verification

**Date:** 2026-10-10  
**Branch:** `codex/m5-capture-timeline`  
**Contract:** `evidence/m5/m5-slice1-capture-reconciliation-contract-20261010.md`  
**CARGO_TARGET_DIR:** `%LOCALAPPDATA%\cryptex-cargo-target`

## Commands executed (Windows)

```text
cargo fmt --all
cargo test -p cryptexd --lib capture:: -- --test-threads=1
cargo test -p cryptexd --lib canonical_writer:: -- --test-threads=1
cargo test -p cryptexd --lib projection:: -- --test-threads=1
cargo clippy -p cryptexd --all-targets -- -D warnings
```

## Results

| Check | Result |
|---|---|
| `cargo fmt --all` | applied |
| `capture::` lib tests | **14/14 PASS** |
| `canonical_writer::` lib tests | **22/22 PASS** |
| `projection::` lib tests | **17/17 PASS** |
| `clippy -p cryptexd --all-targets -- -D warnings` | **PASS** |

Verified commit SHA: `4162cc27e983e611b823311967781417235824a7`.

## macOS verification (prepared; not executed here)

Do **not** claim macOS PASS without running these on a Mac host:

```bash
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/Library/Caches/cryptex-cargo-target}"
cargo fmt --all -- --check
cargo test -p cryptexd --lib capture:: -- --test-threads=1
cargo test -p cryptexd --lib canonical_writer:: -- --test-threads=1
cargo test -p cryptexd --lib projection:: -- --test-threads=1
cargo clippy -p cryptexd --all-targets -- -D warnings
```

## Scope confirmation

Slice 1 only: daemon capture/reconciliation library. No Desktop capture IPC/UI,
timeline APIs, projection-as-saved wiring, extraction, Actions, or merge.
M5 is not complete.
