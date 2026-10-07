# M2 Slice 8f — Windows quarantine path ops + per-device segment binding

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`
**Tip:** `ec9b8b7bab6da31dffbbebd86f31abc6c5a55b86`
**Prior tip under review:** `602aff451959c7a95fcc706dfa08903f8a112960` / behavior `e098e8a431504e34a4659ee48546f7b1041a2315`

## 1. Windows quarantine path behavior (actual FS ops)

ADR-0016 §8.1 layout unchanged: `{utc_compact}_{reason_code}_{entry_id_hex}` under
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\m2-tests\...` (production-like app-data root).

Test: `quarantine::tests::adr_layout_windows_path_ops_under_production_like_app_data_root`
(`cargo test --locked -p cryptex-storage adr_layout_windows_path_ops -- --nocapture`).

| Case | DeviceId | id_len | Projected object path chars | Actual FS quarantine | Cleanup `remove_dir_all` |
|---|---|---:|---:|---|---|
| short | `device-safe-1` | 13 | 253 | **OK** (object=253) | **OK** (root gone) |
| uuid36 | `550e8400-e29b-41d4-a716-446655440000` | 36 | 300 | **OK** (object=300) | **OK** |
| max64 | `a`×64 | 64 | 355 | **OK** (object=355) | **OK** |

Host OS policy (read-only; not modified):

- `HKLM\SYSTEM\CurrentControlSet\Control\FileSystem\LongPathsEnabled` = `0x1`

Classic Win32 MAX_PATH remains 260. UUID/max cases exceed 260 character counts but
succeeded on this host while OS long paths were enabled.

### Packaged Windows application manifest / long-path support

Inspected sources and a freshly built `cryptexd` debug binary:

- Repo packaging (`apps/desktop/src-tauri/tauri.conf.json`, `Cargo.toml`, `build.rs`;
  `services/cryptexd`) contains **no** `longPathAware` / custom Windows `.manifest`.
- PE scan of `cryptexd.exe`: **no** embedded `longPathAware`, `assemblyIdentity`,
  or `requestedExecutionLevel` UTF-16/ASCII markers.
- Desktop package config likewise does not declare long-path awareness.

**Assumption required for portable >260 paths:** do not treat this host’s
`LongPathsEnabled=1` success as a product guarantee. Machines with
`LongPathsEnabled=0` (Windows default on many images) still face classic MAX_PATH
for UUID-length and max DeviceId quarantine object paths under §8.1.

Layout / DeviceId policy were **not** shortened or changed.

### ADR options (owner decision; not implemented)

If Cryptex must support UUID-length / 64-char DeviceIds on Windows without relying
on ambient OS long-path policy:

1. **Embed `longPathAware` manifests** for `cryptexd` + Desktop **and** document
   required OS `LongPathsEnabled=1` (dual requirement per Microsoft guidance).
2. **Use `\\?\` extended paths** inside the quarantine FS adapter only
   (layout names unchanged).
3. **Separate ADR** to alter attribution/name policy or DeviceId length on Windows
   (explicitly out of scope for implementer unilateral change).

## 2. Per-device segment identity binding

### Gap

`VerifyingBodyValidator` verified signatures/schema but did **not** require
`AcceptedEvent.device_id` to equal the DeviceId whose `.open`/`.seg` directory was
being scanned.

### Fix

- `VerifyingBodyValidator` now carries `expected_device_id` and rejects mismatches as
  `ValidationReasonCode::IntegrityFailure` (typed), feeding existing
  quarantine-before-truncate (active) / whole-pack quarantine (sealed) paths.
- `scan_pack_events` re-checks `accepted.device_id` against the scanned DeviceId
  (fail closed).

### Regressions

- `mismatched_device_id_in_active_open_is_quarantined_and_prefix_kept` — PASS  
  Valid prefix retained; mismatch quarantined (`integrity_failure`); subsequent append chains from prefix.
- `mismatched_device_id_in_sealed_pack_fails_closed_and_quarantines` — PASS  
  Sealed pack with good+mismatch fail-closed; pack quarantined.

## Local verification (Windows)

- `cargo test --locked -p cryptex-storage -- --test-threads=8` — PASS (32)
- `cargo test --locked -p cryptexd --lib canonical_writer -- --test-threads=8` — PASS (12)
- `cargo fmt --all -- --check` — PASS
- `cargo clippy --locked -p cryptex-storage --all-targets -- -D warnings` — PASS
- `cargo clippy --locked -p cryptexd --all-targets -- -D warnings` — PASS

## Non-goals

- GitHub Actions / final M2 CI gate
- ADR-0016 layout or DeviceId policy changes
- Registry modification

## Next

- Owner review of path ADR options + this tip
- Actions still deferred until final dual-OS gate is required
