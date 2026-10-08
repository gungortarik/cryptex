# M2 Slice 5b — Storage/Signing Durability Hardening

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`

## Scope

- Required file/directory sync errors propagate as typed `Durability` / IO failures (no ignored `let _ = sync`).
- Unix directory sync for quarantine creation, `.open` recovery/truncation, sealing, and signing-key metadata placement.
- Streaming `.open` recovery + `quarantine_file_range` (no whole-pack/`suffix` `fs::read`); per-frame body allocation capped by ADR max.
- Unix/Windows seal retry: both `.open`+`.seg` reconcile only when byte-identical; divergent pairs fail closed.
- Signing metadata `created_at` is RFC 3339 UTC on create and validated on load (no silent key replace).

## Local verification (Windows)

- `cargo test -p cryptex-storage --locked` — PASS (20 tests)
- `cargo test -p cryptexd --lib event_signing_key --locked` — PASS (10 tests)
