# M2 Slice 5 — Segment Append/Seal + Recovery/Quarantine I/O

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`

## Scope

- Active `.open` append with exclusive lock + `sync_all` (`cryptex-storage::append_frame`).
- Seal rename `.open` → `.seg` exclusive/no-replace (`seal_active_segment`).
- Quarantine exact rejected bytes + `quarantine.json` (ADR-0016 section 8); collision suffix without overwrite.
- `.open` recovery: quarantine-before-truncate; fail closed if quarantine cannot be created.
- Concurrent appends preserve complete frames. Runtime fixtures under OS temp (outside repo).

## Non-goals

- cryptexd writer API / device_seq allocation wiring
- Full schema/JCS/hash/signature validator wiring into recovery (pluggable `FrameBodyValidator` provided)
- GitHub Actions

## Local verification (Windows)

- `cargo test -p cryptex-storage --locked` — PASS (12 tests)
