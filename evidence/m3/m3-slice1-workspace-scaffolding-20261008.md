# M3 Slice 1 - Workspace Scaffolding

**Date:** 2026-10-08  
**Branch:** `m3/local-projection-search`  
**Base tip before slice:** `be65b2b304ca29d72f607ce5d41fe69d4dd4b808` (Phase 0 plan)  
**Plan:** `evidence/m3/m3-phase0-implementation-plan-20261008.md` Slice 1

## Scope

- Add workspace member `crates/projection` (`cryptex-projection`).
- Expose disposable app-data path helpers for `<app-data-root>/derived/projection/projection.sqlite`.
- Focused layout unit tests only.
- Preserve canonical-data and single-writer boundaries: no `.open`/`.seg` I/O, no SQLite dependency pins, no cryptexd writer/search API wiring.

## Non-goals (deferred)

- SQLite / `rusqlite` / FTS5 pins (Slice 2)
- Projection schema DDL, checkpoint tables, rebuild/search APIs
- GitHub Actions / CI workflow wiring
- Desktop / MCP search UX

## Files

| Path | Role |
|---|---|
| `Cargo.toml` | Workspace member `crates/projection` |
| `Cargo.lock` | Registers `cryptex-projection` with **no** package dependencies |
| `crates/projection/Cargo.toml` | Package manifest; empty `[dependencies]` |
| `crates/projection/src/lib.rs` | Crate root; documents derived-state boundary |
| `crates/projection/src/layout.rs` | `derived_projection_dir`, `projection_sqlite_path`, `PROJECTION_DB_FILE_NAME` |

Removed placeholder `crates/projection/.gitkeep` now that sources exist.

## Boundary checks encoded in tests

- Projection DB path is under `derived/projection/` relative to the supplied app-data root.
- Path components exclude `canonical`, `identity`, and `quarantine`.
- Leaf name is exactly `projection.sqlite`.

## Local verification (2026-10-08, Windows)

Host target dir used: `%LOCALAPPDATA%\Cryptex-Dev\cargo-target-m3` (sandbox cargo-target path hit Smart App Control os error 4551 when executing the test binary; compile still succeeded there).

- `cargo fmt -p cryptex-projection -- --check` - PASS
- `cargo test -p cryptex-projection --locked` - PASS (3 tests)
- `cargo clippy -p cryptex-projection --all-targets --locked -- -D warnings` - PASS

No GitHub Actions dispatched for this routine slice.

## Continuation

Next: Slice 2 (SQLite pin + FTS5 smoke) only after this tip is reviewed. Do not expand into rebuild/search product behavior or Actions until the plan says so.
