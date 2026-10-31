# M2 Slice 1 — Workspace Scaffolding

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`
**ADR-0016 acceptance tip:** `5cc89b709dcf236698c3253b8582c873de7c7f0f`

## Scope

- Wire `crates/events` (`cryptex-events`) and `crates/storage` (`cryptex-storage`) into the Cargo workspace.
- Expose ADR-0016 framing/limit constants and typed errors in `cryptex-events`.
- Expose canonical/quarantine path helpers in `cryptex-storage` (no runtime I/O; no repo-local data).

## Non-goals (deferred)

- Event envelope / JCS / schema generation
- Hash/signature libraries and pins
- Segment append/seal/quarantine durability I/O
- cryptexd writer API
- GitHub Actions

## Local verification (2026-10-07, Windows)

- `cargo fmt -p cryptex-events -p cryptex-storage -- --check` — PASS
- `cargo test -p cryptex-events -p cryptex-storage --locked` — PASS (5 + 3 tests)
- `cargo clippy -p cryptex-events -p cryptex-storage --all-targets --locked -- -D warnings` — **NOT RUN locally**: Smart App Control blocked `cargo-clippy` (os error 4551), same class of host restriction recorded during M1. Strict Clippy remains required on CI at the final M2 gate; no Actions dispatched for this routine slice.
