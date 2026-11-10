# Desktop startup/restart correction and Mac custody review

Status: scoped implementation; M8 remains OPEN. Product helper introduced in `4229585`; reviewed fixture/CI candidate: `867c893`.
This supersedes the next-step wording in the preceding raw Mach experiment;
it does not revoke its eight successful synthetic cases.

## Change connected to the real Desktop startup path

`start_daemon_inner` now calls `cryptexd::daemon_startup::await_ready` after
spawning its actual sidecar. Automatic restart uses that same path.
Previously a missing, unreadable or unparsable ready file fell back to the
child PID and startup could report success after the readiness timeout.

The shared helper now:

- requires a complete newline-terminated PID equal to the owned `Child.id()`;
- tolerates an empty/partial file during the daemon's ordinary PID write;
- checks child exit before and after reading a complete readiness claim;
- caps the read at65 bytes, accepts at most64, and refuses nonregular files;
- refuses Unix symlinks/FIFOs using no-follow/nonblocking open and rejects
  Windows reparse points;
- kills and reaps the exact owned child after refusal/timeout, with typed errors.

A ready file and a health response are progress/liveness signals, not proof
of the serving process's trusted package or exact generation. No new Mac
authorization rule, Mach transport, credential handoff or dependency was activated.

## Verification

Local Windows initial `4229585` run passed8/0failed, including the actual
child launch/crash/replacement. After fixture isolation and an additional
scheduler-independent incomplete-file regression, the ordinary local suite
passed8/0failed/1ignored on `df024ab`. The endpoint-using fixture is deliberately
ignored outside disposable CI: a failed health ping alone cannot prove that a
slow owner daemon is absent, so ordinary developer test runs must not overwrite
its shared endpoint. CI explicitly includes the ignored test, and Windows now
uses the existing verified standard-user/SID/private-TEMP runner. That mode
requires the complete enumerated inventory with zero ignored tests.

`cargo +1.98.1 clippy --locked -p cryptexd --all-targets -- -D warnings`,
workspace format/diff checks and PowerShell parser checks passed locally.
A previously reused target directory produced missing-crate cache errors;
a fresh temporary target compiled and passed without source/dependency changes
to work around them.

Exact-candidate native CI:
[42906355843](https://github.com/gungortarik/cryptex/actions/runs/42906355843),
source `867c893dfaa1c8a3ba83b38170b15e189fd49c96`: **SUCCESS**.

| Scoped check | Windows | macOS |
| --- | --- | --- |
| Startup/malformed-readiness/crash/reconnect suite | 9 passed, 0 failed, 0 ignored | 10 passed, 0 failed, 0 ignored |
| Desktop debug Rust foundation | 46 passed, 0 failed | 39 passed, 0 failed |
| Daemon/Desktop all-target clippy, workspace formatting, frontend build | passed | passed |

Each startup count includes one subprocess-worker entry. Windows ran under a
real nonadmin token with verified matching SID/private TEMP ownership, complete
inventory and zero ignored tests; temporary account/profile removal reported
success. Mac includes the actual old connected stream and FIFO/symlink checks.
No physical UI observation, release signing, full M8 readiness/ACL rerun or
replication activation is inferred from this scoped success.

Earlier attempt42905162029/source4229585 failed: a Unix-only test trait import
was missing, and the elevated Windows runner's actual daemon exited before
readiness. The import was restored without relaxing assertions; the Windows
fixture was moved to the established standard-user harness. Intermediate
42905704821/source df024ab passed the Mac startup step, including the real old
connected stream EOF/error and subsequent write refusal. FIFO/symlink refusal
checks are also Unix-specific and are not claimed as Windows old-stream evidence.

Source-only independent review found no remaining actionable helper/wiring issue
on4229585, and confirmed fixture-isolation/deterministic-partial fixes ondf024ab.
It also reviewed the later standard-user wrapper extension; its suggested
zero-ignored inventory check was added. Product helper SHA256 throughout:
`6fc632f1dc63a62be8871f883a94e0e4bc0bd444670812e20afe63b0df58f0c6`.
The PowerShell summary validator was also checked locally against a complete
positive summary, refusal of a skipped startup fixture, and default-suite
compatibility. Agent source review is not the required final human diff acceptance.

The integration test exercises the shared production helper with the real daemon;
it is not a physical Tauri UI launch, signed application-bundle custody proof,
approval-session restart proof, PID-reuse proof or native replication acceptance.

## Why the Mac identity candidate was withdrawn

Source-only independent review found two blocking issues in the newly written
candidate before activation. That draft was removed from product code and
archived outside Git at `%TEMP%\cryptex-m8-launch-review-20261110` for diagnosis.
No runtime credentials/token values are in that archive.

1. **Non-atomic pipe inheritance.** `pipe()` followed by separate CLOEXEC calls
   leaves a window for another thread's process creation to inherit an end.
   Prefilling/closing the writer before the intended spawn does not close that
   creation-time window. Locking only this helper does not cover native/framework
   process creation. Sole-child custody cannot be claimed from this draft.
2. **No trusted product-image anchor.** Deriving the designated requirement and
   image hash from a sibling path validates equality with that path's image,
   not that the sibling is an authorized Cryptex release. An internally valid
   ad-hoc signature alone cannot establish release identity. This carries forward
   an existing path-policy limitation; it is not a newly demonstrated compromise
   of accepted read-only health IPC.

There is also an independent boundary: XNU `LOCAL_PEERTOKEN` obtains a live
socket-owner sample from the peer socket's `last_pid`. It is not a Mach receive
audit trailer. Sampling it before/after a frame does not prove the writer of
that frame when an already-connected FD is voluntarily forwarded and shared.
Do not transfer the raw Mach experiment's sender-attribution claim to Unix IPC.

Review also caught the initial readiness draft's empty-file race; the landed
helper waits for the completed line and has an explicitly synchronized empty/
partial-file regression. Invalid-FD ownership and cleanup issues belonged to
the withdrawn handoff draft and are not present in the landed readiness helper.

Primary references: [Apple public pipe declaration](https://github.com/apple-oss-distributions/Libc/blob/main/include/unistd.h),
[XNU Unix socket credential implementation](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/uipc_usrreq.c),
[XNU socket process attribution updates](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/uipc_socket.c).

## Exact continuation and completion gates

Do not repeat the owner's successful Mach/API/signing or Windows pairing/transfer
packages. No current VM or Mac action is requested.

1. Finish launch custody: establish a trusted packaged-image anchor and a reviewed
   inheritance-safe mechanism on the supported minimum Mac target. Evaluate the
   accepted Unix socket boundary first. A possible socket-first enrollment can
   avoid a pipe but still needs native race/forwarding review and an accepted
   authorization policy. This is research direction, not a selected replacement.
   ADR-0019's Mac candidate wording remains unaccepted; it also needs its
   impossible "close parent reader before spawning" sequence corrected before
   any implementation is called conformant. Keep Mac owner mutation gates closed.
2. Implement/review Mac managed profile/worker and pinned incoming publication,
   including no-follow/volume/no-replace/directory durability and restart parity.
3. Verify actual device offline convergence and restriction propagation through
   the supported integration, reusing earlier Windows receipts where applicable.
4. Complete exact-candidate security/platform/product checks, protocol acceptance
   and required human diff review. Only then close M8 and start M9.

These are four substantial work blocks, not four remaining test commands.
There is no defensible completion percentage or fixed finish date while the
Mac custody and publication implementation gates remain open.
