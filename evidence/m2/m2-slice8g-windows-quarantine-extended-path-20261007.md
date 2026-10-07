# M2 Slice 8g — Windows quarantine extended-path adapter (`\\?\`)

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`
**Tip:** `fa9436c73070e6d0b784247894103c58aa1897c6`
**Prior tip:** `15bbcc0` / behavior predecessor `ec9b8b7` (slice 8f measurements + DeviceId binding)

## 1. Owner decision (ADR)

Owner approved Windows extended-length path I/O **inside the quarantine filesystem adapter only**, preserving:

- ADR-0016 §8.1 directory names `{utc_compact}_{reason_code}_{entry_id_hex}`
- ADR-0015 DeviceId policy (no length/alphabet change)

Recorded as:

- **ACCEPTED** addendum: `docs/adr/ADR-0016-windows-quarantine-extended-path-addendum.md`
- Parent ADR §8.1 note + Date revised; `docs/30_ADR_INDEX.md` ADR-0016 row updated

Explicit non-goals preserved: no registry modification; no reliance on `LongPathsEnabled` as the portable fix; no §8.1 rename; extended paths not authorized outside quarantine adapter by this addendum.

## 2. Implementation

`crates/storage/src/quarantine.rs`:

- `reject_unsafe_path_components` — rejects `..` / `.` via `components()` **and** string-segment guards (`\.\` / `\..\`) so Windows forms that omit CurDir from components cannot bypass validation; rejects separator/NUL inside Normal components
- `quarantine_fs_path` — logical → I/O path; Windows applies `\\?\` / `\\?\UNC\` via `windows_extended_length_path` (idempotent; absolute-only)
- Production quarantine I/O goes through `q_*` wrappers (`q_create_dir_all`, `q_create_dir`, `q_open_create_new_write`, `q_open_read_write`, `q_remove_file`, `q_sync_directory`, `q_sync_parent_directory`) and exclusive no-replace object placement using extended paths on Windows
- Public `QuarantineEntry.directory` remains the logical §8.1 layout path (no `\\?\` leak)

## 3. Regression: UUID / max64 DeviceId create + adapter-read + cleanup

Test: `quarantine::tests::adr_layout_windows_path_ops_under_production_like_app_data_root`

Host observation (read-only; registry **not** modified):

- `HKLM\...\FileSystem\LongPathsEnabled` = `Some(1)` / `0x1` (observed only)
- **Do not claim LongPathsEnabled=0 coverage** — not verified on this host

| Case | DeviceId | id_len | Projected object chars | Quarantine create | Adapter read | Cleanup |
|---|---|---:|---:|---|---|---|
| short | `device-safe-1` | 13 | 253 | OK (object=253, `\\?\` I/O) | OK | OK (`remove_dir_all`, root gone) |
| uuid36 | UUID | 36 | 300 | OK (object=300, `\\?\` I/O) | OK | OK |
| max64 | `a`×64 | 64 | 355 | OK (object=355, `\\?\` I/O) | OK | OK |

Path validation regressions:

- `quarantine_fs_path_rejects_parent_and_curdir_components` — PASS (`.`` / `..` refused; extended prefix does **not** bypass)
- `quarantine_fs_path_prefixes_absolute_windows_paths` — PASS (prefix + idempotent re-prefix)

## 4. Local verification (Windows)

Exact commands and results:

```
cargo test --locked -p cryptex-storage quarantine -- --nocapture
→ test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.39s

cargo test --locked -p cryptex-storage
→ test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.51s

cargo test --locked -p cryptexd
→ lib: ok. 123 passed (1.31s)
→ lifecycle_harness: ok. 9 passed (2.55s)
→ mode_b_supervisor: ok. 3 passed (0.30s)
→ update_rollback: ok. 2 passed (0.81s)

cargo fmt --all -- --check
→ PASS (exit 0)

cargo clippy --locked --workspace --all-targets -- -D warnings
→ PASS (Finished `dev` profile; exit 0)
```

## 5. macOS per-device segment-binding tests

**Not run.** No SSH config / reachable macOS host was available from this Windows implementer environment. Binding regressions remain covered by Windows `cryptexd` lib tests (`mismatched_device_id_*`). Re-run on macOS when that host is available:

```
cargo test --locked -p cryptexd mismatched_device_id
```

## 6. Non-goals / deferred

- GitHub Actions / final M2 CI gate (reserved for final candidate after security + DoD review)
- Claiming portable success under `LongPathsEnabled=0` without measured evidence
- Extended-path adaptation for non-quarantine canonical `.open`/`.seg` writers
- M2 complete claim

## Next

- Optional: macOS binding re-check on available Mac host
- Continue remaining M2 product/DoD gaps; conserve Actions until final dual-OS gate
