# M4 Slice 2 — Typed v2 payload and schema candidate

**Status:** Windows + macOS local verification PASS on the same code commit.
This does not enable production v2 writing or complete M4.

| Field | Value |
|---|---|
| Branch | `m4/knowledge-engine` |
| Code commit | `70dba01ff8fdb4018f6eb9be55d41943d300e90e` |
| Windows host | x86_64; Cargo target under `%LOCALAPPDATA%\Cryptex-Dev\cargo-target-m4` |
| macOS host | 27.0.1 (26A434) / arm64; detached HEAD, clean |
| Actions | Not dispatched |

## Delivered

- Strict typed v2 `knowledge.assertion` payloads for the eight ADR-0017 kinds;
  exact enums/fields, unknown-field rejection, UUID/device/sequence/time checks,
  authorization-confidence rules, decimal/date/NFC validation, and UTF-8 byte
  limits.
- Known valid payloads classify as `VerifiedV2Supported`; malformed known
  payloads fail validation, while cryptographically verified unknown event or
  payload versions remain `VerifiedUnsupported` with original bytes preserved.
- The v1 reader still delegates to the unchanged frozen validator. V1 schema and
  TypeScript artifacts are byte-identical to the parent commit.
- Added separate Rust-generated v2 JSON Schema and TypeScript bindings. The
  only new runtime dependency is pinned `unicode-normalization = 0.1.24` for the
  ADR's Unicode 16 NFC rule.
- No production signing, v2 append, activation, reducer, SQLite migration, or
  future-milestone code was added. Canonical writer only chain-scans typed v2
  and continues to block M3 retrieval of v2 records pending later M4 safety and
  materialization work.

## Windows checks

| Command | Result |
|---|---|
| `cargo test --locked -p cryptex-events` | **56/56 PASS** |
| `cargo test --locked -p cryptex-storage` | **39/39 PASS** |
| `cargo test --locked -p cryptexd --lib canonical_writer:: -- --test-threads=8` | **21/21 PASS** |
| `cargo clippy --locked -p cryptex-events -p cryptex-storage -p cryptexd --all-targets -- -D warnings` | **PASS** |
| `cargo fmt --all -- --check` | **PASS** |
| `pnpm --filter @cryptex/schemas check:drift` | **PASS** (v1 and v2) |
| `git diff --check` | **PASS** |
| V1 schema / TS artifact comparison to parent | **unchanged** |

The independent read-only test runner repeated events 56/56, canonical writer
21/21, formatting, and TypeScript drift successfully. The security reviewer
reported no blocking findings for this slice. No GitHub Actions workflow ran.

## macOS checks

The owner ran the checks at the exact code commit above; recorded HEAD was
`70dba01ff8fdb4018f6eb9be55d41943d300e90e`.

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | **PASS**, exit 0 |
| `cargo test --locked -p cryptex-events` | **56/56 PASS**, exit 0 |
| `cargo test --locked -p cryptex-storage` | **38/38 PASS**, exit 0 |
| `cargo test --locked -p cryptexd --lib canonical_writer:: -- --test-threads=8` | **21/21 PASS**, exit 0 |
| `cargo clippy --locked -p cryptex-events -p cryptex-storage -p cryptexd --all-targets -- -D warnings` | **PASS**, exit 0 |
| `pnpm --filter @cryptex/schemas check:drift` | **PASS**, v1 + v2 match, exit 0 |

Thus Slice 2's Windows/macOS local verification is complete for this code SHA.
GitHub Actions remain reserved for the single required final reviewed M4
candidate run.

## Remaining gates

- Future M4 slices: deterministic reducer, derived SQLite migration/rebuild,
  daemon/retrieval safety enforcement, and final verification/security/DoD.
- Production v2 activation remains a separate closed gate. Unsupported event
  handling and retrieval safety must be implemented before M4 completion.
