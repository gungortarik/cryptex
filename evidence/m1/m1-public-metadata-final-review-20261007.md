# M1 final follow-up: mutable public metadata persistence

Status: Windows verification PASS; current-source cross-platform CI PENDING.

Authority: Master Spec single canonical writer; accepted ADR-0015 pairing records
and Recovery Kit public metadata; Definition of Done failure and platform checks.
The owner explicitly authorized Codex to review and apply necessary corrections.
Owner/human diff review remains OPEN; this agent review does not substitute for it.

## Finding and correction

Pairing and recovery public-metadata writers used a shared, fixed `.json.tmp`
filename with no explicit flush. Concurrent writers could truncate each other's
staging file or race its rename. These mutable records now use a daemon-private
helper with unique create-new staging files, file sync before atomic replacement,
and an OS file lock per target to serialize writers across handles/processes.
Windows uses MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH); Unix uses rename and
parent-directory sync. A Unix post-replacement sync failure is a typed Durability
error, not success or a claim that the previously published file was rolled back.

Write/replace failure attempts staging cleanup. Failed cleanup retains both
errors in PersistenceCleanupFailed and reports whole-operation retry unsafe.
Persistent `.json.lock` files are local coordination artifacts, not canonical
records; future replication must exclude them. Pairing listing ignores them.
No event, pairing product workflow, recovery container, schema or dependency
decision changed. Brain identity remains immutable and is not replaced by this
helper.

The final read-boundary review also found device-id mismatch validation only in
sequence allocate. Validation now lives in read_state, so last_issued and
initialize also reject a counter belonging to another device as typed corruption.
The regression checks all three operations and unchanged corrupt bytes; Windows
sequence suite 14/14 PASS. Allocation's existing defense remains intact.

## Windows verification

- Full `cargo test --locked -p cryptexd`: 97 unit + 9 lifecycle + 3 Mode B + 2 update
  tests PASS; zero failures/skips. After adding the final error-path lock test,
  focused suites PASS: metadata persistence 4/4, pairing 5/5, recovery 3/3.
- Format and git diff checks PASS.
- Strict local Clippy was blocked by Smart App Control, OS error 4551; no local
  lint PASS is claimed. Required CI will supply lint and Mac verification.
- First concurrency regression failed with Windows access-denied during parallel
  destination replacement. Per-target OS locking fixed this without relaxing the
  test: all sixteen writers must succeed and final bytes equal a complete input.
- Tests also cover partial-write/replace failure preserving the prior record,
  cleanup error preservation, and lock release after failed replacement.
- Failed-run TEMP fixture was removed by deleting the inspected file and empty
  directory individually after the recursive cleanup command was policy-rejected.

## Acceptance

Prior implementation CI 37564521336 @ 7619cae remains historical evidence.
This follow-up changes product code and needs its own current-source CI result.
Run 37567278814 @ 25328ac was cancelled before final validation to include the
sequence read-boundary correction in one final implementation-candidate run.
M1 completion, owner review, and completion merge remain OPEN.
