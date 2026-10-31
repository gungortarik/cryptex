# ADR-0014 secure-store evaluation spike

Product-separated Gate A evaluation for `docs/adr/ADR-0014-os-secure-store-integration.md`.

## Rules

- Not a Cryptex product workspace member (standalone `[workspace]` in this `Cargo.toml`).
- Do not add or pin these crates in the product workspace until ADR-0014 is accepted.
- Use only synthetic throwaway secret values.
- Never write secret bytes into the repository, evidence, or logs.
- Always clean up OS store items created by a run.

## Candidates

| ID | Approach |
|---|---|
| K2 | `keyring-core` + explicit platform provider |
| P1 | Direct platform APIs behind spike adapters |

## Windows (run here)

```powershell
cd tools/m1-secure-store-spike
cargo run --release
```

Evidence output is pass/fail/status only; secret bytes are never printed.

## macOS host instructions (WAITING until executed)

Run on a Mac host with the same matrix. Do not mark Mac cells verified from Windows.

```bash
cd tools/m1-secure-store-spike
cargo run --release
```

Expected matrix cells (K2 and P1 each):

1. `store` — write synthetic secret
2. `read` — round-trip equality (report PASS/FAIL only)
3. `delete` — remove item
4. `missing-item` — read after delete → not-found
5. `error-redaction` — Display/Debug of typed errors must not contain the synthetic secret bytes

Cleanup: the binary deletes its spike items on success and on failure paths it can reach. After a crash, remove leftover items manually:

- K2 default target form: `{user}.{service}` in Credential Manager / Keychain
- Spike service label: `Cryptex-Dev-M1-Spike`
- Spike account prefix: `synthetic-`

Record Mac results under `evidence/m1/` and keep unfinished cells labeled **WAITING**.

## Measurement rules (Gate A accuracy)

- P1 macOS maps **only** Keychain errSecItemNotFound (-25300) to missing-item; other statuses (auth failed, interaction not allowed, missing entitlement, etc.) stay sanitized platform errors so missing-item cannot PASS on access-denied.
- K2 error-redaction distinguishes safe missing-item (NoEntry) from payload-bearing upstream variants (BadEncoding). Debug decimal/hex byte recovery counts as exposure; overall redaction PASS requires both surfaces safe.

## Versions under evaluation

Pinned only inside this spike package:

- `keyring-core = 1.0.0`
- `windows-native-keyring-store = 1.1.0` (default features off)
- `apple-native-keyring-store = 1.0.2` (`keychain` feature)
- P1 Windows: `windows-sys` Cred* APIs
- P1 macOS: `security-framework` 3.7.0 `generic_password(PasswordOptions::new_generic_password(..))` + `set_generic_password` / `delete_generic_password`
