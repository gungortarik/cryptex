# Gate C procedure — macOS product SecureStore

**Status:** VERIFIED — see `evidence/m1/gate-c-product-secure-store-macos-20261006.md` (product `MacosKeychainStore` on macOS host at SHA `610733f8f911c67f341694c4333e34070b706d50`). Windows Gate C remains separate evidence.

**Coordination (2026-10-06):** Windows Gate C is recorded VERIFIED in `evidence/m1/gate-c-product-secure-store-windows-20261006.md`. macOS Gate C is recorded VERIFIED in `evidence/m1/gate-c-product-secure-store-macos-20261006.md`. Neither host’s result may be inferred from the other. GitHub Actions remain deferred.

**Authority:** ADR-0014 section 8.2 (gate C); ADR-0014 decision addendum (P1 pins). Gate A spike evidence is **not** a substitute.

**Distinct from Gate A:** Gate A compared K2 vs P1 in a product-separated spike (Keychain prompt not observed on that run — run-specific caveat). Gate C verifies the **accepted P1 product path** under `cryptexd` (`MacosKeychainStore`).

## Preconditions

- Branch/SHA under test recorded (include `cryptexd` product adapter commits).
- Product pin: `security-framework = 3.7.0`.
- Runtime data outside the Git repository.
- Synthetic secrets only; never real Recovery Kit material.
- Physical macOS host (arm64 or x86_64 as available); Windows results do **not** count.
- No GitHub Actions required for this procedure (local physical run).
- Prefer `cargo test -p cryptexd --lib ...` filters.

## Adapter under test

- Type: `cryptexd::MacosKeychainStore` (Security framework generic password APIs).
- Service/account labels: `ServiceLabel` / `AccountLabel` (Dev environment for lab runs).

## Physical matrix (must all PASS on the macOS host)

| # | Case | How to exercise | Pass criteria |
|---|---|---|---|
| 1 | Store | `put` synthetic bytes via product adapter | Keychain write succeeds; no panic |
| 2 | Read | `get` same service/account | Exact byte equality with stored secret |
| 3 | Delete | `delete` then `get` | `SecureStoreError::Missing` |
| 4 | Missing-item | `get` **and** `delete` never-written account | Typed `Missing` |
| 5 | Redacted errors | Format `Display`/`Debug` of failures while secret known | `payload_bytes_recoverable(secret, surface)` is false |
| 6 | Auth / interaction classification | If Keychain denies or prompts, surface stays typed `Platform` (or documented Missing) without secret bytes | No secret recovery in logs/fixtures; prompt observation recorded as run-specific |

## Suggested local commands (macOS)

```text
cargo test -p cryptexd --lib --locked secure_store::macos -- --nocapture
cargo test -p cryptexd --lib --locked identity_runtime -- --nocapture
```

Add a focused never-written `delete`/`get` Missing check under `secure_store::macos` if not already present before claiming matrix row 4.

## Keychain UI note

- Record whether a Keychain access prompt appeared during **this** Gate C run.
- Absence of a prompt is **not** proof prompts never occur under other codesign/ACL/headless conditions (same caveat as Gate A).

## Evidence artifact

Recorded: `evidence/m1/gate-c-product-secure-store-macos-20261006.md` with host version/build/arch, tested SHA, commands, matrix, Keychain prompt observation, cleanup, and **Gate C macOS product verification: VERIFIED** for this adapter/run.

## Explicit non-claims

- Gate A macOS spike does **not** satisfy Gate C by itself.
- Windows Gate C VERIFIED does **not** imply macOS Gate C (and the reverse is also true).
- No Keychain prompt in one short Mac product run does not prove prompts never occur.
