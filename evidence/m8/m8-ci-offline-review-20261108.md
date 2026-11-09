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

## Hosted launcher first execution

[Run 42560323415](https://github.com/gungortarik/cryptex/actions/runs/42560323415)
on `92d3f5b176f2348e503f8171912436ebba99b576` reached credential child startup:
SID/non-admin checks and profile existence passed, then Get-Acl could not autoload
Microsoft.PowerShell.Security because Windows PowerShell5.1 inherited PS7 module
paths. Daemon tests did not run in that child. Parent reported child test failure;
no account/profile cleanup failure was reported. IPC cross-account job passed;
Mac passed daemon library/binary and continued toward release Desktop tests.

Child now sets process-local PSModulePath to its own PSHOME\Modules before any
module-dependent command. Native WindowsPS5.1 security-module/Get-Acl loading was
verified locally read-only; no account created on owner host. The fix changes
only test-launcher environment, not machine policy, production code or ownership.
New hosted run must verify the complete daemon test/cleanup path.

Run [42560988740](https://github.com/gungortarik/cryptex/actions/runs/42560988740)
on `bd8c3c6fd6be215017629a0ad58e9c4ab8cdcdd9` passed module loading but stopped
at the launcher's profile-owner check before daemon execution. Environment-based
GetFolderPath is not adequate proof of the credential process profile. The child
now resolves its profile with GetUserProfileDirectoryW using its actual verified
WindowsIdentity token, checks a bounded result against the fresh account name,
then replaces inherited USERPROFILE/app-data values. The existing strict
SID-owned private TEMP check stays in place before compiling the small API wrapper.
OS-managed profile directory ownership is not substituted for canonical runtime
ownership; all production runtime checks remain unchanged.

Official contract: [GetUserProfileDirectoryW](https://learn.microsoft.com/en-us/windows/win32/api/userenv/nf-userenv-getuserprofiledirectoryw).
Token query and wrapper compile were verified locally read-only, no account created.
Independent source reviewer found no blocker in the bounded API/token use or final
summary inventory logic. Hosted full test execution/cleanup is still required.

## Complete standard-user library execution exposed long metadata paths

[Run42561855133](https://github.com/gungortarik/cryptex/actions/runs/42561855133)
on `802f95d1b832df3b3460325217a7e676cf5efc9a` completed: Mac readiness and
Windows different-user IPC ACL passed. Windows daemon library ran **587 passed,
3 failed, 5 ignored**, in41.60s. Credential launch/private TEMP/token profile
resolution therefore reached the full library; parent reported no cleanup failure.
Windows readiness remains failed. Three remaining failures:

- import_staging::tests::local_policy_batch_verification_rejects_unapproved_candidate_device
- ipc::tests::bootstrap_completion_cannot_bypass_journaled_owner_approval
- ipc::tests::bootstrap_exchange_requires_the_current_adapter_identity

All fail preparing signing public metadata: seed persisted, raw MoveFileExW returns
OS3 (path not found) under long runtime paths. Two new Windows >260-character
Unicode-path regressions, for exclusive signing metadata and atomic mutable public
metadata, independently reproduced OS3 before the fix. No test path was shortened.

A shared Windows metadata encoder now resolves ordinary DOS/UNC paths without
filesystem lookup and adds extended UTF-16 prefixes/backslashes; existing verbatim
paths retain literal semantics. NUL/device namespaces are refused. Relative paths
retain absolute Win32 resolution semantics. It is explicitly not a path-security
validator. Strict import staging conversion, pinned handles, owner/reparse/volume
checks are untouched. Signing placement retains no-replace + write-through;
mutable metadata retains replace-existing + write-through. No seed/key rotation,
canonical mutation, policy relaxation or transport activation.

Long signing regression proves recovery from an injected post-seed metadata error,
same seed on retry/reload, and native refusal of a competing destination with both
files unchanged. Mutable metadata regression proves create/replace at long paths
without temporary leftovers. Encoder tests cover drive/forward slash/Unicode,
UNC, already-verbatim, relative paths and invalid namespace/NUL refusal. UNC
encoding is tested without contacting a share; no network filesystem durability
claim is made.

Local corrected checks: event key14/14, metadata5/5, encoder2/2, previously failing
policy test1/1; strict daemon all-target Clippy, format and diff checks pass.
Independent source reviewer found no actionable regression and checked pinned
Rust1.98.1 absolute-path implementation: verbatim paths unchanged, ordinary paths
use GetFullPathNameW without link resolution. Review was source-only, not execution
or human certification; native no-replace and persisted-seed behavior unchanged.
Exact-candidate hosted Windows/macOS CI remains required.

## Same-defect caller audit: Brain identity and pairing journal

Final source audit found the same raw MoveFileExW encoding in exclusive Brain
identity creation and mutable bootstrap journal replacement. These are part of
the same device-enrollment/trust persistence path, not unrelated refactoring.
Two additional >260-character Unicode-path regressions reproduced OS3 before
correction; both callers now use the same metadata encoder. Identity flags remain
no-replace/write-through; journal flags remain replace/write-through, with existing
strict journal locking/validation/cleanup unchanged.

Regression coverage: public Brain create/load plus native occupied-destination
refusal leaves both files unchanged; actual journal store creates, updates and
reopens retaining two unique transactions. Local identity11/11, journal15/15,
strict daemon all-target Clippy/fmt/diff pass. Existing staging adapter untouched.
Run42563020408 onfcfb65d was superseded before completion so the next full hosted
run covers all final metadata callers. It is not claimed as a successful CI run.
Independent source-only review found no actionable issue in these two callers;
typed cleanup/placement errors, journal validation/locking and native move flags
remain intact. No executable was run by the reviewer.

## Full writer long-path follow-through

[Run42563317369](https://github.com/gungortarik/cryptex/actions/runs/42563317369)
on18774b2baa8cd0096ef7475253154c2c89536807 completed with Mac readiness and
Windows IPC ACL passing. Windows library **595passed/1failed/5ignored**, 34.64s.
The sole failure was the original unapproved-candidate policy fixture, now past
enrollment but failing canonical append with Path(OS0). Full suite remains failed.

A new full writer regression under a >260 Unicode path independently reproduced
Path(OS0). Correcting tip encoding then revealed Sequence(OS3). Source audit also
identified unencoded sequence initialization/counter, segment seal, local device
and install identity moves. They now share the already reviewed encoder, moved
without algorithm changes to storage's Windows-only platform module; daemon
platform reexports it using its existing storage dependency. No duplicated
algorithm or reversed dependency. Existing strict staging/quarantine/capture
path adapters remain untouched; encoding is not a containment validator.

Exclusive marker/local/install/segment moves retain WRITE_THROUGH without
REPLACE_EXISTING; derived tip and counter retain REPLACE_EXISTING|WRITE_THROUGH.
Writer tip last_os_error is captured before cleanup, retaining the real failure.
No canonical naming/event bytes, counter/lock policy, trust rule, seed lifecycle
or activation changed.

Full writer regression appends and seals two signed events, reopens verification
and proves prior sealed bytes unchanged. Foundation/enrollment regression reloads
the same install/device identities and one stored seed. Storage regression invokes
native no-replace against an occupied destination, proves both files unchanged,
then seals successfully and replays identically. Local checks pass: writer29/29,
sequence16/16, localidentity11/11, install10/10, storage44/44, daemon long-path7/7,
strict all-target daemon/storage Clippy, workspace format and diff checks.
Independent source-only review found no actionable issue and verified native
flags, typed errors, coverage and helper move; no reviewer binaries executed.
Next exact-candidate hosted run remains required.
