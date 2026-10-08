# M4 Slice 4 — Additive SQLite Migration

**Status:** Partial implementation; Slice 4 remains in progress.

The M4 Slice 3 macOS verification report was checked at exact source SHA
`c4e692c2d335a3abdf8fa6edbe70dc38661e97dd`; its results are recorded in
`m4-slice3-deterministic-reducer-windows-20261008.md`. Windows and macOS local
verification therefore agree on Slice 3 before this Slice 4 work began.

## Implemented in this sub-slice

- Added `cryptex-knowledge::sqlite` migration support for all ADR-0017 M4 table
  boundaries in the existing validated `projection.sqlite`.
- M4 schema version is independent; singleton metadata stores reducer,
  cardinality, normalization, build, and rebuild status fields. It contains no
  duplicated device watermarks.
- Uses `BEGIN IMMEDIATE`; validates M3 schema v1 first; creates or validates
  the complete M4 schema in one transaction; never drops existing M4 tables.
- Validates exact M4 column order, primary keys, schema version, and singleton
  metadata constraint. Unknown M4 versions and incompatible partial tables
  fail closed.
- Regression tests prove idempotent migration, preservation of an existing
  M3 event row and schema version, rollback on an incompatible table, and
  rejection of a permissive metadata singleton constraint.

## Windows local verification

Using `CARGO_TARGET_DIR=%LOCALAPPDATA%\Cryptex-Dev\cargo-target-m4`:

- `cargo fmt -p cryptex-knowledge -p cryptex-projection`: PASS
- `cargo test --locked -p cryptex-knowledge -p cryptex-projection`: PASS —
  knowledge 24, projection 47
- `cargo clippy --locked -p cryptex-knowledge -p cryptex-projection
  --all-targets -- -D warnings`: PASS
- `git diff --check`: PASS

GitHub Actions were not dispatched.

## Still required for Slice 4

Transactional persistence of reducer output and provenance, pending dependency
rows with canonical frame positions, distinct integrity/safe-materialization
watermarks, and full idempotent M4-only rebuild are not implemented yet. Do not
call Slice 4 complete or integrate daemon materialization until these are
implemented and tested. M3 schema v1 and canonical packs remain unchanged;
production v2 writing remains disabled.
