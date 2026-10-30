# M8 Windows pre-publication interruption tests

**Date:** 2026-10-25  
**Branch:** `codex/m8-device-trust`

## Change

Added subprocess interruption tests at two durability boundaries in prepared
manifest creation:

1. After the prepared temporary file is synced, before no-replace placement.
2. After the prepared manifest is placed and its parent directory is synced.

Each child exits abruptly through the existing test-only interrupt helper. The
parent then calls startup recovery and checks that the incoming transport file
remains retryable and the uncommitted staging/journal state is removed. This is
process-crash coverage for pre-publication states; it is not power-loss testing
and does not cover canonical publication, which is not implemented.

## Verification

- `cargo check -p cryptexd --tests` — **PASS**.
- `cargo clippy -p cryptexd --all-targets -- -D warnings` — **PASS**.
- `cargo fmt --all -- --check` and `git diff --check` — **PASS**.
- `cargo test -p cryptexd --lib import_staging::tests::xproc_interrupt_after_prepared -- --nocapture` — **PASS, 2/2**:
  `xproc_interrupt_after_prepared_temp_sync_recovers_unprepared_state` and
  `xproc_interrupt_after_prepared_manifest_sync_recovers_prepublication_state`.
- `cargo test -p cryptexd --lib import_staging::tests` — **PASS, 19/19** at
  the pre-publication crash-test checkpoint below.
- `cargo test -p cryptexd --lib` — **PASS, 417 passed, 0 failed, 4 ignored**.

The earlier execution attempt was blocked by Windows Application Control
error **4551**; the ordinary local test retry subsequently ran successfully.
These results verify process interruption and pre-publication recovery on this
Windows host. They are not power-loss evidence, do not verify publication or
partial-publication replay, and do not close M8. No Actions ran.

## Follow-up: committed-marker placement probe

Added a Windows-only test using the proposed committed-marker temporary/final
names under `canonical/import-journal`. It syncs the temporary file, exercises
the existing same-volume `MoveFileExW(MOVEFILE_WRITE_THROUGH)` no-replace
placement helper, flushes the journal parent with `sync_staged_parent`, and
reads back the exact marker bytes. Both calls succeed on this host. This does
not exercise a production marker parser/writer, process interruption, power
loss, or portable filesystem durability. Microsoft documents the
`MOVEFILE_WRITE_THROUGH` move behavior, but its directory-handle API list does
not list `FlushFileBuffers` as a directory-handle operation; therefore the
successful directory flush here is observed host behavior, not a documented
cross-filesystem guarantee. The test fixture uses `%TEMP%` on `C:`; this host
reports `C:` as NTFS. No other filesystem was tested.

Sources:

- Microsoft Learn, [MoveFileExW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw) (`MOVEFILE_WRITE_THROUGH`, no replace flag)
- Microsoft Learn, [Directory Handles](https://learn.microsoft.com/en-us/windows/win32/fileio/obtaining-a-handle-to-a-directory) (supported directory-handle function list)
- Microsoft Learn, [FlushFileBuffers](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-flushfilebuffers)

- Focused test — **PASS, 1/1**.
- Focused no-replace collision test — **PASS, 1/1**; an existing final marker
  remained byte-identical and the temporary marker remained available after
  the attempted placement.
- `cargo test -p cryptexd --lib import_staging::tests` — **PASS, 21/21**.
- `cargo fmt --check` — **PASS**.
- `cargo clippy -p cryptexd --lib --tests --release -- -D warnings` — **PASS**.
- `git diff --check` — **PASS**.

No Actions ran; no canonical publisher or trust/transport activation was added.
