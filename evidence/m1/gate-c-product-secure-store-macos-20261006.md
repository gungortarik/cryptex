# Gate C evidence — macOS product SecureStore (WAITING)

**Status: WAITING** — owner screenshot records successful filtered product lib tests, but Gate C cannot be marked VERIFIED without the missing host/SHA/prompt/cleanup fields below.

**Authority:** ADR-0014 §8.2 gate C; procedure `evidence/m1/gate-c-product-secure-store-macos-procedure.md`  
**Adapter under test (from procedure + current source):** `cryptexd::MacosKeychainStore`  
**Product pin observed in screenshot compile lines:** `security-framework v3.7.0`  
**GitHub Actions:** not run (deferred)

## Source / branch check (this recording session)

| Item | Value |
|---|---|
| Current branch (Mac checkout when recording) | `m1/brain-device-identity` |
| Current HEAD (Mac checkout when recording) | `53dece8f7c1b7550545df054db305003dbb7cfa9` |
| Screenshot shows tested Git SHA | **NOT RECORDED** |
| Compatible with current tip content? | **Yes, content-compatible only** — screenshot lists both current `secure_store::macos` tests (`macos_delete_never_written_account_is_typed_missing`, `macos_store_read_delete_missing_redaction_matrix`) and all three current `identity_runtime` tests. Those macos tests exist on this branch from `e7b83cf` onward (ancestor of current HEAD). **This is not proof the screenshot run used HEAD `53dece8`.** |

## Fields from owner screenshot (supported facts only)

**Screenshot artifact (outside Git):**  
`/Users/tarikgungor/Library/Application Support/CleanShot/media/media_Nd2DS4bnGR/CleanShot 2026-10-06 at 10.40.05 AM@2x.png`  
SHA-256: `19d48ab197d35c46d38369d3e769f3ac82e79393e91254c6655efce613a32e1e`  
Filename timestamp: 2026-10-06 10:40:05 local (CleanShot).

**Host identity visible in prompt:** user `tarikgungor`, hostname `Tariks-Air`, cwd under `/Users/tarikgungor/github_repos/cryptex`.

### Commands and outcomes (exact from screenshot)

```text
cargo test -p cryptexd --lib --locked secure_store::macos -- --nocapture
→ test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 42 filtered out; finished in 0.21s
  - secure_store::macos::tests::macos_delete_never_written_account_is_typed_missing ... ok
  - secure_store::macos::tests::macos_store_read_delete_missing_redaction_matrix ... ok

cargo test -p cryptexd --lib --locked identity_runtime -- --nocapture
→ test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 41 filtered out; finished in 0.00s
  - identity_runtime::tests::refuses_duplicate_brain_identity_file ... ok
  - identity_runtime::tests::synthetic_brain_keeps_secret_out_of_public_metadata ... ok
  - identity_runtime::tests::secure_store_put_failure_rolls_back_brain_identity_for_retry ... ok
```

Screenshot also shows a compile of `security-framework v3.7.0` / `cryptexd v0.0.0` for the first command. Exit codes are not printed as numbers; both commands show `test result: ok` with zero failures.

## Fields NOT RECORDED (screenshot + no other Gate C Mac-host output found)

| Field | Status |
|---|---|
| Tested Git SHA | **NOT RECORDED** |
| macOS version | **NOT RECORDED** |
| macOS build | **NOT RECORDED** |
| Architecture | **NOT RECORDED** |
| Keychain UI prompt during this Gate C run | **NOT RECORDED** |
| Cleanup result for this Gate C run | **NOT RECORDED** |

Gate A spike evidence (`evidence/m1/secure-store-spike-macos-20261006.md`) has host version/arch and a Keychain-prompt note for a **different** Gate A spike run at SHA `4f4ba278…`. Those values are **not** copied here as Gate C facts.

## Physical product matrix (from screenshot test names only)

| # | Case | Result | Evidence basis |
|---|---|---|---|
| 1 | Store | **PASS** (command outcome) | `macos_store_read_delete_missing_redaction_matrix` … ok |
| 2 | Read | **PASS** (command outcome) | same |
| 3 | Delete | **PASS** (command outcome) | same (`delete` then `get` → Missing in test body) |
| 4 | Missing-item (`get` **and** `delete` never-written) | **PASS** (command outcome) | `macos_delete_never_written_account_is_typed_missing` … ok |
| 5 | Redacted errors | **PASS** (command outcome) | redaction asserts in matrix / never-written tests |
| 6 | Auth / interaction classification / prompt observation | **WAITING** | Keychain prompt for this run **NOT RECORDED** |

Secret bytes do not appear in the screenshot text.

## Verdict

**Gate C macOS product verification: WAITING** (not VERIFIED).

Required to close: record tested Git SHA on the Mac host, macOS version/build, architecture, Keychain prompt observation for **this** run, and cleanup status; then re-affirm the matrix against that SHA. Do not infer those fields from Gate A or from the recording-session HEAD alone.

## Explicit non-claims

- This file does **not** verify macOS Gate C.
- Windows Gate C VERIFIED does **not** imply macOS Gate C.
- Gate A macOS spike does **not** satisfy Gate C.
- Content-compatibility with current `m1/brain-device-identity` tip is not a substitute for a recorded tested SHA.
