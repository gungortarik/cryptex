# Gate C evidence — macOS product SecureStore

**Status: VERIFIED** (macOS product adapter only; this run)

**Date/time (UTC):** 2026-10-06T15:10:23Z–15:10:24Z (host-run report)
**Host:** macOS 27.0.1 (Build 26A434), arm64
**Branch:** `m1/brain-device-identity`
**Tested SHA:** `610733f8f911c67f341694c4333e34070b706d50`
**Authority:** ADR-0014 §8.2 gate C; procedure `evidence/m1/gate-c-product-secure-store-macos-procedure.md`
**Adapter:** `cryptexd::MacosKeychainStore` (P1 Security Framework generic password)
**Product pin:** `security-framework = 3.7.0`
**GitHub Actions:** not run (deferred)

## Commands executed (lib-only; exact)

```text
cargo test -p cryptexd --lib --locked secure_store::macos -- --nocapture
→ ok. 2 passed; 0 failed; 0 ignored; 0 measured; 42 filtered out; finished in 0.15s
  - secure_store::macos::tests::macos_delete_never_written_account_is_typed_missing ... ok
  - secure_store::macos::tests::macos_store_read_delete_missing_redaction_matrix ... ok

cargo test -p cryptexd --lib --locked identity_runtime -- --nocapture
→ ok. 3 passed; 0 failed; 0 ignored; 0 measured; 41 filtered out; finished in 0.00s
  - identity_runtime::tests::refuses_duplicate_brain_identity_file ... ok
  - identity_runtime::tests::secure_store_put_failure_rolls_back_brain_identity_for_retry ... ok
  - identity_runtime::tests::synthetic_brain_keeps_secret_out_of_public_metadata ... ok
```

## Complete physical product matrix

| # | Case | Result | Evidence |
|---|---|---|---|
| 1 | Store | **PASS** | `macos_store_read_delete_missing_redaction_matrix` put |
| 2 | Read | **PASS** | same; byte-equal get |
| 3 | Delete | **PASS** | same; delete then get → `Missing` |
| 4 | Missing-item (`get` **and** `delete` never-written) | **PASS** | `macos_delete_never_written_account_is_typed_missing` |
| 5 | Redacted errors | **PASS** | matrix + never-written Display/Debug vs `payload_bytes_recoverable` |
| 6 | Auth / interaction / prompt observation | **PASS (run-specific)** | Keychain UI prompt **not observed** during this short run |

## Keychain prompt

Keychain access prompt was **not observed** during this Gate C host run (sub-second completion; no dialog wait). Absence of a prompt in this short run does **not** prove prompts never occur under other codesign/ACL/headless conditions (same caveat as Gate A).

## Cleanup

**Confirmed** for Dev product service `Cryptex-Dev-DeviceCredentials` after the run:

- `security find-generic-password -s Cryptex-Dev-DeviceCredentials` → item not found
- Keychain dump mentions of that service: 0
- No leftover `mac-adapter-` / `mac-never-written-` account-prefix hits

Secret bytes were not printed in command output or cleanup checks.

## Prior incomplete observation (not the VERIFIED claim)

An earlier owner CleanShot of the same command filters (without recorded SHA/host/prompt/cleanup) is retained only as historical context. The VERIFIED claim for this file is the host-run at `610733f8f911c67f341694c4333e34070b706d50` above.

## Verdict

**Gate C macOS product verification: VERIFIED** at `610733f8f911c67f341694c4333e34070b706d50` for `cryptexd::MacosKeychainStore` on this host/run only.

## Explicit non-claims

- Windows Gate C is separate evidence (`evidence/m1/gate-c-product-secure-store-windows-20261006.md`).
- Gate A macOS spike is not this verification.
- No Keychain prompt in this short run is not proof prompts never occur.
- GitHub Actions were not run for this slice.
