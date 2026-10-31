# M3 Slice 3 follow-up - fixture exclusivity + schema shape hardening

**Date:** 2026-10-08  
**Branch:** `m3/local-projection-search`  
**Base tip before fix:** `5a21b8edf9ebe3a09577c9231423946d7059aefc`  
**Parent evidence:** `evidence/m3/m3-slice3-projection-schema-v1-20261008.md`

## Fixes

### Exclusive temp fixture allocator

`exclusive_temp_root` in `crates/projection/src/db.rs` tests now mirrors `services/cryptexd/src/test_temp.rs`:

- process-local `AtomicU64` sequence
- `{prefix}-{pid}-{seq}` names
- exclusive `fs::create_dir` with retry on `AlreadyExists`
- never pre-deletes a candidate path
- `TempRoot` RAII cleanup via `remove_dir_all`; cleanup failures panic (or `eprintln` if already panicking)

File-backed tests explicitly `drop(db)` before `TempRoot` cleanup to avoid Windows SQLite file locks leaving directories behind.

### Schema validation hardening

`validate_schema_v1` now:

- requires `projection_meta`, `events`, and `device_checkpoint` tables
- requires **exact** column sets for those tables (any unexpected column fails closed)
- validates `events_fts` is FTS5 with `content='events'` and user column list exactly `text`

Regression tests:

- unexpected `events.entity_id` column rejected
- missing `events` / `device_checkpoint` rejected
- distinct exclusive temp roots

## Non-goals

Still no ingestion, rebuild, checkpoint advancement, or search APIs.

## Local verification (2026-10-08, Windows)

Host target dir: `%LOCALAPPDATA%\Cryptex-Dev\cargo-target-m3`

- `cargo fmt -p cryptex-projection -- --check` - PASS
- `cargo test -p cryptex-projection --locked` - PASS (13 tests)
- `cargo clippy -p cryptex-projection --all-targets --locked -- -D warnings` - PASS

No GitHub Actions dispatched.

## macOS verification still needed

macOS FTS5/schema smoke on this tip remains **pending** until actually run.
