# M3 Slice 5 - Rebuild, incremental catch-up, checkpoint

**Date:** 2026-10-08  
**Branch:** `m3/local-projection-search`  
**Base tip before slice:** `83e332186034691f05aef3e079b113d8006aeb96`  
**Plan:** `evidence/m3/m3-phase0-implementation-plan-20261008.md` Slice 5 (docs/17 §9 matrix)

## Scope

- Rebuild and incremental projection from the existing cryptexd verified-event feed.
- Advance each device checkpoint in the same SQLite transaction as its projected rows.
- Detect missing / corrupt / unsupported-schema / checkpoint-ahead / unknown-hash cases; preserve canonical packs; rebuild only disposable projection.
- Keep sealed-pack fail-closed verification and active `.open` recovery on the M2 path.
- Focused tests for clean rebuild, restart catch-up without duplicates, missing/corrupt DB rebuild, interrupted transaction, unchanged canonical packs, unsupported schema, and checkpoint invalidation.

## Non-goals (deferred)

- Search / metadata-filter APIs (Slice 6)
- Desktop/MCP wiring
- M4 semantic fields
- GitHub Actions
- macOS host verification (still pending)

## Design

### Disposable open/replace (`cryptex-projection`)

- `ProjectionDb::open_replacing_unusable` opens a healthy v1 DB or deletes disposable files (`projection.sqlite` + `-wal`/`-shm`/`-journal`) and recreates empty v1 when:
  - SQLite open/`quick_check` fails (`CorruptDb`)
  - `schema_version` unsupported
  - schema objects inconsistent
- `remove_projection_files` never touches `canonical/`.

### Checkpoints

- `DeviceCheckpoint` + `read_device_checkpoint` / `upsert_device_checkpoint`
- `clear_device_projection` removes events + external-content FTS rows + checkpoint for one device
- Checkpoint fields: `last_device_seq`, `last_content_hash`, `last_sealed_index`, `open_verified_prefix_len`

### cryptexd sync APIs

- `open_projection_db` → replace-unusable open
- `catch_up_device_projection` → incremental from checkpoint; rebuild on missing/unknown/ahead tip
- `rebuild_device_projection` → clear device rows, project verified feed, write checkpoint
- `project_verified_device_events` → full idempotent feed project + checkpoint (no clear)
- Insert failures remain `ProjectionFeedError::Projection`; reader failures remain `Canonical`

## Local verification (2026-10-08, Windows)

Host target dir: `%LOCALAPPDATA%\Cryptex-Dev\cargo-target-m3`

- `cargo fmt --all -- --check` - PASS
- `cargo test -p cryptex-projection --lib` - PASS (21 tests)
- `cargo test -p cryptexd --lib projection::` - PASS (11 tests), including:
  - `clean_rebuild_projects_all_verified_events_and_checkpoint`
  - `restart_catch_up_projects_only_new_events_without_duplicates`
  - `missing_db_rebuild_restores_rows_and_leaves_canonical_unchanged`
  - `corrupt_db_rebuild_restores_rows_and_leaves_canonical_unchanged`
  - `interrupted_transaction_rolls_back_and_retry_catch_up_succeeds`
  - `checkpoint_unknown_hash_triggers_rebuild_without_duplicates`
  - `checkpoint_ahead_of_packs_triggers_rebuild`
  - `unsupported_schema_replaces_disposable_db_then_catch_up`
  - `corrupt_sealed_pack_fails_closed_and_does_not_commit_rows`
- `cargo clippy -p cryptex-projection -p cryptexd --all-targets -- -D warnings` - PASS

No GitHub Actions dispatched.

## macOS verification still needed

macOS FTS5/feed/rebuild smoke remains **pending** until actually run on a Mac.

## Continuation

Next: Slice 6 (search API + metadata filters) after this tip is reviewed. Do not start Slice 6 opportunistically.

## Follow-up

Replace-only-unusable open hardening: `evidence/m3/m3-slice5-replace-only-unusable-20261008.md`.
