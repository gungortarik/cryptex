# M3 Slice 7 - Cross-platform verification + CI wiring

**Date:** 2026-10-08  
**Branch:** `m3/local-projection-search`  
**Base tip before slice:** `243eec0ff21c6e83c30fcbd420ea5b8bb7cdcf0f`
**Slice 7 tip:** `d13c59434398379b344818e73b0d0d4f602a5f00` on `m3/local-projection-search` (behavior `176195af4e70f734ab465a09589c000201d785a0`; later docs-only commits may advance HEAD — `git pull` then run on branch tip)
**Plan:** `evidence/m3/m3-phase0-implementation-plan-20261008.md` §8.1–8.2 / §9 Slice 7

## Scope

- Wire `cryptex-projection` into `.github/workflows/ci.yml` readiness jobs (both `windows-latest` and `macos-latest`). Rely on the existing full `cargo test --locked -p cryptexd` step for `projection::` coverage (no duplicate filtered cryptexd step).
- Close remaining §8.1 matrix gaps: sealed+open projection with truncated open-tail exclusion, multi-device union search + `device_id` filter, idempotent double rebuild.
- Run and record Windows-local verification.
- Prepare exact matching macOS owner commands.
- Do **not** dispatch GitHub Actions (one required final workflow reserved for after both-OS verification + review).
- No Desktop/MCP UI, semantic search, or later-milestone work.

## CI wiring

Added one readiness step after the M2 events/storage tests:

```yaml
- name: Test M3 projection crate
  run: cargo test --locked -p cryptex-projection
```

`cryptexd` `projection::` coverage is provided by the existing readiness step `cargo test --locked -p cryptexd` (full package tests already include the library `projection::` module). A separate `cargo test --locked -p cryptexd --lib projection::` step was removed before the final Actions run to avoid duplicating those tests on both OS jobs.

Existing readiness already runs workspace `fmt`, `clippy -D warnings`, and full `cargo test -p cryptexd`. No Actions dispatched from this slice.

## Contract matrix coverage (§8.1)

| # | Requirement | Primary coverage | Windows result |
|---|-------------|------------------|----------------|
| 1 | Clean build from canonical writer | `projection::tests::projects_verified_memory_notes_from_canonical_writer`, `clean_rebuild_*` | PASS (in `projection::` 14) |
| 2 | FTS5 match/non-match + Unicode | `search::tests::fts_match_and_non_match`, `fts_unicode_match`; `fts::` smoke | PASS (in `cryptex-projection` 35) |
| 3 | Metadata + exact-time filters | `search::tests::metadata_filters_*`, `created_at_range_*`, fractional inclusive bounds | PASS |
| 4 | Missing DB rebuild; canonical unchanged | `missing_db_rebuild_restores_rows_and_leaves_canonical_unchanged` | PASS |
| 5 | Corrupt DB rebuild; canonical unchanged | `corrupt_db_rebuild_restores_rows_and_leaves_canonical_unchanged` | PASS |
| 6 | Checkpoint / restart catch-up | `restart_catch_up_projects_only_new_events_without_duplicates` | PASS |
| 7 | Sealed + open; truncated open tail excluded | **new** `sealed_and_open_events_project_truncated_open_tail_excluded` | PASS |
| 8 | Sealed corruption fail-closed | `corrupt_sealed_pack_fails_closed_and_does_not_commit_rows` | PASS |
| 9 | Multi-device union search + `device_id` | **new** `multi_device_union_search_filters_and_leaves_canonical_unchanged` | PASS |
| 10 | Idempotent rebuild; disposable only | **new** `idempotent_double_rebuild_stable_counts_and_canonical_unchanged` | PASS |

Canonical sealed packs are asserted unchanged across rebuild/search paths that mutate disposable state. Truncated-open recovery may shrink `.open` (expected M2 recovery); sealed `.seg` bytes are asserted unchanged.

## Local verification (Windows host) — PASS

Host target dir: `%LOCALAPPDATA%\Cryptex-Dev\cargo-target-m3-s7`

| Command | Result |
|---------|--------|
| `cargo fmt --all -- --check` | PASS |
| `cargo test --locked -p cryptex-projection` | PASS (35) |
| `cargo test --locked -p cryptexd --lib projection::` | PASS (14) |
| `cargo clippy --locked -p cryptex-core -p cryptex-projection -p cryptexd --all-targets -- -D warnings` | PASS |

No GitHub Actions dispatched.

## macOS verification — PASS (owner follow-up)

Owner macOS four-check PASS at exact HEAD `428b03f33eda8fbf645678220dbd3ce3d88d108e` is recorded in `evidence/m3/m3-slice7-macos-four-check-20261008.md`. Windows-local PASS above is preserved. Both OS **local** checks pass at that tip. Commands below remain the reference checklist; they have been executed on the Mac.

Historical note — original Slice 7 write-up left macOS pending with these commands:

```bash
cd /path/to/cryptex
git fetch origin
git checkout m3/local-projection-search
git pull --ff-only origin m3/local-projection-search
# Confirm tip is at least behavior 176195a (branch tip at write-up: d13c594).

cargo fmt --all -- --check
cargo test --locked -p cryptex-projection
cargo test --locked -p cryptexd --lib projection::
cargo clippy --locked -p cryptex-core -p cryptex-projection -p cryptexd --all-targets -- -D warnings
```

Optional focused matrix smokes on macOS (same binaries as above):

```bash
cargo test --locked -p cryptex-projection --lib search::
cargo test --locked -p cryptex-projection --lib fts::
cargo test --locked -p cryptexd --lib projection::tests::sealed_and_open_events_project_truncated_open_tail_excluded
cargo test --locked -p cryptexd --lib projection::tests::multi_device_union_search_filters_and_leaves_canonical_unchanged
cargo test --locked -p cryptexd --lib projection::tests::missing_db_rebuild_restores_rows_and_leaves_canonical_unchanged
cargo test --locked -p cryptexd --lib projection::tests::corrupt_db_rebuild_restores_rows_and_leaves_canonical_unchanged
cargo test --locked -p cryptexd --lib projection::tests::corrupt_sealed_pack_fails_closed_and_does_not_commit_rows
cargo test --locked -p cryptexd --lib projection::tests::restart_catch_up_projects_only_new_events_without_duplicates
```

Record macOS host, date, tip SHA, and pass/fail in a follow-up evidence note before any Actions dispatch.

## Final CI (both-OS local PASS recorded; after tip review only — do not run yet)

```bash
gh workflow run ci.yml --ref m3/local-projection-search
# or open a PR into the agreed base and rely on pull_request readiness jobs
```

Confirm readiness on **windows-latest** and **macos-latest** each execute the M3 `cryptex-projection` step, and that cryptexd projection coverage comes only from the existing full `cryptexd` test (no duplicate `projection::` filter step). Record the Actions run URL only after green.

## Non-goals / out of scope

- Dispatching Actions now
- Claiming dual-OS / CI PASS
- Desktop Cmd/Ctrl+K, MCP tools, semantic/vector search, M4+, Syncthing of DB
- Slice 8 DoD/security/owner review gate

## Follow-up (redundant cryptexd CI step removed)

Before the final M3 Actions run, removed the readiness step
`cargo test --locked -p cryptexd --lib projection::` because it duplicated tests already
executed by `cargo test --locked -p cryptexd`. Kept `cargo test --locked -p cryptex-projection`.
Local Windows matrix commands that filter `projection::` remain valid focused verification; they
are not duplicated in CI. No product behavior change. Owner macOS four-check later PASS at `428b03f` is recorded in `evidence/m3/m3-slice7-macos-four-check-20261008.md`; Actions still not dispatched.


## Follow-up (macOS four-check)

Owner macOS four-check PASS at `428b03f33eda8fbf645678220dbd3ce3d88d108e`: `evidence/m3/m3-slice7-macos-four-check-20261008.md`.
Both OS local verification now PASS at that tip. Actions still not dispatched; M3 not complete.
