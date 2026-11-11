# M8 transport-neutral approval sessions, still disabled on Mac

Product source: `449dc3c61c8b5263ab22d1caffbe6524ffd5fe95`.
Final test source: `e0da25e1f72d1c0407c07f145b2d9bbfa6b94880`.
Scope: the remaining prepare/confirm/cancel/complete domain/session extraction
authorized by [ADR-0020](../../docs/adr/ADR-0020-macos-m8-owner-channel-candidate.md).
This extends the [first boundary/reply seam](m8-disabled-macos-owner-adapter-20261110.md).
It does not implement a native authenticated Mac owner channel or close M8.

## Implementation

Private `ipc/approval_domain.rs` owns preparation, one-use pending approval work,
signing/cancellation and durable completion without a Stream. A pending session
freezes its original runtime root, admission allocation, secure-store environment,
store, expected Desktop process, registry allocation, request ID and verified
snapshot/prefix. It does not clone SearchContext's operation lease, accept a
replacement context or expose Clone/serialization/authority factories.

The owned ApprovalConnection invalidates ephemeral grants on every consumed
finish/rejection or abandoned/error path. Losing a connection is not a durable
cancellation or rollback. Already journaled signatures/anchors remain recoverable
through existing journal semantics. Existing cryptographic validators, signing
keys, canonical writer and current owner-operation policy checks remain in use.

The socket wrapper keeps platform/envelope/actual-process checks, bounded frame
I/O and deadlines. AdmittedReply owns admission through actual prepared/final
delivery. Owner think-time holds no operation permit; followup reacquires from
the frozen original admission. Domain entry additionally checks the exact
admission allocation, rejecting a permit from another incarnation even if its
runtime path is identical. This instance binding is not peer authentication.

Mac product gates still refuse all M8 families before Unix/domain/native access;
the existing four native Mac owner IPC tests remain ignored. Pure domain and
isolated socket fixtures explicitly do not establish native Mac sender authority.
No environment/UI/wire switch, native backend, permissive proof constructor or
release signer was introduced. Ordinary Unix APIs and Windows pipes retain their
existing transport and wire contracts.

## Verification ledger

Eleven added domain/wrapper tests cover abandonment and grant burning without
durable cancellation, terminal mismatch and a fresh review, replacement registry,
same-path foreign admission, frozen runtime/store, owner cancel/one append,
prepared/final delivery failure, think-time exclusion and durable completion
requiring journaled owner approval followed by immutable no-op replay.
The parked-session fixture also seeds an existing request lease and drops its
external owners before exclusive admission, detecting an accidental full-context
clone that would retain admission throughout owner think-time.

Windows scoped CI now selects the complete IPC unit inventory with an actual
standard-user token, alongside registry and request/reply tests. Mac runs the
full IPC inventory and runtime-admission tests. Full/selected inventory checks,
zero-ignored Windows selection and temporary account/profile cleanup remain
required; none of the existing Mac ignores is counted as native acceptance.

Local format/diff and PowerShell parser/summary validator pass. Local native Rust
execution is not claimed: the previously recorded Application Control4551/cache
limitation remains; no policy exception or bypass was attempted.

Initial CI42963370651 exposed four new fixture mistakes on Windows and Mac: two incorrectly
expected OwnerPending rather than the existing authenticated-prefix ProofReceived
stage, and two used the intentionally v1-only reader after appending v3 security
events. The corrections retain exact stage/unsigned/canonical assertions and use
the existing version-aware, complete-inventory, lock-scoped trust verifier with
the real fixture secure-store key. Production validators were not changed.
Intermediate338af1b run42963706037 was cancelled after this diagnosis.

Independent source-only review of exact449dc3c, carried-lease338af1b and final
e0da25e found no actionable finding or weakened invariant. The reviewer ran no
commands/tests/native binaries and made no edits. It specifically confirmed
that the test corrections retain authenticated v3 records and do not require
content retrieval readiness after a legitimate self-retirement.

Final [CI42963863735](https://github.com/gungortarik/cryptex/actions/runs/42963863735)
matches exact `e0da25e1f72d1c0407c07f145b2d9bbfa6b94880`: **SUCCESS**.
Completed Mac job130336443408: startup10, Desktop39, registry17, pending-client3,
closed request/reply9, product-gate2, complete IPC71passed/0failed/4existing
ignored and runtime-admission5 passed. All11 new domain/wrapper tests passed.
Mac library inventory497: selected17/3/9/2/75/5, excluded480/494/488/495/422/492.
Windows job130336443128: startup9, Desktop46, registry17, complete IPC80 and
closed request/reply9 passed, all zero failures/ignored. All11 new domain/wrapper
tests passed. Library inventory628: selected17/80/9, excluded611/548/619. Both
actual standard-user invocations confirmed temporary account/profile cleanup.
Native format, all-target lint and frontend build passed on both platforms.
Actual completed logs, step conclusions, exact head and all listed summaries/
inventories were checked. No full readiness/release/native authenticated Mac
product or human interactive acceptance is inferred from this scoped run.

Exact final Git-source SHA256 (UTF-8/LF blobs, not checkout line-ending hashes):

- `ipc.rs`: `2af64fe2ea42073f49dd603ee3d23125e0aaaa3d7c854a44aeaae91ccd08c78a`.
- `ipc/approval_domain.rs`: `fa859977aa74677427649dd2d705f20a1e905e8108ecd904b066f5d0bf859766`.
- `ipc/tests/approval_domain_tests.rs`: `a820a6cea4c2328198225e8ad6171d0e59f370ff0f3547e6b7201e23791c3206`.
- `runtime_admission.rs`: `30a9868d6efc172808942ebc340144a692becd84f26089a9f3efd40a346e789e`.

## Exact continuation

The transport-neutral exchange and approval/session boundaries are implemented.
Next: review the product-image/stability and long-lived native-right ownership
policy, then implement the disabled native backend and kernel-verified exact
request authority. Product signer/minimum OS selection, actual signed native
startup/restart/forwarding/bundle-change/cleanup acceptance, activation, platform
parity and final M8 reviews remain open. No new owner Mac/VM run is requested for
this domain extraction. Do not repeat old packages or the manual transcript loop.
