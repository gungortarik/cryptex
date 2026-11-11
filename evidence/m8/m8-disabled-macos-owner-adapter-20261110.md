# Initially disabled Mac M8 adapter: boundary and first domain seam

Status: **candidate foundation implemented; native authority NOT implemented**.
Owner approval and bounds: [ADR-0020](../../docs/adr/ADR-0020-macos-m8-owner-channel-candidate.md).
This receipt does not enable a Mach channel, select a product signer or close M8.

## Implemented boundary

Source913a5f2 moves the existing request wire fields/serde attributes unchanged
into `m8_owner_channel/request.rs`, with shared limits and a separate closed
ten-method decoder. That decoder validates untrusted envelope/shape only. It
cannot construct peer authentication, approval authority or canonical-write access.
It rejects duplicate/unknown fields, claimed kernel-proof fields, incompatible
roles/capabilities/protocol, cross-stage payloads and oversized data. Signatures,
current adapter ownership and canonical trust remain domain-service checks.

Every Mac Desktop M8 request refuses with mac_owner_channel_disabled before Unix
connection or pending-session/native access. The daemon also refuses M8 on its
product Unix entry before context/admission/secure-store/journal/canonical access,
even when that connection otherwise recognizes Desktop. All three families move
together, including public Brain identity/offer/proof. Unknown family members
cannot fall back. Ordinary APIs and Windows pipes retain their existing paths.
There is no environment, wire or UI switch that can enable this candidate.

Old domain/protocol fixtures now explicitly choose the private cfg(test)-only
DomainFixture delivery, preserving domain regression coverage without pretending
to authenticate a Mac product channel. That variant is absent from shipped code.
New negative tests instead use the actual ProductSocket entry and all ten actual
Desktop APIs, checking refusal for both recognized/unrecognized peer states with
no domain context available. Existing Windows wire/sign/journal semantics remain.

## First transport-neutral domain/reply seam

Sourceca5665d moves the existing response wire fields unchanged into OwnerReply.
The existing exchange wrapper retains admission -> peer/envelope/shape checks ->
domain execution ordering. Its private executor now returns an owned typed reply
without a Stream parameter. It reuses the existing identity/offer/proof signers
and journal functions; no client writer or alternative cryptography is introduced.

AdmittedReply owns the exact serialized bounded response and OperationPermit
through its single bounded synchronous send. The actual current send callback is
the existing write_response, not a background task. Expiry, send error, drop,
oversize serialization and unwind release admission; delivery failure is not a
claim that an already durable operation was cancelled. No request proof, grant or
native peer authority can be obtained from this reply type.

Three lifecycle tests use stop_and_drain(now): while a real permit is held this
must report Busy, and after the send/error/unwind/drop scope it must drain. This
avoids a deadline-only assertion that could pass without holding a lease. The
existing original-stream and registry lifetime suites remain separate regressions.

## Review / verification

Independent source-only review of913a5f2 and exactca5665d found no actionable
finding. Reviewer executed no tests/scripts/native binaries and made no edits.
It confirmed no production DomainFixture, fake Stream/peerproof factory, native
authority, environment activation or Mac Unix fallback. New module/session tests
are not claimed as an authenticated native Mac product path.

First-slice [CI42959708363](https://github.com/gungortarik/cryptex/actions/runs/42959708363)
on exact913a5f299114354efc3584ff7c0e0899ef4d1943: **SUCCESS**.
Actual completed job summaries were checked:

| Check | Windows | Mac |
| --- | --- | --- |
| Startup/readiness/crash/reconnect | 9pass/0fail/0ignored | 10pass/0fail/0ignored |
| Desktop foundation | 46pass/0fail | 39pass/0fail |
| Approval registry | 17pass/0fail | 17pass/0fail |
| Pending stream client helpers | 3pass/0fail | 3pass/0fail |
| New closed method/request contract | 6pass/0fail | 6pass/0fail |
| Actual Mac outbound/Unix gate fixtures | not applicable | 2pass/0fail |
| Complete Mac IPC unit inventory | not run in this scoped Windows job | 60pass/0fail/4existing ignored |

Windows scoped library inventory614: selected17/3/6 and excluded597/611/608,
with zero ignored; actual standard-user identity/private TEMP checks and account/
profile cleanup passed for both invocations. Mac inventory483: filters17/3/6/2
excluded466/480/477/481; full IPC selected64 includes four preexisting native
owner-operation ignores with419 excluded. Those ignored tests remain open, not
newly skipped or counted as native success. Format, all-target clippy and frontend
build passed on both platforms.

Final [CI42960387437](https://github.com/gungortarik/cryptex/actions/runs/42960387437)
on exact`ca5665d6a19dc7935a87d04d246d89be9f24b28b`: **SUCCESS** for the
additional admitted reply seam. Actual job130325266121 Windows and130325265697 Mac
logs, exact head and step conclusions were verified. Startup9/10, Desktop46/39,
registry17/17, pending-stream3/3 and new owner-channel contract/reply9/9 passed
with zero failures. The Mac product-gate2 tests also passed; full Mac IPC remains
60passed/0failed/4preexisting ignored. All-target lint/format/frontend passed.

Final library inventories: Windows617 (selected17/3/9, excluded600/614/608);
Mac486 (selected17/3/9/2, excluded469/483/477/484); full Mac IPC has422 excluded.
New scoped suites have zero ignored. Both Windows standard-user cleanup markers
confirmed temporary account/profile removal. No full readiness, installer,
signed native product or owner mutation execution is inferred.

Exact Git-source SHA256:

- ipc.rs: `aef6a4d5ba20413f1b8092a298b46af6047ebaa94743955b04bae5d60d5f7527`.
- mod.rs: `499a6f9175a56a911fca165f088ffe9016175f2f8bb97103a289128e6791d9b3`.
- request.rs: `2a0c746191083ff71c09ef23de21e14d709b084f6080046bfda90e778161d3bf`.
- reply.rs: `855f38df2b1c71e61c7b8e14614ec04d1ced082493d73b14acc9f8ea0a8aafc1`.

Local workspace format/diff and PS summary tests pass; local Rust execution is
not claimed because the earlier App Control4551/build-cache limitation remains.
The native CI is the execution evidence. No new Mac/VM owner package is needed.

## Explicit remaining implementation

| Component | State |
| --- | --- |
| Approval/ADR scope and fixed default-off product boundary | implemented |
| Shared closed request/response wire contracts and bounded reply admission | implemented |
| Exchange identity/offer/proof domain executor without Stream | extracted, behind existing authorized Windows wrapper |
| Transport-neutral prepare/confirm/cancel/complete domain/session execution | remaining; do not call legacy Stream handlers with fabricated peer authority |
| Private kernel-verified exact-request proof and native Mach receive/send/pins | NOT implemented; no production proof constructor exists |
| Configured product signing authority and stable bundle/live-image observation | NOT configured/proven |
| Product native restart/forwarding/bundle-change/cleanup acceptance | NOT run |
| Mac activation, publication/transport parity and M8 completion | CLOSED / OPEN work, respectively |

Next authorized work: extract the remaining typed domain/session boundary with
real epoch/grant invalidation and retained admission, then develop the native
backend only against a reviewed product-image/stability policy and correct
long-lived failure/right cleanup. The disposable probe's _exit cleanup and local
development signer must not be silently promoted into production. Static
signature revalidation alone is not proof against concurrent modification; see
the [documented native design boundary](../../docs/proposals/m8-macos-owner-channel-integration-decision.md).
No repeated permission is needed within the approved initially disabled scope.
