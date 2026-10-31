# M2 Slice 6 — cryptexd Canonical Writer API

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`
**Prior:** durability hardening `274b114` / tip `a450d38`

## Scope

- `cryptexd::canonical_writer::append_memory_note` — sole product write path for `memory.note`.
- Ensures signing key, recovers active `.open` with hash/signature validation, allocates `device_seq`, signs accepted JCS body, appends framed record, updates tip, seals at configurable threshold.
- Refuses runtime roots inside the repository. Fixtures under OS temp.

## Non-goals

- Full schema-version matrix / import quarantine paths beyond recovery validator
- GitHub Actions / merge / M2 complete claim

## Local verification (Windows)

- `cargo test -p cryptex-storage --locked` — PASS (20)
- `cargo test -p cryptexd --lib canonical_writer --locked` — PASS (3)
- `cargo test -p cryptexd --lib event_signing_key --locked` — PASS (10)
