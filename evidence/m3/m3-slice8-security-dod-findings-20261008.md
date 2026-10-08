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

## macOS / final CI — historically pending at the initial Slice 8 review

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

Historical status at this point: no Actions had been dispatched. This was superseded by the final candidate verification below; run 37720627242 covered only `51e6dc7`.


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


## Follow-up (constraint-class only)

**Constraint-class tip:** `7043c1f8633beebe5f43dfc313e7d72a8c682366`

`is_constraint_rejection` now accepts only `ErrorCode::ConstraintViolation`. The previous fallback that treated any error message containing `"constraint"` as success was removed — error text is not proof of the SQLite result class.

Regression: inject `SystemIoFailure` whose message contains `"constraint"`; schema validation must fail closed as non-replaceable `Sqlite` (not treated as a successful non-1 id rejection). Existing true-constraint and busy-inject coverage retained.

### Windows re-check

| Command | Result |
|---------|--------|
| `cargo fmt --all -- --check` | PASS |
| `cargo test --locked -p cryptex-projection --lib` | PASS |
| `cargo clippy --locked -p cryptex-core -p cryptex-projection -p cryptexd --all-targets -- -D warnings` | PASS |

Historical status at this point: Actions and owner macOS verification were pending for this intermediate follow-up tip.

## Final candidate verification

**Candidate SHA:** `275fb7acf0c2c7b4d7c050acb54670e81520377f`

- Windows local checks: fmt, projection tests (45), projection integration tests (17), and strict Clippy PASS; see branch change record.
- Owner macOS four-check: PASS on macOS 27.0.1 (26A434) arm64 at this exact SHA; see `evidence/m3/m3-slice8-macos-four-check-20261008.md`.
- Final dual-OS Actions: PASS in [run 37725223487](https://github.com/gungortarik/cryptex/actions/runs/37725223487) at this exact SHA; see `evidence/m3/m3-final-ci-gate-20261008.md`.

These results close the candidate's local platform and CI verification. They do not themselves close M3: final security/DoD disposition and human owner review are still required. No merge or M4 start is recorded here.

## Post-gate review finding — error text can still trigger replacement

**Affected candidate:** `275fb7acf0c2c7b4d7c050acb54670e81520377f`
**Status:** Code fixed on branch tip (see follow-up below); Windows-local re-check PASS; macOS four-check + final dual-OS Actions still required before closeout.

Review of `crates/projection/src/db.rs` found that both `map_fts_integrity_sqlite_error` and `map_open_sqlite_error` classify an error as replaceable `CorruptDb` if its message contains words such as `corrupt`, `integrity-check`, `malformed`, or `not a database`, even when SQLite's primary `ErrorCode` is `SystemIoFailure` or another transient/non-corruption class. `open_replacing_unusable` then removes the projection database and its sidecars for `CorruptDb`.

This is an untested error-classification path, not a report that the normal Windows/macOS runs experienced data loss. Existing injected transient tests use messages such as `disk I/O error`, so they do not exercise an I/O error whose diagnostic text contains a corruption keyword. The previous `ConstraintViolation` fix established that message substrings must not substitute for the typed SQLite result class; apply the same fail-closed rule to both corruption mappers.

Required follow-up for the M3 product-code implementer:

1. Make replaceability depend on SQLite error/extended-code classes that identify corruption, not diagnostic-message substrings. Preserve explicitly supported FTS virtual-table corruption codes.
2. Add injected `SystemIoFailure` regressions with messages containing `malformed`, `corrupt`, `integrity-check`, and `not a database` for both mappers.
3. Assert each remains non-replaceable `ProjectionError::Sqlite` and that main DB bytes plus `-wal`/`-shm`/`-journal` sidecars remain unchanged.
4. Retain regression coverage proving real `DatabaseCorrupt` / FTS5 inconsistency is replaceable and rebuildable.
5. Re-run Windows local checks, the exact-tip macOS four-check, then one final dual-OS Actions run. The current PASS at `275fb7a` does not cover the fix.

Do not merge or declare M3 complete until this finding is closed and final human owner review is recorded.


## Follow-up (corruption class codes only)

**Corruption-class tip:** `6b61882dce0814abd51fbf063c69d7775c483e37`

Closed the post-gate finding: `map_fts_integrity_sqlite_error` and `map_open_sqlite_error` classify replaceable `CorruptDb` only from SQLite error/extended-code classes (`DatabaseCorrupt`, `NotADatabase`, and `SQLITE_CORRUPT_VTAB` extended code `267`). Diagnostic message substrings (`malformed`, `corrupt`, `integrity-check`, `not a database`) are ignored.

Regressions:
- Mapper unit coverage: `SystemIoFailure` with each misleading message stays `ProjectionError::Sqlite`.
- Injected open + FTS integrity paths: same messages fail closed as non-replaceable `Sqlite`; main DB bytes preserved; open-path keeps `-wal`/`-shm`/`-journal`; FTS-path plants `-journal` after open (Windows WAL maps `-wal`/`-shm`).
- Genuine `DatabaseCorrupt` / `NotADatabase` / FTS external-content inconsistency remain replaceable and rebuildable.

### Windows re-check

| Command | Result |
|---------|--------|
| `cargo fmt --all -- --check` | PASS |
| `cargo test --locked -p cryptex-projection --lib` | PASS (47) |
| `cargo test --locked -p cryptexd --lib projection::` | PASS (17) |
| `cargo clippy --locked -p cryptex-core -p cryptex-projection -p cryptexd --all-targets -- -D warnings` | PASS |

### macOS four-check (owner report — PASS)

**Tested HEAD:** `4a2ea56aa2ddc4024d82599f8dc671472179cbe8`

**Host:** macOS 27.0.1 (26A434) / arm64; working tree clean.

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS (exit 0) |
| `cargo test --locked -p cryptex-projection --lib` | PASS (47; exit 0) |
| `cargo test --locked -p cryptexd --lib projection::` | PASS (17; exit 0) |
| `cargo clippy --locked -p cryptex-core -p cryptex-projection -p cryptexd --all-targets -- -D warnings` | PASS (exit 0) |

### Final dual-OS Actions (PASS)

Run [37780668640](https://github.com/gungortarik/cryptex/actions/runs/37780668640)
completed successfully at exact branch HEAD `4a2ea56aa2ddc4024d82599f8dc671472179cbe8`.
Windows readiness, macOS readiness, and Windows IPC cross-account ACL passed;
the unsigned Windows package job was skipped by design. See
`evidence/m3/m3-final-ci-gate-20261008.md` for job details.

The post-gate error-message classification finding is closed for this
candidate. Its local Windows verification, owner-reported Mac verification,
and final dual-OS CI are recorded above. The scoped final security/DoD review
is recorded in `evidence/m3/m3-final-review-20261008.md`. Human owner review
remains before M3 can be completed. Do not merge or start M4 before the M3
completion gate is satisfied.
