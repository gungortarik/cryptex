# ADR-0014 — OS secure-store decision addendum

**Status:** ACCEPTED

**Date drafted:** 2026-10-06

**Date accepted:** 2026-10-06

**Parent:** `docs/adr/ADR-0014-os-secure-store-integration.md` (**ACCEPTED**)

**Milestone:** M1 — Brain + Device Identity

**Authority:** Gate A evidence in `evidence/m1/secure-store-spike-windows-20261005.md` and `evidence/m1/secure-store-spike-macos-20261006.md`; parent ADR §§2, 4, 6, 8

This addendum records the **accepted** gate B decision: **P1 (direct platform APIs)**. It authorizes product `cryptexd` SecureStore implementation under the parent ADR. Product pins below are authoritative. **K2 remains unselected.** This acceptance does **not** claim Gate C product verification. Keychain UI prompts were **not observed during the macOS Gate A run**; that caveat remains run-specific.

## 1. Decision

**Accepted: candidate P1 — direct platform APIs** behind a daemon-owned Cryptex `SecureStore` trait.

### Why P1 (evidence-backed)

| Finding | Windows | macOS |
|---|---|---|
| P1 store / read / delete / missing-item | PASS | PASS |
| P1 error-redaction (typed errors; UTF-8 + Debug/hex checked) | PASS | PASS |
| K2 store / read / delete / missing-item | PASS | PASS |
| K2 overall error-redaction | **FAIL** | **FAIL** |

K2 fails overall error-redaction on **both** first-class platforms because `keyring_core::Error::BadEncoding(Vec<u8>)` **Debug** recovers secret bytes as a decimal list, even when Display and missing-item (`NoEntry`) surfaces are safe. P1’s typed adapters never carry secret payloads in Display/Debug and passed the same matrix on both hosts.

K1 (high-level `keyring` facade) was not required for the selection and remains non-default per the parent ADR.

## 2. Proposed product API surface (after acceptance only)

Only after owner acceptance of ADR-0014 (gate B) may product code implement adapters under `cryptexd` authority. The intended native surfaces are:

### 2.1 Windows — Credential Manager

- APIs: `CredWriteW`, `CredReadW`, `CredDeleteW` (generic credentials)
- Persistence used in Gate A: `CRED_PERSIST_LOCAL_MACHINE`
- Target naming shape used in the spike: `{service}/{account}` with Cryptex / Cryptex-Dev isolation policy from the parent ADR
- Binding exercised in Gate A: `windows-sys = 0.61.2` (`Win32_Foundation`, `Win32_Security_Credentials`) plus `widestring` for UTF-16

### 2.2 macOS — Keychain Services (generic password)

- APIs via `security-framework` 3.7.0:
  - `set_generic_password(service, account, secret)`
  - `generic_password(PasswordOptions::new_generic_password(service, account))`
  - `delete_generic_password(service, account)`
- Public error type: `security_framework::base::Error` with `.code()` → OSStatus
- Keychain UI prompts: **not observed during the macOS Gate A run**; that is run-specific and is **not** proof that prompts never occur under other codesign/ACL/headless conditions

### 2.3 Shared Cryptex policy (unchanged from parent)

1. Only `cryptexd` (and tightly scoped harnesses) may read/write device private material.
2. Desktop / CLI / MCP / UI never receive raw private keys.
3. Service/account naming isolates `Cryptex` vs `Cryptex-Dev` without embedding secret bytes in names.
4. Hardware-backed / non-exportable keys remain **not** a V1 requirement.

## 3. Typed sanitized error mapping

Product adapters (when authorized) must classify platform failures without attaching secret payloads:

| Condition | Typed outcome | Notes |
|---|---|---|
| Windows `CredReadW` / `CredDeleteW` → `ERROR_NOT_FOUND` | Missing item | Only this status maps to missing |
| macOS OSStatus `-25300` (`errSecItemNotFound`) | Missing item | Only this status maps to missing |
| Any other Windows `GetLastError` / macOS OSStatus (e.g. auth failed, interaction not allowed, missing entitlement) | Sanitized platform error | Numeric status / op label only — never secret bytes, never raw password strings |
| Display / Debug of Cryptex secure-store errors | Must not recover secrets | UTF-8 text and Debug/hex byte-list recovery are both treated as exposure |

Gate A unit tests cover Keychain classification (missing vs auth-failed / interaction-not-allowed / missing-entitlement) and byte-aware redaction detection in the standalone spike package.

## 4. Evidence-backed product pins

These versions were exercised in the product-separated spike at tested SHA `4f4ba2784cbb3d305a8a462aa9b7286b9a8e770d` (macOS evidence) and the Windows Gate A runs. **On acceptance they become the product pins for P1 adapters under `cryptexd`.**

| Role | Product pin |
|---|---|
| P1 Windows bindings | `windows-sys = 0.61.2`, `widestring = 1.2.1` |
| P1 macOS bindings | `security-framework = 3.7.0` |
| K2 | **Unselected** — spike comparison only (`keyring-core` / native keyring stores must not be added to product) |
| High-level `keyring` | **Unselected** |
| Spike location (historical) | `tools/m1-secure-store-spike/` (standalone; not a product member) |

## 5. Alternatives not selected

| ID | Outcome | Reason |
|---|---|---|
| K2 | Not recommended | Full matrix store/read/delete/missing PASS, but overall error-redaction **FAIL** on Windows and macOS due to payload-bearing `BadEncoding` Debug |
| K1 | Not selected | Weaker audit/control; parent ADR rejects high-level `keyring` as default |
| X1 / X2 / X3 | Rejected | Per parent ADR (not OS secure storage / not V1 HSM / not sync-set secrets) |

## 6. Explicit non-claims

- Gate A ≠ Gate C: both-OS **product** physical verification remains an M1 completion requirement and is **not** claimed by this acceptance.
- Absence of a Keychain prompt in one Mac spike run does not prove prompts never occur under other codesign/ACL/headless conditions.
- K2 and high-level `keyring` must not be introduced as product dependencies.

## 7. Acceptance

The owner accepted ADR-0014 and this addendum on **2026-10-06**, selecting **P1** with the product pins in §4. Product SecureStore work may proceed under `cryptexd`. Gate C both-OS product physical verification remains a separate M1 completion requirement.

## 8. References

- Parent: `docs/adr/ADR-0014-os-secure-store-integration.md`
- `evidence/m1/secure-store-spike-windows-20261005.md`
- `evidence/m1/secure-store-spike-macos-20261006.md`
- `docs/08_SECURITY_DEVICE_TRUST_AND_RECOVERY_ROOT.md`
- `docs/15_PLATFORM_MACOS_WINDOWS.md`
- `docs/milestones/M1_BRAIN_DEVICE_IDENTITY.md`
