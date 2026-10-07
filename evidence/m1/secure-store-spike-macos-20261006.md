# ADR-0014 secure-store evaluation spike — macOS results

**Date:** 2026-10-06
**Gate:** A (product-separated evaluation spike)
**Branch:** `m1/brain-device-identity`
**Tested SHA:** `4f4ba2784cbb3d305a8a462aa9b7286b9a8e770d`
**Host:** macOS 27.0.1 (build 26A434), arm64
**Windows counterpart:** `evidence/m1/secure-store-spike-windows-20261005.md`
**ADR-0014 status after this evidence:** remains **PROPOSED** (pending owner review)

## Scope and constraints

- Compared **K2** (`keyring-core` + explicit `apple-native-keyring-store` keychain provider) and **P1** (direct `security-framework` 3.7.0 APIs).
- Spike package: `tools/m1-secure-store-spike/` (standalone `[workspace]`; not a product workspace member).
- No product dependency pins; no `cryptexd`/production secure-store backend.
- Synthetic throwaway secrets only; secret bytes were not printed and are not recorded here.
- Cleanup: spike deletes created Keychain items after each candidate run; leftover cleanup confirmed for this run.

## Candidate versions (spike-local pins at tested SHA)

| Component | Version / note |
|---|---|
| `keyring-core` | `1.0.0` |
| `apple-native-keyring-store` | `1.0.2` (`keychain` feature) |
| P1 macOS | `security-framework = 3.7.0` via `generic_password(PasswordOptions::new_generic_password(..))`, `set_generic_password`, `delete_generic_password` |
| Missing-item mapping | Only OSStatus `-25300` (`errSecItemNotFound`) → Missing |

## Local checks on this host

| Check | Result |
|---|---|
| `cargo test --manifest-path tools/m1-secure-store-spike/Cargo.toml` | **12/12 passed** |
| `cargo run --manifest-path tools/m1-secure-store-spike/Cargo.toml --release` | operator-reported **exit 0**; matrices below |
| Secret bytes in stdout/stderr/evidence | **none observed** |
| Keychain UI prompt | **not observed during this run** (not proof that prompts never occur under other codesign/ACL/headless conditions) |
| Cleanup | **confirmed** — spike items removed after matrix; no retained `Cryptex-Dev-M1-Spike` / `synthetic-` leftovers from this run |

## Matrix results (macOS)

Legend: **PASS** / **FAIL** = measured on this Mac host. Windows cells were not re-executed here (see Windows evidence).

| Candidate | Platform | store | read | delete | missing-item | error-redaction |
|---|---|---|---|---|---|---|
| K2 | macOS | PASS | PASS | PASS | PASS | **FAIL** |
| P1 | macOS | PASS | PASS | PASS | PASS | PASS |

### macOS K2 observations

- Provider: `apple_native_keyring_store::keychain::Store` set as default; `Entry::set_secret` / `get_secret` / `delete_credential`.
- Missing item: `keyring_core::Error::NoEntry` after delete.
- Error-redaction assessment: `missing_item_errors=SAFE`; `payload_bearing_upstream=EXPOSED` (`Error::BadEncoding` Debug recovers secret bytes as a decimal list); **overall=FAIL**.
- Same class of limitation as Windows Gate A for K2.

### macOS P1 observations

- Direct Security Framework generic-password APIs (`set_generic_password` / `generic_password` + `PasswordOptions` / `delete_generic_password`).
- Missing-item uses only `-25300`; other statuses would remain sanitized platform errors (classification covered by unit tests).
- Typed P1 errors omit secret payloads (UTF-8 and Debug/hex forms checked) → error-redaction **PASS**.

## Cross-platform Gate A summary

| Candidate | Windows | macOS |
|---|---|---|
| K2 store/read/delete/missing | PASS | PASS |
| K2 error-redaction | FAIL (BadEncoding Debug) | FAIL (BadEncoding Debug) |
| P1 store/read/delete/missing | PASS | PASS |
| P1 error-redaction | PASS | PASS |

## Recommendation input (spike, not acceptance)

Gate A comparison now has recorded evidence on **both** first-class platforms. **Provisional preference: P1** (direct platform APIs), because P1 passed the full matrix on Windows and macOS while K2 failed overall error-redaction on both hosts due to payload-bearing upstream `Debug` exposure. Owner review still required before ADR acceptance (gate B). No product pins in this slice.

## Non-claims

- No product `cryptexd` SecureStore verification (Gate C still open).
- No ADR-0014 acceptance.
- Keychain prompt absence is **run-specific** (“not observed during this run”), not a permanent guarantee.
- No GitHub Actions run for this evidence.
