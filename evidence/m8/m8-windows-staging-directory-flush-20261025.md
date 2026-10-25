# M8 Windows staging/journal directory flush

**Date:** 2026-10-25  
**Source before this change:** `8a58326276c94dd97c1066ad68f1a9ff89504644`  
**Platform:** Windows development host, NTFS temporary directory

## Change

The M8 `sync_staged_parent` Windows path no longer returns success without a
parent-directory operation. After a staging or prepared-journal entry is placed
or removed, it opens the parent directory with `FILE_FLAG_BACKUP_SEMANTICS` and
`FILE_FLAG_OPEN_REPARSE_POINT`, verifies the opened handle is an ordinary
directory, and calls `FlushFileBuffers`. Open, identity inspection, or flush
failure is returned as an error. The existing `MoveFileExW(MOVEFILE_WRITE_THROUGH)`
placement barrier remains in place.

This was intentionally scoped to M8 staging/journal. Applying directory flush
to the general storage helper caused unrelated canonical writer tests to try
flushing the volume root and fail with access denied; that broad change was
reverted completely.

## Verification

- `cargo test -p cryptexd --lib import_staging::tests` — **16/16 PASS**
  - Includes actual directory flush on the current Windows filesystem.
  - Includes refusal to flush a junction/reparse-point parent.
- `cargo test -p cryptexd --lib` — **414 passed, 4 ignored**.
- `cargo clippy -p cryptexd --all-targets -- -D warnings` — **PASS**.
- `cargo fmt --all -- --check` and `git diff --check` — **PASS**.

## Limits

The Windows API probe establishes that this filesystem accepts
`FlushFileBuffers` on the opened directory handle. It does not establish
power-loss behavior for every Windows filesystem/device, nor does it replace
process/power-loss fault injection at each journal and publication boundary.
Canonical publication, committed-marker replay, and trust activation remain
unimplemented; no M8 readiness or durability-complete claim is made.

Microsoft API references:

- [`FlushFileBuffers`](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-flushfilebuffers)
- [`MoveFileExW`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw)
