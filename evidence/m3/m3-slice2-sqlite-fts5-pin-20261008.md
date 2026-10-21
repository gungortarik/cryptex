# M3 Slice 2 - SQLite pin + FTS5 smoke

**Date:** 2026-10-08  
**Branch:** `m3/local-projection-search`  
**Base tip before slice:** `30ee2f147f448198f2609f8324ecd9124d85446f` (Slice 1)  
**Plan:** `evidence/m3/m3-phase0-implementation-plan-20261008.md` Slice 2

## Scope

- Exact workspace pin for bundled SQLite via `rusqlite`.
- Minimum feature set that compile-time enables FTS5 on Windows and macOS.
- Fail-closed FTS5 availability probe + unit smoke tests.
- Keep SQLite disposable/derived; no canonical format or writer ownership changes.

## Non-goals (deferred)

- Projection content/FTS product tables and schema_version DDL (Slice 3+)
- Verified event ingestion / rebuild / checkpoint
- Search / metadata-filter APIs
- cryptexd wiring
- GitHub Actions / CI workflow member expansion

## Exact dependency pin

Workspace (`Cargo.toml` `[workspace.dependencies]`):

```toml
rusqlite = { version = "=0.40.2", default-features = false, features = ["bundled"] }
```

Crate (`crates/projection/Cargo.toml`):

```toml
rusqlite = { workspace = true }
```

Resolved lock packages (this tip):

| Package | Version | Notes |
|---|---|---|
| `rusqlite` | `0.40.2` | `default-features = false`; feature `bundled` only |
| `libsqlite3-sys` | `0.38.2` | Bundled amalgamation; build uses `-DSQLITE_ENABLE_FTS5` |

Rationale for `default-features = false`: avoid enabling rusqlite's default `ffi-sqlite-wasm-rs` path; native Cryptex hosts only need the bundled amalgamation. Feature `bundled` is the minimum set that compiles/links SQLite with FTS5 enabled at build time (verified in `libsqlite3-sys` `build.rs`).

## Code surface

| Path | Role |
|---|---|
| `crates/projection/src/error.rs` | `ProjectionError::Fts5Unavailable` (+ SQLite mapping) |
| `crates/projection/src/fts.rs` | `ensure_fts5_available`, `probe_bundled_fts5` |
| `crates/projection/src/layout.rs` | unchanged path helpers from Slice 1 |

Probe behavior:

1. Require `ENABLE_FTS5` in `PRAGMA compile_options`.
2. Exercise `CREATE VIRTUAL TABLE ... USING fts5` on a temp in-memory table.
3. Return typed `Fts5Unavailable` (fail closed) if either step fails.

No product projection tables are created.

## Local verification (2026-10-08, Windows)

Host target dir: `%LOCALAPPDATA%\Cryptex-Dev\cargo-target-m3`

- `cargo fmt -p cryptex-projection -- --check` - PASS
- `cargo test -p cryptex-projection --locked` - PASS (5 tests: 3 layout + 2 FTS5)
- `cargo clippy -p cryptex-projection --all-targets --locked -- -D warnings` - PASS

No GitHub Actions dispatched.

## macOS verification still needed

This slice's FTS5 smoke was executed on Windows only. Before the M3 final gate, macOS must run the same three commands (`fmt --check`, `test --locked`, `clippy -D warnings`) against the slice tip to confirm the bundled amalgamation + FTS5 probe on arm64/x86_64 macOS. CI four-check / dual-OS readiness remains deferred until a reviewed M3 candidate (plan Slice 7).

## Continuation

Next: Slice 3 (schema + empty open/create) after this tip is reviewed.
