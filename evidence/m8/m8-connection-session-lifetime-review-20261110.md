# Product connection/session lifetime corrections and restart checks

Status: **bounded product corrections and exact native CI passed; M8 OPEN**.
Source candidate: `9dd0a40fbd5aecd6f534702158ac7a00fcee58d6`.
This work uses the accepted Windows pipe / Mac Unix transport. The separate
[signed Mach experiment](m8-macos-signed-bootstrap-owner-receipt-20261110.md)
is not integrated or enabled by this change.

## Corrected behavior

`ApprovalSessionRegistry` now checks that each `ApprovalConnection` belongs to
its exact registry allocation before issue, consume, signer access, cancellation
or invalidation. Checking the random connection ID alone did not enforce that
API lifetime contract: a connection retained after registry destruction could
previously be supplied to a replacement registry's issue method, whose grants
would then not be invalidated by that connection's Drop. Current production IPC
creates connections from its own process-local registry; this finding is not
evidence that a wire client carried an object across an actual process restart.

The retained Weak allocation identity provides the independent membership check;
no capability is persisted and no caller chooses a registry generation. Existing
WrongConnection typed errors refuse foreign connections before grant mutation or
signer access. Tests cover an old destroyed registry, deliberate equal-ID foreign
registries, preservation of the genuine owner's grant, old-capability refusal
after replacement and fresh grant single use.

Desktop's real bootstrap-confirm/cancel client now requires the operation-specific
response: confirm expects signed, cancel expects cancelled. Generic result is
accepted only as a refusal without signature. Cancellation cannot return a
signature, and accepted confirmation requires a nonempty signed message. Protocol
and request-ID correlation remain required. Both confirmation families retain
the original IPC stream and remove their Desktop-local handle before exchange;
connection loss cannot trigger a reconnect/replay of that capability.

Three native isolated socket/pipe tests exercise the actual Desktop client
functions: bootstrap confirm/cancel loss, owner confirm/cancel loss and exact
response correlation (including genuine success/refusal cases). Their streams
are test fixtures, not authenticated product Desktop/daemon image custody or a
canonical mutation proof.

## Real daemon restart test

The existing disposable-account integration test launches the actual daemon using
the production await_ready helper, verifies serving PID, kills/reaps it, starts
a replacement and requires fresh readiness and fresh health connections. It now
checks all three supported read-only roles before and after restart, verifies
the health client's deliberate local unknown-role refusal, and sends an actual
content request claiming Desktop. That untrusted test-parent request must be
refused as unauthorized_client_process by the expected actual daemon with no hits.

The existing Mac old connected stream must reach EOF/error and refuse a later
write. That raw old-stream observation is not claimed on Windows. Both platforms
still require different child PIDs and absence of canonical events. The fixture
does not run on the owner's shared endpoint; it requires disposable hosted CI,
and Windows additionally verifies a real standard-user SID/private TEMP owner.

## Review and verification record

Independent source review found the first scoped Windows summary validator would
incorrectly demand zero filtered tests from a filtered library run. The corrected
wrapper independently enumerates full and selected inventories and requires exact
selected passed+ignored count, exact full-minus-selected filtered count, zero
failures and zero ignored for the scoped suites. It still requires exact artifact
and suite counts and verified nonadmin identity. Local PowerShell7 and Windows
PowerShell5.1 regression: three positive/six refusal summaries pass.

Initial94bfc94 CI42955888018 was cancelled after that source-review finding.
Run42956046298 on11e7ba1 passed format/native lint but the newly added startup
assertion failed on both platforms: health_ping rejects an unknown role locally
as InvalidData before connecting, rather than receiving server PermissionDenied.
Correction9dd0a40 preserves the strict local diagnostic and separately requires
the real server-side content refusal. Existing crash/readiness/old-stream
assertions were not weakened. Final source-only review found no actionable issue;
reviewer performed no executions or edits.

Local native compilation could not establish a pass: the reused target produced
missing-crate cache errors, and a fresh temporary target's build scripts were
blocked by Windows Application Control4551. No policy was disabled or exception
added. Workspace formatting, diff checks and PowerShell parser/validator checks
passed locally. Exact-source native CI is the execution gate, not these local
static checks.

Final [CI42956476071](https://github.com/gungortarik/cryptex/actions/runs/42956476071)
on exact9dd0a40: **SUCCESS**. Completed native job logs and step conclusions were
verified; this is scoped startup/session CI, not the entire readiness/release
matrix (those jobs were deliberately skipped).

| Scoped check | Windows job130312679118 | Mac job130312679287 |
| --- | --- | --- |
| Actual child/readiness/crash/reconnect suite | 9passed/0failed/0ignored | 10passed/0failed/0ignored |
| Desktop Rust foundation | 46passed/0failed/0ignored | 39passed/0failed/0ignored |
| Approval registry/signing/journal lifetime | 17passed/0failed/0ignored | 17passed/0failed/0ignored |
| Original-stream loss and response correlation | 3passed/0failed/0ignored | 3passed/0failed/0ignored |
| Workspace format, daemon/Desktop all-target clippy, frontend build | passed | passed |

Filtered library inventories were checked exactly: Windows608 total gives591
excluded for17 selected and605 for3; Mac475 gives458 and472. Startup suites have
zero filtered tests and include their worker entry. Windows startup and approval
checks used the actual standard-user wrapper, whose identity/owner checks and
account/profile cleanup completed successfully; summary-regression tests also
passed. Mac old-stream EOF/write refusal is included only in the Mac suite.

Exact Git-source SHA256 values:

- approval_session.rs: `6ac076c4c456a2e9c2dc61fbb6caaf15c84dd352fd8075c3ed8632e00b2ca337`.
- ipc.rs: `f79116709c5ad30bae3210d28c1b49d5c142ddeaf74ac289b20e700cb53767d2`.
- desktop_startup.rs: `7d18f9daa93f5a972a51f3c5c849bfc2042961646e6d0877b7dbf65dbe1553a6`.

These results cover shared production helper/client code and actual daemon
processes. They do not claim physical Tauri UI observation, signed packaged
product custody or actual privileged approval sessions across a product restart.

## Remaining product boundary / exact next decision

These corrections do not enable Mac M8 mutations, integrate Mach, accept a release
publisher or prove concurrent bundle replacement. Actual authenticated product
owner-session restart, Unix per-frame sender attribution and Mac managed
publication/transport parity remain open. No new owner Mac/VM package is requested.

The [concrete Mac integration proposal](../../docs/proposals/m8-macos-owner-channel-integration-decision.md)
recommends a bounded, initially disabled candidate carrying the M8 pairing/owner
request families over a sender-attributed Mac adapter while retaining other Unix
IPC. This requires an explicit integration/ADR decision under AGENTS.md; the
earlier experiment approval does not silently authorize this product change.
Do not close M8 or move to M9 from this scoped evidence.
