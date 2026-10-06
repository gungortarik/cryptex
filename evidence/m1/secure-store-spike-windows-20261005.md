# ADR-0014 secure-store evaluation spike — Windows results

**Date:** 2026-10-05  
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

## Matrix results

Legend: **PASS** = physically verified on this host · **WAITING** = not run on required host · never treat WAITING as pass.

| Candidate | Platform | store | read | delete | missing-item | error-redaction |
|---|---|---|---|---|---|---|
| K2 | Windows | PASS | PASS | PASS | PASS | PASS |
| P1 | Windows | PASS | PASS | PASS | PASS | PASS |
| K2 | macOS | WAITING | WAITING | WAITING | WAITING | WAITING |
| P1 | macOS | WAITING | WAITING | WAITING | WAITING | WAITING |

### Windows K2 observations

- Provider wiring: `set_default_store(windows_native_keyring_store::Store::new()?)` then `Entry::new(service, user)`.
- API used: `set_secret` / `get_secret` / `delete_credential`; missing item returns `keyring_core::Error::NoEntry`.
- Default Windows target naming: `{user}.{service}` (provider default delimiter `.`).
- Provider docs warn same-entry multi-thread access is not reliably ordered.
- Error redaction: `NoEntry` Display/Debug did not embed the synthetic secret text. `Error::BadEncoding(Vec<u8>)` exists as a typed carrier for password bytes; product wrappers must never log raw `BadEncoding` / `BadDataFormat` payloads even if Display appears safe in this probe.

### Windows P1 observations

- Direct Cred\* generic credentials with target `{service}/{account}` and `CRED_PERSIST_LOCAL_MACHINE`.
- Missing item maps to `ERROR_NOT_FOUND` → typed `secure_store_missing_item` (no secret in Display/Debug).
- Full control of error surface and naming; more adapter code to maintain per OS.

## Cleanup

- Per-run unique account ids under service `Cryptex-Dev-M1-Spike`.
- Explicit delete after matrix; best-effort delete on early failure.
- After a hard crash, remove leftover items named with service `Cryptex-Dev-M1-Spike` / account prefix `synthetic-` from Windows Credential Manager.

## Mac host instructions (precise)

Do **not** mark Mac verified from this Windows evidence. On a Mac host:

```bash
cd tools/m1-secure-store-spike
cargo run --release
```

Expect the same five cells for K2 and P1. Record a dated `evidence/m1/` addendum. Keep any unfinished cell **WAITING**. Keychain may prompt depending on codesign/ACL; note prompt behavior without writing secret bytes.

Manual leftover cleanup if needed: Keychain Access → search `Cryptex-Dev-M1-Spike` / `synthetic-` and delete spike items only.

## Recommendation (spike, not acceptance)

**Provisional preference: K2** (`keyring-core` + explicit Windows/macOS provider crates), contingent on:

1. Mac matrix completing with PASS (currently WAITING).
2. Product `SecureStore` errors never exposing `BadEncoding`/`BadDataFormat` byte payloads in logs or IPC.
3. Explicit provider pins in product Cargo.toml only after ADR-0014 acceptance (gate B).
4. Daemon-only access and `Cryptex` / `Cryptex-Dev` naming policy as drafted in ADR-0014.

P1 remains a viable alternative if owner prefers maximum explicitness over shared keyring-core API surface. **Do not accept ADR-0014 or start product secure-store implementation from this Windows-only slice alone.**

## Non-claims

- No Mac verification claimed.
- No product backend selected or merged.
- No GitHub Actions run for this spike.
