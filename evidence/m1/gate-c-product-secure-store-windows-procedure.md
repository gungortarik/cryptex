# Gate C procedure — Windows product SecureStore

**Status:** VERIFIED — see `evidence/m1/gate-c-product-secure-store-windows-20261006.md` (product `WindowsCredentialStore` on Windows host). macOS Gate C remains separate and WAITING.

**Authority:** ADR-0014 section 8.2 (gate C); ADR-0014 decision addendum (P1 pins). Gate A spike evidence is **not** a substitute.

**Distinct from Gate A:** Gate A compared K2 vs P1 in a product-separated spike. Gate C verifies the **accepted P1 product path** under `cryptexd` (`WindowsCredentialStore`).

## Preconditions

- Branch/SHA under test recorded (include `cryptexd` product adapter commits).
- Product pins: `windows-sys = 0.61.2`, `widestring = 1.2.1`.
- Runtime data outside the Git repository (app-data / temp root per `cryptexd` path rules).
- Synthetic secrets only; never real Recovery Kit material.
- No GitHub Actions required for this procedure (local physical run).
- Prefer `cargo test -p cryptexd --lib ...` so unrelated integration binaries are not required for the matrix.

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
| 4 | Missing-item | `get` **and** `delete` never-written account (`windows_delete_never_written_account_is_typed_missing`) | Typed `Missing` (not opaque platform string with payload) |
| 5 | Redacted errors | Format `Display`/`Debug` of failures while secret known | `payload_bytes_recoverable(secret, surface)` is false |
| 6 | Inconsistent blob guard | Covered by unit test `copy_credential_blob_rejects_size_with_null_pointer` on this host | Test PASS (documents fail-closed CredRead integrity) |
| 7 | CredFree discipline | Covered by product `get` RAII guard + integration `put/get/delete` | No leak of CredRead allocation on success or error paths exercised above |

## Suggested local commands (Windows)

Record exact command lines and exit codes in the evidence file. Use `--lib` filters:

```text
cargo test -p cryptexd --lib --locked secure_store::windows -- --nocapture
cargo test -p cryptexd --lib --locked windows_delete_never_written_account_is_typed_missing -- --nocapture
cargo test -p cryptexd --lib --locked identity_runtime -- --nocapture
```

Do **not** treat a Smart App Control block of an unrelated binary (for example `mode_b_supervisor`, OS error 4551) as a Gate C pass.

## Evidence artifact

Verified run: `evidence/m1/gate-c-product-secure-store-windows-20261006.md`.

## Explicit non-claims

- Windows VERIFIED does **not** imply macOS Gate C.
- Gate A Windows spike does **not** satisfy Gate C by itself without the product matrix above.
