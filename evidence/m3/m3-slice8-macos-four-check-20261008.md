# M3 Slice 8 — Owner macOS four-check PASS

**Date:** 2026-10-08
**Branch:** `m3/local-projection-search`
**Tested HEAD:** `275fb7acf0c2c7b4d7c050acb54670e81520377f`
**Host:** macOS 27.0.1 (26A434), arm64
**Working tree:** clean

Owner-provided results for this exact SHA:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo test --locked -p cryptex-projection --lib` | 45 passed; exit 0 |
| `cargo test --locked -p cryptexd --lib projection::` | 17 passed; exit 0 |
| `cargo clippy --locked -p cryptex-core -p cryptex-projection -p cryptexd --all-targets -- -D warnings` | exit 0 |

No source changes were made on the Mac. This records local platform verification only; the final dual-OS workflow is recorded separately. M3 completion and merge are not claimed.

## Corruption-class fix candidate — PASS

**Tested HEAD:** `4a2ea56aa2ddc4024d82599f8dc671472179cbe8`

**Host:** macOS 27.0.1 (26A434), arm64

**Working tree:** clean

Owner-provided exact-tip results:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo test --locked -p cryptex-projection --lib` | 47 passed; exit 0 |
| `cargo test --locked -p cryptexd --lib projection::` | 17 passed; exit 0 |
| `cargo clippy --locked -p cryptex-core -p cryptex-projection -p cryptexd --all-targets -- -D warnings` | exit 0 |

This is the exact docs-stamped branch HEAD containing code fix `6b61882`.
The final dual-OS run at this HEAD is `37780668640`; see
`evidence/m3/m3-final-ci-gate-20261008.md`. M3 completion and merge are not
claimed.
