# Gate C evidence — Windows product SecureStore

**Status: VERIFIED** (Windows product adapter only)

**Date/time (local):** 2026-10-06T10:22-04:00 (procedure run); never-written delete check completed same session  
**Host:** Microsoft Windows NT 10.0.26300.0, AMD64  
**Branch:** `m1/brain-device-identity`  
**Authority:** ADR-0014 §8.2 gate C; procedure `evidence/m1/gate-c-product-secure-store-windows-procedure.md`  
**Adapter:** `cryptexd::WindowsCredentialStore` (P1 Cred*)  
**Product pins:** `windows-sys = 0.61.2`, `widestring = 1.2.1`  
**GitHub Actions:** not run (deferred)

## SHAs

| Role | SHA |
|---|---|
| Procedure + prior lib matrix baseline | `1a39431df6874a7e9d17fbe72a745846985c7250` |
| Windows Gate C VERIFIED tip (adds never-written delete check + this evidence) | `e7b83cfb95b565f5eb30d7e4a01c24485122f7b3` |

Gate A spike evidence is not used as a substitute for this product verification.

## Commands executed (lib-only; exact)

These commands use `--lib` so integration binaries (including `mode_b_supervisor`) are not built/run.

### Baseline at `1a39431` (owner-reported; incomplete for matrix row 4 delete path)

```text
cargo test -p cryptexd --lib --locked secure_store::windows -- --nocapture
→ ok. 4 passed; 0 failed
exit 0

cargo test -p cryptexd --lib --locked identity_runtime -- --nocapture
→ ok. 3 passed; 0 failed
exit 0
```

### Broader filtered Cargo (NOT PASSING — do not cite as green)

A broader filtered Cargo invocation that pulled in the unrelated `mode_b_supervisor` test binary was **blocked** when Smart App Control rejected that binary (OS error 4551). That command is **not** reported as passing and is not part of the VERIFIED claim.

### Completion run (never-written delete + full windows module + identity)

```text
cargo test -p cryptexd --lib --locked windows_delete_never_written_account_is_typed_missing -- --nocapture
→ ok. 1 passed; 0 failed
exit 0

cargo test -p cryptexd --lib --locked secure_store::windows -- --nocapture
→ ok. 5 passed; 0 failed
  - copy_credential_blob_allows_empty_and_copies_bytes
  - copy_credential_blob_rejects_size_with_null_pointer
  - map_not_found_is_missing_not_platform
  - windows_delete_never_written_account_is_typed_missing
  - windows_store_read_delete_missing_redaction_matrix
exit 0

cargo test -p cryptexd --lib --locked identity_runtime -- --nocapture
→ ok. 3 passed; 0 failed
exit 0
```

## Complete physical product matrix

| # | Case | Result | Evidence |
|---|---|---|---|
| 1 | Store | **PASS** | `windows_store_read_delete_missing_redaction_matrix` CredWrite/`put` |
| 2 | Read | **PASS** | same test; byte-equal `get` |
| 3 | Delete | **PASS** | same test; `delete` then `get` → `Missing` |
| 4 | Missing-item (`get` **and** `delete` never-written) | **PASS** | `windows_delete_never_written_account_is_typed_missing` (typed `Missing` on both); post-delete `get` also covered in matrix test |
| 5 | Redacted errors | **PASS** | matrix test + never-written delete Display/Debug vs `payload_bytes_recoverable` |
| 6 | Inconsistent blob guard | **PASS** | `copy_credential_blob_rejects_size_with_null_pointer` |
| 7 | CredFree discipline | **PASS** | product `get` RAII `CredReadGuard` exercised via put/get/delete paths above |

## Verdict

**Gate C Windows product verification: VERIFIED** at `e7b83cfb95b565f5eb30d7e4a01c24485122f7b3`.

## Explicit non-claims

- macOS Gate C is **not** verified by this evidence.
- Gate A Windows spike is not this verification.
- Unrelated Cargo binaries blocked by Smart App Control are not treated as test failures or passes for Gate C.
