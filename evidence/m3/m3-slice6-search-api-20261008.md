# M3 Slice 6 - Library search API (FTS + metadata filters)

**Date:** 2026-10-08  
**Branch:** `m3/local-projection-search`  
**Base tip before slice:** `da0b9a8023dc141517668912cab902fe1a1baffc`  
**Plan:** `evidence/m3/m3-phase0-implementation-plan-20261008.md` Slice 6 / §4.2–4.3

## Scope

- Library-level `search_events` over disposable projection rows (`cryptex-projection`).
- Optional FTS5 text query plus conjunction filters: `classification`, `event_type`, `device_id`, `created_at` range, optional `brain_id`.
- Parameterized SQL for all bound values.
- Typed `ProjectionError::InvalidQuery` for empty text, bad RFC3339 bounds, inverted ranges, and invalid/unsupported FTS (never silent empty on query errors).
- M2 `memory.note` fields only; no semantic/AI/MCP/desktop surfaces.

## Design

### API

- `SearchQuery`, `SearchHit`, `search_events(conn, query)`
- Deterministic order: UTC `created_at` ascending, then `event_id` ascending
- Optional `limit` applied after ordering

### `created_at` ranges

- Bounds validated with the same M2 RFC3339 grammar as events (`parse_rfc3339` / `rfc3339_unix_seconds` in `cryptex-core`)
- Comparison uses UTC Unix seconds so offset spellings that name the same instant are equivalent
- No new stored projection column; lexical string ordering is not used for ranges

### FTS

- `events_fts MATCH ?` via `rowid IN (SELECT …)` when `text` is set
- SQLite FTS failures while a text query is active map to `InvalidQuery`

## Tests (`search::`)

- FTS match / non-match
- Unicode (`café`)
- Each metadata filter alone and combined
- `created_at` range with offset-equivalent bounds
- Deterministic ordering + limit
- Malformed FTS / empty text / bad bounds → `InvalidQuery`

## Local verification (2026-10-08, Windows)

Host target dir: `%LOCALAPPDATA%\Cryptex-Dev\cargo-target-m3-s6`

- `cargo fmt --all -- --check` - PASS
- `cargo test -p cryptex-core --lib validate::` - PASS (8)
- `cargo test -p cryptex-projection --lib` - PASS (33)
- `cargo test -p cryptexd --lib projection::` - PASS (11)
- `cargo clippy -p cryptex-core -p cryptex-projection -p cryptexd --all-targets -- -D warnings` - PASS

No GitHub Actions dispatched.

## macOS verification still needed

macOS FTS5/feed/rebuild/search smoke remains **pending** until actually run on a Mac.

## Continuation

Next: Slice 7 (cross-platform + CI wire) after this tip is reviewed. Do not start Slice 7 opportunistically.
