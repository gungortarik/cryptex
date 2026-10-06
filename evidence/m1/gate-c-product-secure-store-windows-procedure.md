# Gate C procedure — Windows product SecureStore (NOT VERIFIED)

**Status:** WAITING — procedure only. Do **not** mark Windows Gate C VERIFIED until this matrix has been physically exercised against the **product** `cryptexd` adapter on a Windows host.

**Authority:** ADR-0014 §8.2 (gate C); ADR-0014 decision addendum (P1 pins). Gate A spike evidence is **not** a substitute.

**Distinct from Gate A:** Gate A compared K2 vs P1 in a product-separated spike. Gate C verifies the **accepted P1 product path** under `cryptexd` (`WindowsCredentialStore`).

## Preconditions

- Branch/SHA under test recorded (include `cryptexd` product adapter commits).
- Product pins: `windows-sys = 0.61.2`, `widestring = 1.2.1`.
- Runtime data outside the Git repository (app-data / temp root per `cryptexd` path rules).
- Synthetic secrets only; never real Recovery Kit material.
- No GitHub Actions required for this procedure (local physical run).

## Adapter under test

- Type: `cryptexd::WindowsCredentialStore` (CredWriteW / CredReadW / CredDeleteW).
- Service/account labels: `ServiceLabel` / `AccountLabel` (Dev environment for lab runs).
- Target name shape: `{service}/{account}`.

## Physical matrix (must all PASS on the Windows host)

| # | Case | How to exercise | Pass criteria |
|---|---|---|---|
| 1 | Store | `put` synthetic bytes via product adapter | CredWrite succeeds; no panic |
| 2 | Read | `get` same service/account | Exact byte equality with stored secret |
| 3 | Delete | `delete` then `get` | `SecureStoreError::Missing` |
| 4 | Missing-item | `get`/`delete` never-written account | Typed `Missing` (not opaque platform string with payload) |
| 5 | Redacted errors | Format `Display`/`Debug` of failures while secret known | `payload_bytes_recoverable(secret, surface)` is false |
| 6 | Inconsistent blob guard | Covered by unit test `copy_credential_blob_rejects_size_with_null_pointer` on this host | Test PASS (documents fail-closed CredRead integrity) |
| 7 | CredFree discipline | Covered by product `get` RAII guard + integration `put/get/delete` | No leak of CredRead allocation on success or error paths exercised above |

## Suggested local commands (Windows)

Record exact command lines and exit codes in the evidence file.

```text
cargo test -p cryptexd --locked secure_store::windows -- --nocapture
cargo test -p cryptexd --locked identity_runtime -- --nocapture
cargo clippy -p cryptexd --all-targets --locked -- -D warnings
```

Optional focused filter:

```text
cargo test -p cryptexd --locked windows_store_read_delete_missing_redaction_matrix -- --nocapture
```

## Evidence artifact (fill only after physical run)

Create `evidence/m1/gate-c-product-secure-store-windows-YYYYMMDD.md` with:

- Host OS build, arch, date/time (local)
- Git SHA tested
- Command lines + exit codes
- Matrix table with PASS/FAIL per row (no silent WAITING→PASS)
- Explicit statement: **Gate C Windows product verification: VERIFIED** only if all required rows PASS
- Any Cred Manager UI / policy notes

## Explicit non-claims

- This procedure file does **not** verify Windows Gate C.
- Gate A Windows spike does **not** satisfy Gate C.
- macOS Gate C is a separate procedure and must not be inferred from Windows results.
