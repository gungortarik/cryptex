# M2 Slice 3 — Integrity Primitives

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`
**Prior:** schema validation alignment `f3cb7f92941a861a72805701e8e65f56f75a233a`

## Scope

- RFC 8785 JCS canonicalization of signable events via `serde_json_canonicalizer = 0.3.2`.
- BLAKE3-256 content hash with domain label `cryptex-event-content-v1` + `0x00` separator.
- Ed25519 signatures over domain-separated content-hash bytes (`cryptex-event-sig-v1`).
- `signing_key_id` derivation: first 16 bytes of SHA-256(Ed25519 public key), lowercase hex.
- Integrity-chain helpers: first-event zero `prev_content_hash`, subsequent link equality.
- Duplicate JSON object member rejection (ADR-0016 section 4.1).
- Typed `EventError::Integrity` failures; no custom crypto.

## Non-goals (deferred)

- Signing-key SecureStore lifecycle / public metadata persistence
- Segment append/seal/quarantine I/O
- cryptexd writer API
- GitHub Actions

## Local verification (Windows)

- `cargo fmt -p cryptex-events -p cryptex-storage` — applied
- `cargo test -p cryptex-events -p cryptex-storage --locked` — PASS (29 events + 3 storage)
- Clippy still blocked locally by Smart App Control (4551); required at final CI gate
