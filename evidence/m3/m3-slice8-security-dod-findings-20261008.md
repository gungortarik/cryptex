# M3 Slice 8 - Security/DoD review findings (partial)

**Date:** 2026-10-08  
**Branch:** `m3/local-projection-search`  
**Base tip before slice:** `676f46bb146cc2a5cd8705d5b7f1a1fea1067380`
**Slice 8 tip:** `10a11c82b0e07e09fd7ef97c2384e9a385991adc`  
**Plan:** Phase 0 §9 Slice 8 (DoD / security / owner review) — focused findings only; no architecture change.

## Scope

Addressed security/DoD review findings without weakening tests or expanding into Desktop/MCP/M4:

1. FTS5 external-content integrity on open  
2. Empty-device checkpoint incremental catch-up  
3. Schema v1 type/constraint validation  
4. Bounded-memory review for clear/search  

## Fixes

### 1. FTS5 external-content consistency

- `ProjectionDb::try_open_existing` now runs FTS5 `INSERT INTO events_fts(events_fts, rank) VALUES('integrity-check', 1)` after `PRAGMA quick_check` + schema init.
- Failures map to `ProjectionError::CorruptDb` (replaceable unusable disposable state).
- Regressions: library replace+restore search; cryptexd replace → catch-up rebuild → search restored with canonical packs unchanged.

### 2. Empty-device checkpoint

- `catch_up_device_projection` treats a present checkpoint with both `last_device_seq` and `last_content_hash` null as an empty tip already at feed start (no spurious rebuild).
- Regressions: second empty catch-up is not a rebuild; first append after empty tip projects incrementally (`rebuilt == false`).

### 3. Schema validation

- `validate_schema_v1` now checks declared column types, NOT NULL, PRIMARY KEY flags via `PRAGMA table_info`, plus CREATE-sql needles for `events`/`device_checkpoint` PRIMARY KEY and `projection_meta` `CHECK (id = 1)`.
- Fixtures: same column names with wrong type, missing PK, missing CHECK — fail closed as `SchemaInconsistent`.

### 4. Bounded memory

- **Fixed (straightforward):** `clear_device_projection` deletes content rows then `INSERT INTO events_fts(events_fts) VALUES('rebuild')` instead of materializing all `(rowid, text)` pairs.
- **Fixed (straightforward):** `search_events` with `limit` retains an O(limit) max-heap of best exact-instant hits; `limit == 0` returns empty.
- **Documented (outside aggressive M3 redesign):** unlimited `search_events` (`limit: None`) still collects all matches after filters because exact UTC-instant ordering requires considering every candidate. Callers should pass `limit` for large local stores. This does not weaken the M3 contract (search remains correct; bound is available).

## Local verification (Windows) — PASS

Host target dir: `%LOCALAPPDATA%\Cryptex-Dev\cargo-target-m3-s8`

| Command | Result |
|---------|--------|
| `cargo fmt --all -- --check` | PASS |
| `cargo test --locked -p cryptex-projection --lib` | PASS (39) |
| `cargo test --locked -p cryptexd --lib projection::` | PASS (17) |
| `cargo clippy --locked -p cryptex-core -p cryptex-projection -p cryptexd --all-targets -- -D warnings` | PASS |

No GitHub Actions dispatched.

## macOS / final CI — PENDING

Code changed after CI PASS run [37720627242](https://github.com/gungortarik/cryptex/actions/runs/37720627242) (SHA `51e6dc7`). That Actions result does **not** cover this tip.

Owner Mac must re-run the equivalent four-check on the new tip before any new Actions dispatch:

```bash
cargo fmt --all -- --check
cargo test --locked -p cryptex-projection --lib
cargo test --locked -p cryptexd --lib projection::
cargo clippy --locked -p cryptex-core -p cryptex-projection -p cryptexd --all-targets -- -D warnings
```

Do **not** claim M3 complete, merge, or start M4 until Slice 8 review + required gates pass.

## Non-goals

- Architecture changes, Desktop/MCP surfaces, semantic search  
- Dispatching Actions from this commit  
- Declaring M3 complete


## Follow-up (integrity-check error class + semantic CHECK)

**Follow-up tip:** 04c794f907704455cea2463f13cbce83635940ff

Review blockers fixed on tip after `eee344f`:

1. **FTS integrity-check error mapping:** `map_fts_integrity_sqlite_error` maps only confirmed corruption / FTS integrity mismatch signals to `CorruptDb`. Busy/locked, I/O, permission, and other ordinary SQLite failures remain `ProjectionError::Sqlite` (non-replaceable). Regressions: mapper unit coverage; transient integrity inject preserves main DB bytes and planted `-journal` sidecar; genuine FTS inconsistency still replaces/rebuilds.
2. **projection_meta CHECK:** Replaced `sqlite_master.sql` substring `check (id = 1)` with a semantic savepoint probe inserting `id = 2` (must fail). Permissive `CHECK (id = 1 OR 1=1 /* intended: id = 1 */)` fails closed.

### Windows re-check (this follow-up)

Host target dir: `%LOCALAPPDATA%\Cryptex-Dev\cargo-target-m3-s8b`

| Command | Result |
|---------|--------|
| `cargo fmt --all -- --check` | PASS |
| `cargo test --locked -p cryptex-projection --lib` | PASS (42) |
| `cargo test --locked -p cryptexd --lib projection::` | PASS (17) |
| `cargo clippy --locked -p cryptex-core -p cryptex-projection -p cryptexd --all-targets -- -D warnings` | PASS |

No Actions dispatched. macOS four-check + new dual-OS Actions still required before any CI PASS claim for this tip (run 37720627242 covers `51e6dc7` only).


## Follow-up (meta id CHECK probe hardening)

**Meta-CHECK hardening tip:** 52fe953ecdb29db1f1730148921f1af07be5f50c

Hardened `require_projection_meta_id_singleton_constraint` after tip `d54ef4f`:

1. Only SQLite **constraint** rejection counts as a successful non-1 id probe; busy/locked/I/O/readonly and other errors propagate as non-replaceable `ProjectionError::Sqlite`.
2. Savepoint `ROLLBACK TO` / `RELEASE` errors are never discarded; probe + cleanup failures are both preserved in the returned error; connection left without an open savepoint.
3. Structural parse (comment-stripped) requires CHECK expression normalize to exactly `id=1`; semantic probes cover ids `{0,2,3,-1,100}` so `CHECK (id = 1 OR id = 3)` fails closed.
4. Injected non-constraint probe busy error: validation fails closed as `Sqlite`, `is_autocommit()` remains true, connection still readable.

### Windows re-check

| Command | Result |
|---------|--------|
| `cargo fmt --all -- --check` | PASS |
| `cargo test --locked -p cryptex-projection --lib` | PASS (44) |
| `cargo test --locked -p cryptexd --lib projection::` | PASS (17) |
| `cargo clippy --locked -p cryptex-core -p cryptex-projection -p cryptexd --all-targets -- -D warnings` | PASS |

No Actions dispatched.

### macOS four-check (owner — required before Actions)

```bash
cd /path/to/cryptex
git fetch origin && git checkout m3/local-projection-search
git pull --ff-only origin m3/local-projection-search
# Confirm HEAD equals the follow-up tip recorded below.

cargo fmt --all -- --check
cargo test --locked -p cryptex-projection --lib
cargo test --locked -p cryptexd --lib projection::
cargo clippy --locked -p cryptex-core -p cryptex-projection -p cryptexd --all-targets -- -D warnings
```

Only after tip review **and** macOS PASS: dispatch one new dual-OS Actions run. Prior run 37720627242 covers `51e6dc7` only. Do not merge or claim M3 complete.
