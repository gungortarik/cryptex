# M3 Slice 3 - Projection schema v1 open/create

**Date:** 2026-10-08  
**Branch:** `m3/local-projection-search`  
**Base tip before slice:** `08af64df7d4986568cd859ae8d4fb78ebc82bbcd` (Slice 2)  
**Plan:** `evidence/m3/m3-phase0-implementation-plan-20261008.md` Slice 3

## Scope

- Empty projection schema version 1: `events`, text-only `events_fts` (content=`events`), `projection_meta`, `device_checkpoint`.
- Columns derived strictly from M2 `AcceptedEvent` / `memory.note` (no M4 semantic fields).
- `ProjectionDb::open_or_create` / `open_in_memory` with schema-version validation.
- Focused tests for fresh create, repeated open, schema/FTS consistency, unsupported schema_version.

## Non-goals (deferred)

- Event ingestion / verified feed integration
- Checkpoint advancement / rebuild
- Search / metadata-filter APIs
- cryptexd wiring
- GitHub Actions

## Schema v1

`PROJECTION_SCHEMA_VERSION = 1` (derived schema; not event `schema_version`).

| Object | Role |
|---|---|
| `projection_meta` | Singleton (`id=1`): `schema_version`, `built_at`, `build_id` |
| `events` | Content table: `event_id`, `brain_id`, `device_id`, `device_seq`, `created_at`, `event_type`, `classification`, `content_hash`, `signing_key_id`, `text` |
| `events_fts` | FTS5 over `text` only; `content='events'`, `content_rowid='rowid'` |
| `device_checkpoint` | Per-device cursor columns (empty until later slices): `last_device_seq`, `last_content_hash`, `last_sealed_index`, `open_verified_prefix_len` |

Unsupported `projection_meta.schema_version` fails closed as `ProjectionError::UnsupportedSchema`.

## Code surface

| Path | Role |
|---|---|
| `crates/projection/src/schema.rs` | DDL apply + validate |
| `crates/projection/src/db.rs` | `ProjectionDb` open/create |
| `crates/projection/src/error.rs` | `UnsupportedSchema`, `SchemaInconsistent`, `Io` |

## Local verification (2026-10-08, Windows)

Host target dir: `%LOCALAPPDATA%\Cryptex-Dev\cargo-target-m3`

- `cargo fmt -p cryptex-projection -- --check` - PASS
- `cargo test -p cryptex-projection --locked` - PASS (9 tests)
- `cargo clippy -p cryptex-projection --all-targets --locked -- -D warnings` - PASS

No GitHub Actions dispatched.

## macOS verification still needed

macOS FTS5 / schema smoke (`fmt --check`, `test --locked`, `clippy -D warnings` on this tip) remains **pending** until actually run on a macOS host. Windows-only evidence does not claim macOS PASS.

## Continuation

Next: Slice 4 (verified feed integration) after this tip is reviewed.

## Follow-up

Fixture exclusivity + exact schema-shape hardening: `evidence/m3/m3-slice3-fixture-schema-hardening-20261008.md`.
