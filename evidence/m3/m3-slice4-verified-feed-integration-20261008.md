# M3 Slice 4 - Verified feed integration

**Date:** 2026-10-08  
**Branch:** `m3/local-projection-search`  
**Base tip before slice:** `ffae4e5c2bc11c285282f2b793bfe126854a1c7a`  
**Plan:** `evidence/m3/m3-phase0-implementation-plan-20261008.md` Slice 4

## Scope

- Stream verified canonical events through cryptexd without a parallel `.open`/`.seg` parser.
- Project only M2 `memory.note` fields into disposable SQLite + text FTS.
- Keep canonical packs read-only for projection (M2 recover/quarantine remains the verified-reader path).
- Focused tests for valid feed and corrupt sealed failure/rollback.

## Non-goals (deferred)

- Full rebuild/corruption matrix and checkpoint advancement (Slice 5+)
- Search / metadata-filter APIs
- Desktop/MCP wiring
- M4 semantic fields
- GitHub Actions

## Design

### Verified reader (cryptexd)

- Added `canonical_writer::for_each_verified_event` that reuses discover / validate / sealed fail-closed quarantine / active-open recover-then-prefix.
- Yields events **pack-at-a-time** so a full-history `Vec` is not required for projection.
- `load_verified_events` now wraps the streaming API (diagnostics/tests).

### Projection owner (cryptexd)

- New `services/cryptexd/src/projection.rs`:
  - `project_verified_device_events` opens a SQLite transaction, feeds `for_each_verified_event`, inserts via `cryptex_projection::project_memory_note`, commits only on full success.
  - Feed/canonical errors roll back projected rows.

### Insert helper (cryptex-projection)

- `project_memory_note` / `project_memory_notes` insert M2-derived columns and sync `events_fts(rowid, text)`.
- Depends on `cryptex-events` for `AcceptedEvent` typing only (no pack I/O).

## Local verification (2026-10-08, Windows)

Host target dir: `%LOCALAPPDATA%\Cryptex-Dev\cargo-target-m3`

- `cargo fmt -p cryptex-projection -p cryptexd -- --check` - PASS
- `cargo test -p cryptex-projection --locked` - PASS (15 tests)
- `cargo test -p cryptexd --locked --lib projects_verified_memory_notes` - PASS
- `cargo test -p cryptexd --locked --lib corrupt_sealed_pack_fails_closed` - PASS
- `cargo test -p cryptexd --locked --lib load_verified_events_fail_closes_and_quarantines_corrupt_sealed_pack` - PASS
- `cargo clippy -p cryptex-projection -p cryptexd --all-targets --locked -- -D warnings` - PASS

No GitHub Actions dispatched.

## macOS verification still needed

macOS fmt/test/clippy for projection + cryptexd feed path remains **pending** until actually run.

## Continuation

Next: Slice 5 (rebuild + incremental + corruption / checkpoint semantics) after this tip is reviewed.
