# M2 Slice 8b — Strict Clippy CI Fix

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`
**Tip:** `1f27e035a421824d2a86c3918ad80bb3afe2401d`

## Failed final CI (before fix)

- GitHub Actions run: [37648931640](https://github.com/gungortarik/cryptex/actions/runs/37648931640)
- Both Windows and macOS readiness failed at **Lint Rust workspace** (`cargo clippy --locked --workspace --all-targets -- -D warnings`) before M2 tests ran.
- Reported `cryptex-events` errors:
  1. `clippy::module_inception` — `mod hex` inside `hex.rs`
  2. `clippy::manual_is_multiple_of` — odd-length hex check
  3. `clippy::collapsible_if` — nested schema_version check in `validate_record.rs`

## Fixes (no lint allows)

- Renamed inner hex helper module to `codec`; use `usize::is_multiple_of(2)`.
- Collapsed unsupported-schema check via `Option::filter`.
- Additional strict-Clippy blockers exposed once events compiled (fixed without allows):
  - `useless_concat` KAT string; schema_gen let-chain; quarantine/`append_memory_note` arity via request structs; needless match / contains / useless `format!` in cryptexd.

## Local verification (Windows)

- `cargo fmt -p cryptex-events -p cryptex-storage -p cryptexd`
- `cargo clippy --locked --workspace --all-targets -- -D warnings` — PASS
- `cargo test --locked -p cryptex-events -p cryptex-storage` — PASS
- `cargo test -p cryptexd --lib canonical_writer --locked` — PASS (10)
- `pnpm --filter @cryptex/schemas check:drift` — PASS

## Non-goals

- Dispatching Actions from this agent (owner dispatches once after review)

## Next

- Owner-reviewed final dual-OS CI on this tip only.
