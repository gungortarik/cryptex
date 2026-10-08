# M3 Slice 4 follow-up - Idempotent replay, typed conflict, feed error categories

**Date:** 2026-10-08  
**Branch:** `m3/local-projection-search`  
**Base tip before follow-up:** `df3d1558904e1975c2ed6ada07583eab84ee2ff3`  
**Parent evidence:** `evidence/m3/m3-slice4-verified-feed-integration-20261008.md`

## Scope

- Replace `INSERT OR IGNORE` with plain insert + PK-conflict reconcile.
- Identical `event_id` replay is idempotent (one events row / one FTS row).
- Existing `event_id` with differing M2-derived fields returns `ProjectionError::EventConflict` (field names only; no event text / hashes / secrets in the message body beyond `event_id` identity).
- Unrelated SQLite constraint failures are not swallowed.
- `project_verified_device_events` surfaces projection insert failures as `ProjectionFeedError::Projection` (not remapped to Canonical/`Invalid`).
- Canonical reader failures remain `ProjectionFeedError::Canonical`.
- Focused regression: projection conflict category + in-txn insert rollback.

## Non-goals (unchanged)

- Checkpoint advancement / rebuild / search APIs
- Desktop/MCP wiring
- GitHub Actions
- macOS host verification (still pending)

## Design notes

### `project_memory_note`

- Attempts `INSERT INTO events ...`.
- On `events.event_id` unique/PK constraint only, loads the stored row and compares M2-derived fields (including text).
- Matching fields → `Ok(())` (idempotent).
- Differing fields → `ProjectionError::EventConflict { event_id, differing_fields }` with column names only.
- Other SQLite errors propagate as `ProjectionError::Sqlite`.

### Feed error category preservation

- Callback stashes the original `ProjectionError` before returning a sentinel `CanonicalWriterError::Invalid` to stop the verified reader.
- After the feed returns, stashed projection errors become `ProjectionFeedError::Projection`.
- Reader/canonical failures with no stash remain `ProjectionFeedError::Canonical`.
- Failed feeds drop the SQLite transaction (rollback).

## Local verification (2026-10-08, Windows)

Host target dir: `%LOCALAPPDATA%\Cryptex-Dev\cargo-target-m3`

- `cargo fmt --all -- --check` - PASS
- `cargo test -p cryptex-projection --lib` - PASS (17 tests), including:
  - `identical_replay_is_idempotent_single_event_and_fts_row`
  - `conflicting_replay_returns_typed_error_and_leaves_row_unchanged`
- `cargo test -p cryptexd --lib projection::` - PASS (3 tests), including:
  - `projection_conflict_is_surfaced_as_projection_error_and_rolls_back`
  - `corrupt_sealed_pack_fails_closed_and_does_not_commit_rows` (Canonical category preserved)
- `cargo clippy -p cryptex-projection -p cryptexd --all-targets -- -D warnings` - PASS

No GitHub Actions dispatched.

## macOS verification still needed

macOS projection/feed fmt/test/clippy remains **pending** until actually run on a Mac.

## Continuation

Next: Slice 5 (rebuild/checkpoint/corruption) after this follow-up tip is reviewed. Do not start Slice 5 opportunistically.
