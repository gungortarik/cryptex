# M3 Slice 7 - Cross-platform verification + CI wiring

**Date:** 2026-10-08  
**Branch:** `m3/local-projection-search`  
**Base tip before slice:** `243eec0ff21c6e83c30fcbd420ea5b8bb7cdcf0f`
**Slice 7 tip:** `176195af4e70f734ab465a09589c000201d785a0`  
**Plan:** `evidence/m3/m3-phase0-implementation-plan-20261008.md` §8.1–8.2 / §9 Slice 7

## Scope

- Wire `cryptex-projection` and `cryptexd` `projection::` checks into `.github/workflows/ci.yml` readiness jobs (both `windows-latest` and `macos-latest`).
- Close remaining §8.1 matrix gaps: sealed+open projection with truncated open-tail exclusion, multi-device union search + `device_id` filter, idempotent double rebuild.
- Run and record Windows-local verification.
- Prepare exact matching macOS owner commands.
- Do **not** dispatch GitHub Actions (one required final workflow reserved for after both-OS verification + review).
- No Desktop/MCP UI, semantic search, or later-milestone work.

## CI wiring

Added readiness steps after the M2 events/storage tests:

```yaml
- name: Test M3 projection crate
  run: cargo test --locked -p cryptex-projection

- name: Test M3 cryptexd projection module
  run: cargo test --locked -p cryptexd --lib projection::
```

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

## macOS verification — PENDING (owner Mac)

Do **not** claim cross-platform PASS until the following succeeds on macOS against the same tip SHA as this evidence tip.

```bash
cd /path/to/cryptex
git fetch origin
git checkout m3/local-projection-search
git pull --ff-only origin m3/local-projection-search
# Confirm tip matches the Slice 7 push SHA recorded in handoff.

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

## Final CI (after both-OS verification + review only — do not run yet)

```bash
gh workflow run ci.yml --ref m3/local-projection-search
# or open a PR into the agreed base and rely on pull_request readiness jobs
```

Confirm readiness on **windows-latest** and **macos-latest** each execute the new M3 projection steps. Record the Actions run URL only after green.

## Non-goals / out of scope

- Dispatching Actions now
- Claiming dual-OS / CI PASS
- Desktop Cmd/Ctrl+K, MCP tools, semantic/vector search, M4+, Syncthing of DB
- Slice 8 DoD/security/owner review gate
