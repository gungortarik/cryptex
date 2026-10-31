# M2 Slice 2 — Event Domain Types + Schema Generation

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`
**Prior:** framing body_len enforcement `ebeb5c02c9494f270ca78b68a0da0c228626f219`

## Scope

- Production envelope types in `cryptex-events` (`SignableEvent`, `AcceptedEvent`, `memory.note` only).
- `device_seq` as canonical decimal JSON string with strict full-`u64` parse (ADR-0016 §3.1.1).
- JSON Schema generation via `schemars = 1.2.2` into `packages/schemas/cryptex-event-v1.schema.json`.
- TypeScript bindings via pinned `json-schema-to-typescript = 16.0.0` into `packages/schemas/cryptex-event-v1.ts`.
- Drift checks: Rust schema test + `pnpm --filter @cryptex/schemas check:drift`.

## Non-goals (deferred)

- JCS canonicalization / hash / signature productization
- Segment append/seal/quarantine I/O
- cryptexd writer API
- GitHub Actions

## Local verification (Windows)

- `cargo fmt -p cryptex-events -p cryptex-storage -- --check` — PASS
- `cargo test -p cryptex-events --locked` — PASS (16 tests)
- `cargo test -p cryptex-storage --locked` — PASS
- `pnpm --filter @cryptex/schemas check:drift` — PASS
- Clippy still blocked locally by Smart App Control (4551); required at final CI gate
