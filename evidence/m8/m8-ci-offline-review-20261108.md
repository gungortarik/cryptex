# Offline/restriction source review and first exact-candidate CI

Date: 2026-11-08 (America/New_York).

## Reviewed candidate

`8cfcd50de773e757dad5f67278fe57f38d4f266f`: two new daemon offline/restriction
integration tests, M8 completion ledger, previous long-path production fix.
Independent technical agent completed read-only source review with no actionable
finding. It checked local-drive/NUL/parent path refusal, separator conversion
without canonicalization, owner/ancestor/reparse/no-replace flags; and genuine
seq 1–3 fixture/cutoff 3 vs offline seq 4 semantics for retired/compromised.
No reviewer executable was run. This is not human acceptance or activation approval.

## CI dispatch and observed failures

Run [42558672783](https://github.com/gungortarik/cryptex/actions/runs/42558672783)
was explicitly dispatched on that exact source. Windows different-user IPC ACL
job passed. Mac readiness reached full daemon library tests: 462 passed, 2 failed,
4 ignored. Later steps skipped. Windows readiness was still running when the
failure was analyzed. No green CI claim.

Mac failures:

- `ipc::tests::timed_out_search_returns_hold_and_keeps_its_slot_until_work_exits`:
  expected active slot 1, observed 0. Its background worker sleeps 120ms while
  caller times out after 30ms; under parallel scheduling, the worker can finish
  before caller examines the slot. This observation does not establish a product
  slot-lifetime bug.
- `syncthing_rest::tests::total_budget_covers_body_reads_and_both_requests`:
  fixture waits ten seconds for a second connection that need not occur if the
  correct client exhausts its shared 250ms budget during the first response.
  Thus fixture join panics after an otherwise expected Deadline result.

## Test fixture corrections

IPC worker waits on a release channel until after the test asserts no late hits,
deadline hold, occupied slot and refusal of another reservation. Test then releases
work and still requires capacity to return. A caller panic drops the release end;
the worker does not stay blocked indefinitely. Product run_bounded_search unchanged.

The SyncthingRest fixture client is entirely cfg(test), separate from production
OwnedSyncthingRest. Added a private inter-request deadline seam; after successful
version validation, the test explicitly exhausts that same deadline. It must
return Deadline before the status phase receives a new budget. The fixture records
one version request; that single-reply counter alone is not independent wire-level
proof of no second connection. The Deadline assertion and pre-send remaining-budget
check support the bounded shared-deadline behavior. Existing delayed-body deadline test and happy two-request
identity/version tests remain. Production owned endpoint/credential code unchanged.

Local checks: IPC targeted test passes, all 19 SyncthingRest tests pass with eight
test threads, strict all-target daemon Clippy/fmt/diff checks pass. Independent
source review of the fixture-only corrections found no blocking finding; it
identified the request-counter evidence limitation stated above. Corrected CI
remains pending.

Mac product limitations remain separate: incoming_publication and native managed
worker are Windows-only; non-Windows staging ancestor inspection returns no pinned
handles; native Mac owner security/bootstrap completion refuses platform peer
identity unavailable. Prior passing Mac tests do not establish those product gates.

## Second native CI and environment corrections

[Run 42559236900](https://github.com/gungortarik/cryptex/actions/runs/42559236900)
on `34f58668f94ad8de969a4fc7083974b62914ba4d` failed overall. Mac library now
passes **464/464**, 4 ignored. Binary group failed 1/14 due timestamp-only
fresh_pilot_root allocation collision (AlreadyExists). Replace that test helper
with process-local atomic sequence + exclusive create/retry, no pre-delete;
32-thread uniqueness/marker regression and all 16 Windows binary tests pass.
Independent source reviewer found no blocking issue; test name narrowed to match
actual fresh-root coverage. It does not claim an injected collision test.

Windows library reported 512 passed/78 failed/5 ignored; 45 error occurrences
name strict staging ownership refusal. Hosted Windows runners are elevated;
new directories default to Administrators ownership. This duplicates the earlier
owner-elevated VM environment issue. We do not weaken SID/owner checks or ignore
these tests. Other failures require reassessment after fixing execution identity.

CI-only standard-user launcher prebuilds exact daemon lib/bin test artifacts from
Cargo JSON, creates a crypto-random disposable Users account, grants source/binary
read/execute and scratch-only write, then starts a hidden credentialed process
with its profile loaded. Child checks SID, non-admin token, profile/temp ownership,
uses its own profile/appdata/private TEMP, and retains original CARGO_MANIFEST_DIR.
All ordinary tests execute with no new filters. Each executable is enumerated
before execution; successful final summary must cover passed+ignored inventory,
zero failures/filtered entries, nonzero passed tests and zero process exit. Nested
process-fixture summaries are allowed before the final root summary. Native
stderr is captured as text through ProcessStartInfo to avoid PowerShell5.1 error
record promotion. Daemon doctests remain a separate cargo invocation.

Parent observes process completion, verifies child proof, waits bounded profile
unload and removes only the recorded fresh SID/profile/account. Scratch deletion
checks the resolved generated path under RUNNER_TEMP; cleanup failures fail CI.
Failure diagnostics stay only on the disposable runner; no profile/key archive.
Existing different-user IPC ACL job is unchanged.

Local checks for the launcher: PowerShell AST parse, non-CI refusal before account
creation, complete-summary validator accepts complete/nested results and rejects
nonzero exit, missing/zero/filter/mismatched inventories. No local host account,
profile or credential process was created. Independent source review found no
launch blocker and recommended stderr-safe capture/separate account cleanup;
both were applied. Actual hosted credential/profile compatibility remains pending.
