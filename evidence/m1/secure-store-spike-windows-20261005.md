# ADR-0014 secure-store evaluation spike — Windows results

**Date:** 2026-10-05
**Revised:** 2026-10-05 (measurement correction: P1 Keychain classification + K2 byte-aware redaction)
**Gate:** A (product-separated evaluation spike)
**Branch:** `m1/brain-device-identity`
**Host:** Windows (local)
**Mac host:** not executed — all Mac cells **WAITING**
**ADR-0014 status after this evidence:** remains **PROPOSED** (pending owner review)

## Scope and constraints

- Compared **K2** (`keyring-core` + explicit platform providers) and **P1** (direct platform APIs).
- Spike lives at `tools/m1-secure-store-spike/` with a standalone `[workspace]` — **not** a Cryptex product workspace member.
- No product workspace dependency pins; no `cryptexd`/production secure-store backend merge.
- Synthetic throwaway secrets only; secret bytes never written to evidence, logs, or the repository.
- Cleanup: spike deletes created Credential Manager items after each candidate run (and on failure paths it can reach).

## Candidate versions (spike-local pins)

| Component | Version / note |
|---|---|
| `keyring-core` | `1.0.0` |
| `windows-native-keyring-store` | `1.1.0` (`default-features = false`, no `search`/regex) |
| `apple-native-keyring-store` | `1.0.2` (`keychain` feature) — Mac only; not executed here |
| P1 Windows | `windows-sys = 0.61.2` CredWriteW / CredReadW / CredDeleteW |
| P1 macOS | `security-framework = 3.7.0` generic-password helpers — Mac only; not executed here |

## Measurement corrections (this revision)

1. **P1 macOS classification:** only Keychain `errSecItemNotFound` (-25300) maps to missing-item; auth-failed / interaction-not-allowed / missing-entitlement and other statuses remain sanitized platform errors. Unit tests cover this classification on every host.
2. **K2 error-redaction:** detects recoverable secret bytes in UTF-8 text **and** Debug/hex forms (e.g. `BadEncoding([..decimal bytes..])`). Matrix notes separate `missing_item_errors` from `payload_bearing_upstream`. Overall redaction PASS requires both safe.

## Matrix results

Legend: **PASS** / **FAIL** = physically measured on this host · **WAITING** = not run on required host · never treat WAITING as pass.

| Candidate | Platform | store | read | delete | missing-item | error-redaction |
|---|---|---|---|---|---|---|
| K2 | Windows | PASS | PASS | PASS | PASS | **FAIL** |
| P1 | Windows | PASS | PASS | PASS | PASS | PASS |
| K2 | macOS | WAITING | WAITING | WAITING | WAITING | WAITING |
| P1 | macOS | WAITING | WAITING | WAITING | WAITING | WAITING |

### Windows K2 observations

- Provider wiring: `set_default_store(windows_native_keyring_store::Store::new()?)` then `Entry::new(service, user)`.
- API used: `set_secret` / `get_secret` / `delete_credential`; missing item returns `keyring_core::Error::NoEntry`.
- Default Windows target naming: `{user}.{service}` (provider default delimiter `.`).
- Provider docs warn same-entry multi-thread access is not reliably ordered.
- **Error redaction (corrected):** `NoEntry` Display/Debug are SAFE (no recoverable secret bytes). `Error::BadEncoding(Vec<u8>)` Display is SAFE, but **Debug exposes the full secret as a decimal byte list** → `payload_bearing_upstream=EXPOSED` → overall error-redaction **FAIL**. Product wrappers must never log/`Debug` raw `BadEncoding` / `BadDataFormat` values.

### Windows P1 observations

- Direct Cred* generic credentials with target `{service}/{account}` and `CRED_PERSIST_LOCAL_MACHINE`.
- Missing item maps to `ERROR_NOT_FOUND` → typed `secure_store_missing_item` (no secret in Display/Debug; UTF-8 and Debug/hex forms checked).
- Full control of error surface and naming; more adapter code to maintain per OS.

## Cleanup

- Per-run unique account ids under service `Cryptex-Dev-M1-Spike`.
- Explicit delete after matrix; best-effort delete on early failure.
- After a hard crash, remove leftover items named with service `Cryptex-Dev-M1-Spike` / account prefix `synthetic-` from Windows Credential Manager.

## Mac host instructions (precise)

Do **not** mark Mac verified from this Windows evidence. On a Mac host **after this measurement-correction commit is present**:

```bash
cd tools/m1-secure-store-spike
cargo test
cargo run --release
```

Expect the same five cells for K2 and P1. Record a dated `evidence/m1/` addendum. Keep any unfinished cell **WAITING**. Keychain may prompt depending on codesign/ACL; note prompt behavior without writing secret bytes. Confirm P1 missing-item PASS only occurs with `errSecItemNotFound`, not access-denied.

Manual leftover cleanup if needed: Keychain Access → search `Cryptex-Dev-M1-Spike` / `synthetic-` and delete spike items only.

## Recommendation (spike, not acceptance)

**Provisional preference: still K2** for explicit provider wiring, **only if** product `SecureStore` errors never surface `BadEncoding`/`BadDataFormat` payloads in Display/Debug/logs/IPC. Windows Gate A now records that upstream K2 error Debug is unsafe without that wrapper.

P1 remains viable where maximum error-surface control is preferred (Windows redaction PASS without upstream payload-bearing variants). **Do not accept ADR-0014 or start product secure-store implementation from this Windows-only slice alone.** Mac matrix remains WAITING.

## Local checks run (this revision)

- `cargo test --manifest-path tools/m1-secure-store-spike/Cargo.toml` — 12 passed (classify + redaction)
- `cargo run --manifest-path tools/m1-secure-store-spike/Cargo.toml --release` — Windows matrix as tabulated above

## Non-claims

- No Mac verification claimed.
- No product backend selected or merged.
- No GitHub Actions run for this spike.
- No ADR acceptance.
