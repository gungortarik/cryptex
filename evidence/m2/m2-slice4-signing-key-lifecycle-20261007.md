# M2 Slice 4 — Daemon-Owned Signing-Key Lifecycle

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`
**Prior:** integrity known-answer vectors

## Scope

- `cryptexd::event_signing_key::ensure_event_signing_key` crash-safe init (ADR-0016 section 6).
- SecureStore account `device-event-signing-seed:{device_id}`; public metadata under `identity/devices/<device_id_hex>/event_signing_key.json`.
- Exclusive no-replace metadata create; seed-only recovery; fail-closed mismatch / metadata-without-seed / foreign key-id.
- Concurrent ensure converges on one key; runtime fixtures under OS temp (outside repo).

## Non-goals

- Segment append/seal/quarantine I/O
- cryptexd writer API wiring to sequences
- GitHub Actions

## Local verification (Windows)

- `cargo test -p cryptexd event_signing_key -p cryptex-storage --locked` — PASS (8 signing-key + storage path tests)
