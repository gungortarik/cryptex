# M3 Slice 5 follow-up - Replace only explicitly unusable projection DBs

**Date:** 2026-10-08  
**Branch:** `m3/local-projection-search`  
**Base tip before follow-up:** `ff212aaf75f5c78be10293c72b174eede6c782c2`  
**Parent evidence:** `evidence/m3/m3-slice5-rebuild-incremental-checkpoint-20261008.md`

## Problem

`ProjectionDb::open_replacing_unusable` treated every `try_open_existing` failure as grounds to delete the disposable DB and sidecars. Transient SQLite/I/O failures or `Fts5Unavailable` could destroy a healthy projection. Arbitrary open failures were also folded into `CorruptDb` via a catch-all replace reason.

## Fix

- Replace only for:
  - `ProjectionError::CorruptDb` (including `quick_check` failure and SQLite `NotADatabase` / `DatabaseCorrupt`)
  - `ProjectionError::UnsupportedSchema`
  - `ProjectionError::SchemaInconsistent`
- Propagate without deletion: `ProjectionError::Io`, ordinary `ProjectionError::Sqlite` (busy/locked/permission/etc.), `ProjectionError::Fts5Unavailable`
- `map_open_sqlite_error` maps only clear corruption codes/messages to `CorruptDb`; other SQLite failures stay `Sqlite`
- Sidecar paths use `OsStr` concatenation (`projection_sidecar_paths`) instead of lossy `path.display()` formatting

## Tests

- Injected non-corrupt open errors (`Fts5Unavailable`, `Io`, `Sqlite`) leave main DB + `-wal`/`-shm`/`-journal` markers untouched; reopen yields `OpenedExisting` with original `built_at`
- Directory-as-DB open failure preserves sidecars (end-to-end non-corruption path)
- Existing corrupt + unsupported-schema replacement tests retained
- Classifier unit coverage for replaceable vs non-replaceable errors
- Sidecar path construction asserts non-lossy suffix append

## Local verification (2026-10-08, Windows)

Host target dir: `%LOCALAPPDATA%\Cryptex-Dev\cargo-target-m3`

- `cargo fmt --all -- --check` - PASS
- `cargo test -p cryptex-projection --lib` - PASS (25 tests)
- `cargo test -p cryptexd --lib projection::` - PASS (11 tests)
- `cargo clippy -p cryptex-projection -p cryptexd --all-targets -- -D warnings` - PASS

No GitHub Actions dispatched.

## macOS verification still needed

macOS projection/feed/rebuild smoke remains **pending** until actually run on a Mac.

## Continuation

Do not start Slice 6 until this follow-up tip is reviewed.
