# M4 Windows IPC overlapped-I/O cancellation and drain

**Status:** implemented and verified; M4 remains IN PROGRESS
**Code commit:** `f8e7636fbf4ac5413d25ef6299f16a3b20629117`
**Branch:** `m4/knowledge-engine`

## Change

When Windows named-pipe I/O is pending, timeout and unexpected wait failures
now request cancellation and perform a blocking completion query before the
stack `OVERLAPPED`, event handle, or borrowed transfer buffer can leave scope.
An unexpected `ERROR_IO_INCOMPLETE` after wake also enters the same cancel and
drain path. Successful transfers are checked against the shared deadline before
their bytes are returned.

The regression suite injects `WAIT_FAILED` while a read is pending, sends a
marker from the peer after the failure, and confirms the caller's buffer stays
unchanged. A separate stalled-write case verifies timeout behavior alongside
the existing stalled-read test.

## Verification

Windows local checks at the code commit:

- `cargo fmt --all -- --check` — PASS
- `cargo test --locked -p cryptexd --lib ipc::tests:: -- --nocapture` — PASS, 18/18
- `cargo clippy --locked -p cryptexd --all-targets -- -D warnings` — PASS
- `git diff --check` — PASS

Read-only security re-review found no remaining P1/P2 issue. It notes that an
unexpected non-`ERROR_NOT_FOUND` `CancelIoEx` failure can delay return while the
implementation waits for terminal I/O completion. Waiting preserves pointer
lifetimes and is the safe behavior for that rare OS/API failure.

Final Actions run [38018083958](https://github.com/gungortarik/cryptex/actions/runs/38018083958)
passed at the exact code commit: macOS readiness, Windows readiness, and
Windows IPC cross-account ACL all PASS; unsigned Windows package skipped by
workflow design.

## Remaining M4 gates

- macOS visual review
- Final Definition of Done and owner review

Production v2 writing remains disabled. No M8/M14 functionality was added.
