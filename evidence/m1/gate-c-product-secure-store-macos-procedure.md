# Gate C procedure — macOS product SecureStore (NOT VERIFIED)

**Status:** WAITING — procedure only. Do **not** mark macOS Gate C VERIFIED until this matrix has been physically exercised against the **product** `cryptexd` adapter on a macOS host.

**Authority:** ADR-0014 §8.2 (gate C); ADR-0014 decision addendum (P1 pins). Gate A spike evidence is **not** a substitute.

**Distinct from Gate A:** Gate A compared K2 vs P1 in a product-separated spike (Keychain prompt not observed on that run — run-specific caveat). Gate C verifies the **accepted P1 product path** under `cryptexd` (`MacosKeychainStore`).

## Preconditions

- Branch/SHA under test recorded (include `cryptexd` product adapter commits).
- Product pin: `security-framework = 3.7.0`.
- Runtime data outside the Git repository.
- Synthetic secrets only; never real Recovery Kit material.
- Physical macOS host (arm64 or x86_64 as available); Windows CI/host results do **not** count.
- No GitHub Actions required for this procedure (local physical run).

## Adapter under test

- Type: `cryptexd::MacosKeychainStore` (Security framework generic password APIs).
- Service/account labels: `ServiceLabel` / `AccountLabel` (Dev environment for lab runs).

## Physical matrix (must all PASS on the macOS host)

| # | Case | How to exercise | Pass criteria |
|---|---|---|---|
| 1 | Store | `put` synthetic bytes via product adapter | Keychain write succeeds; no panic |
| 2 | Read | `get` same service/account | Exact byte equality with stored secret |
| 3 | Delete | `delete` then `get` | `SecureStoreError::Missing` |
| 4 | Missing-item | `get`/`delete` never-written account | Typed `Missing` |
| 5 | Redacted errors | Format `Display`/`Debug` of failures while secret known | `payload_bytes_recoverable(secret, surface)` is false |
| 6 | Auth / interaction classification | If Keychain denies or prompts, surface stays typed `Platform` (or documented Missing) without secret bytes | No secret recovery in logs/fixtures; prompt observation recorded as run-specific |

## Suggested local commands (macOS)

Record exact command lines and exit codes in the evidence file.

```text
cargo test -p cryptexd --locked secure_store::macos -- --nocapture
cargo test -p cryptexd --locked identity_runtime -- --nocapture
cargo clippy -p cryptexd --all-targets --locked -- -D warnings
```

Optional focused filter:

```text
cargo test -p cryptexd --locked macos -- --nocapture
```

## Keychain UI note

- Record whether a Keychain access prompt appeared during **this** Gate C run.
- Absence of a prompt is **not** proof prompts never occur under other codesign/ACL/headless conditions (same caveat as Gate A).

## Evidence artifact (fill only after physical run)

Create `evidence/m1/gate-c-product-secure-store-macos-YYYYMMDD.md` with:

- Host macOS version, build, arch, date/time (local)
- Git SHA tested
- Command lines + exit codes
- Matrix table with PASS/FAIL per row (no silent WAITING→PASS)
- Keychain prompt observed: yes/no (run-specific)
- Explicit statement: **Gate C macOS product verification: VERIFIED** only if all required rows PASS

## Explicit non-claims

- This procedure file does **not** verify macOS Gate C.
- Gate A macOS spike does **not** satisfy Gate C.
- Windows Gate C is a separate procedure and must not be inferred from macOS results.
