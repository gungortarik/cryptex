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
