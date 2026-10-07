# M2 Slice 8j — Harden all cryptexd lib test temp roots

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`
**Tip:** *(filled after commit)*
**Prior tip:** `1c77ab9` / behavior `e5568bd` (install_runtime-only exclusive temp_root)

## CI failure that triggered this slice

Final dual-OS rerun (post-8i) failed on macOS:

- `identity_runtime::tests::refuses_duplicate_brain_identity_file`
- Cause: timestamp-only `cryptexd-identity-{nanos}` + pre-delete `remove_dir_all` raced under `--test-threads>1`

## Audit (`cargo test -p cryptexd --lib`)

Vulnerable patterns found (timestamp-only and/or pre-delete / non-exclusive create):

| Module | Helper | Previous pattern |
|---|---|---|
| `identity_runtime` | `temp_root` | nanos + pre-delete |
| `pairing_store` | `temp_root` | nanos + pre-delete |
| `recovery_store` | `temp_root` | nanos + pre-delete |
| `sequence_runtime` | `temp_root` | nanos + pre-delete |
| `install_runtime` | `temp_root` | already exclusive (8i); moved to shared helper |
| `event_signing_key` | `temp_root` | pid+nanos, no exclusive create |
| `instance` | `unique_root` | stamp + `create_dir_all` |
| `compat` | `temp_runtime` | stamp only |
| `update` | `temp_root` | stamp + `create_dir_all` |
| `supervisor` | inline root | stamp only |
| `metadata_persistence` | `fixture` | pid+seq `create_dir` without AlreadyExists retry |
| `sequence_store` | `temp_store` + several roots | pid+label + pre-delete |
| `canonical_writer` | `temp_root` | pid+seq path without exclusive create |
| `compare` | measure render root | fixed name |
| `platform/windows/hkcu_run` | `unique_root` | stamp + `create_dir_all` |
| `platform/macos` | LaunchAgent roots | fixed names |
| `platform/windows/logon_task` | XML/BOM/spaces roots | fixed names / pre-create |

Left unchanged (not a per-test fixture allocator): `paths.rs` production fallback `temp_dir()/Cryptex-Dev/...`.

## Fix

Added `services/cryptexd/src/test_temp.rs` (`#[cfg(test)]` via `lib.rs`):

- `exclusive_temp_dir(prefix)` / `exclusive_temp_dir_in(parent, prefix)`
- Name `{prefix}-{pid}-{seq}`
- Exclusive `fs::create_dir`, retry on `AlreadyExists`
- Never pre-delete a candidate path

All listed helpers now call this allocator. Production behavior and test assertions unchanged (spaces path test still uses a child directory named `cryptex logon task` under an exclusive parent).

## Verification

### Windows

```
cargo test --locked -p cryptexd --lib -- --test-threads=16
→ run1..run5: ok. 126 passed each

cargo test --locked -p cryptexd
→ lib 126; lifecycle_harness 9; mode_b_supervisor 3; update_rollback 2 — PASS

cargo test --locked -p cryptex-storage
→ ok. 35 passed

cargo fmt --all -- --check
→ PASS

cargo clippy --locked --workspace --all-targets -- -D warnings
→ PASS
```

### macOS (local host)

**Not run** — no reachable macOS host from this Windows implementer environment. Stability proof for Mac remains the reserved final dual-OS Actions run on this tip (not dispatched here).

## Non-goals

- GitHub Actions dispatch
- Assertion weakening
- Production path layout changes
