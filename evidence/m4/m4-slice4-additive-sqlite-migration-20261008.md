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
- Added one-transaction M4-only rebuild from reducer state produced by the
  trusted `VerifiedV2History` boundary. It persists entities, aliases,
  assertions, provenance (including signing-key id and authorization receipt
  reference), conflicts, corrections, merge records, pending dependencies,
  and per-device integrity/safe watermarks. Rebuild is idempotent and leaves
  M3 rows/schema intact.
- Rebuild input validation prevents a safe watermark from crossing an
  unresolved dependency, prevents watermarks from exceeding verified
  integrity progress, and requires supplied frame positions to match device,
  event, and content hash. Pending dependency rows retain the referenced
  semantic ID without mislabeling it as an event ID.
- Added regressions for transactional rollback preserving the prior M4
  generation, pending dependency persistence with the safe watermark held
  before it, and signing-key provenance persistence.

## Windows local verification

Using `CARGO_TARGET_DIR=%LOCALAPPDATA%\Cryptex-Dev\cargo-target-m4`:

- `cargo fmt -p cryptex-knowledge`: PASS
- `cargo test --locked -p cryptex-knowledge -p cryptex-projection`: PASS —
  knowledge 28, projection 47
- `cargo clippy --locked -p cryptex-knowledge -p cryptex-projection
  --all-targets -- -D warnings`: PASS
- `git diff --check`: PASS

GitHub Actions were not dispatched.

## Still required for Slice 4

Slice 4 remains in progress. Remaining work: integrate the verified canonical
scanner with this rebuild boundary; prove late cross-device dependencies
resolve incrementally to the same output as a full rebuild; verify recovery
from a disposable M4 database replacement; and complete the required M4
verification matrix. Do not call Slice 4 complete before those checks. M3
schema v1 and canonical packs remain unchanged; production v2 writing remains
disabled.
