# M2 Slice 8h — Tighten Windows extended-path regression assertions

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`
**Tip:** *(filled after commit)*
**Prior tip:** `478e35d` / behavior `fa9436c` (slice 8g adapter + ADR addendum)

## Change

`quarantine::tests::adr_layout_windows_path_ops_under_production_like_app_data_root` no longer uses unprefixed Win32 paths as **required** existence, read, or cleanup checks.

Required path I/O after successful quarantine:

1. Assert public `QuarantineEntry.directory` does **not** start with `\\?\` (logical layout preserved).
2. Build `io_object` / `io_root` via `quarantine_fs_path` (extended-length prefix).
3. Existence: `io_object.is_file()` (prefixed).
4. Read: `q_open_read_write` + prefixed `fs::read(&io_object)`.
5. Cleanup: `fs::remove_dir_all(&io_root)` and `!io_root.exists()` (prefixed).
6. uuid36/max64 also assert logical object path char count `> 260`; short remains `< 260`.

Registry was **not** modified. Host observation only: `LongPathsEnabled = Some(1)`. **LongPathsEnabled=0 is not claimed.**

## Verification (Windows host)

```
cargo test --locked -p cryptex-storage adr_layout_windows_path_ops -- --nocapture
→ ok. 1 passed (short/uuid36/max64 create + prefixed read + prefixed cleanup)

cargo test --locked -p cryptex-storage quarantine -- --nocapture
→ ok. 20 passed; 0 failed

cargo fmt --all -- --check
→ PASS

cargo clippy --locked -p cryptex-storage --all-targets -- -D warnings
→ PASS
```

Measured path lengths (same host pattern as 8g): short object=253; uuid36=300; max64=355.

## Non-goals

- GitHub Actions
- Claiming LongPathsEnabled=0 coverage
- Production code changes beyond the regression test
