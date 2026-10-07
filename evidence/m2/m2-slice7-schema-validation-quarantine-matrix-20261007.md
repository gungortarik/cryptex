# M2 Slice 7 — Schema Validation + Quarantine Failure Matrix

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`
**Tip:** `92d9c6b6930b4363c1705bb559de61c49907e124`

## Scope

- `cryptex-events::validate_accepted_record_bytes` with typed `ValidationReasonCode` (exact `schema_version == 1`, unknown fields, duplicate keys, limits, integrity).
- Writer `VerifyingBodyValidator` uses full accepted-record validation (not parse+verify only).
- `StorageError::Validation` carries reason codes into `.open` recovery quarantine metadata (`schema_version_if_parsed` optional field).
- Sealed packs that fail verification are quarantined whole and fail closed (including empty verified prefix).
- Quarantine entry directories use ADR-0016 §8.1 `{utc_compact}_{reason_code}_{entry_id_hex}` with allow-listed `QuarantineReasonCode` path tokens (metadata still records `reason_code`).
- **Historical interim (superseded):** slice-7 initially shortened directories to `{utc}_{entry_id}` (reason only in `quarantine.json`) to dodge Windows `MAX_PATH` under deep TEMP fixtures. That was out of contract with accepted ADR-0016 §8.1 and was restored; see correction below.
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

## Correction (2026-10-07) — ADR-0016 §8.1 directory format restored

**Behavior tip (this correction):** `e098e8a431504e34a4659ee48546f7b1041a2315`

Accepted ADR-0016 §8.1 directory format is restored in `crates/storage/src/quarantine.rs`:

`{utc_compact}_{reason_code}_{entry_id_hex}`

- `reason_code` is a typed allow-list (`QuarantineReasonCode`): fixed snake_case tokens, no separators or arbitrary path text.
- Collision handling appends a monotonic suffix to the leaf name under the same parent (no-replace).
- Windows path-length regression uses a production-like `%LOCALAPPDATA%\Cryptex-Dev\p0-2\m2-tests\...` root (not deep TEMP).

### Measured Windows path lengths (host LocalAppData)

| Case | Object path length (chars) | Classic MAX_PATH (260) |
|---|---:|---|
| Realistic `device-safe-1` + longest reason `corrupt_frame_header` | 247 | under budget |
| Worst-case max-length DeviceId (64 → 128 hex stem) + same leaf | 349 | **exceeds** |

Realistic production-like roots succeed with the accepted layout. Max-length DeviceId under §8.1 attribution `device-<device_id_hex>` exceeds classic Win32 MAX_PATH; layout was **not** shortened again. That edge case needs a **separate ADR decision** (e.g. long-path APIs vs attribution/name policy)—not an implementer unilateral format change.

### Local verification (Windows, this correction)

- `cargo test --locked -p cryptex-storage quarantine -- --test-threads=8` — PASS
- `cargo test --locked -p cryptex-storage -- --test-threads=8` — PASS (32)
- `cargo test --locked -p cryptexd --lib canonical_writer -- --test-threads=8` — PASS (10)
- `cargo fmt --all -- --check` — PASS
- `cargo clippy --locked -p cryptex-storage --all-targets -- -D warnings` — PASS

### Remaining verification

- Owner review of this tip
- Dual-OS Actions still deferred until final gate is required after tip review
- Max-length DeviceId × classic MAX_PATH: open for separate ADR (see table)

### Follow-up measurement (slice 8f)

Actual Windows FS quarantine ops (not only character counts) for UUID-length and
max DeviceId under production-like LocalAppData are recorded in
`evidence/m2/m2-slice8f-windows-path-ops-and-device-binding-20261007.md`. Historical
character-count notes above remain; do not treat them as the final path-ops evidence.
