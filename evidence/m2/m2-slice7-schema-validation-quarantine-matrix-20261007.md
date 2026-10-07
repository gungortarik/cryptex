# M2 Slice 7 — Schema Validation + Quarantine Failure Matrix

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`
**Tip:** `eb66027d5bcfce6b5a406cd7fb6a06174d584e51`

## Scope

- `cryptex-events::validate_accepted_record_bytes` with typed `ValidationReasonCode` (exact `schema_version == 1`, unknown fields, duplicate keys, limits, integrity).
- Writer `VerifyingBodyValidator` uses full accepted-record validation (not parse+verify only).
- `StorageError::Validation` carries reason codes into `.open` recovery quarantine metadata (`schema_version_if_parsed` optional field).
- Sealed packs that fail verification are quarantined whole and fail closed (including empty verified prefix).
- Quarantine entry directories use `{utc}_{entry_id}` (reason_code retained in `quarantine.json` only) to avoid Windows `MAX_PATH` failures under deep temp roots; metadata still records full `reason_code` per ADR-0016 section 8.
- Failure-matrix coverage in cryptexd: unsupported schema, integrity failure, sealed corruption; storage: reason-code propagation.

## Local verification (Windows)

- `cargo test -p cryptex-events --lib validate_record --locked` — PASS (6)
- `cargo test -p cryptex-storage --locked` — PASS (22)
- `cargo test -p cryptexd --lib canonical_writer --locked` — PASS (9)

## Non-goals

- GitHub Actions / final M2 CI gate
- Merge or M2 completion claim
- Cross-host macOS physical parity run (Windows local matrix only this slice)

## Next

- Remaining cross-platform / docs-17 matrix gaps as needed, then final M2 candidate CI once.
