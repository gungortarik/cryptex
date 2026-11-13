# 34 — Current Implementation Handoff

## Disabled Mac transport identity continuation — November 12

The owner delegated the helper choice. ADR-0021 selects the existing signed
cryptexd in a separate transport-only role, behind the default-off local feature.
Mac private-profile, pinned-package, process-generation and established-socket
adapters are implemented. Read/offer/proof in the native fixture context require
a fresh authenticated helper observation. Ordinary product Mac/Unix gates stay closed.

Current reviewed code is `0d98a16d075ca317b208c6e3dd6b0c66b64df707`.
Both selected Mac/Windows jobs passed in
[CI 43334196206](https://github.com/gungortarik/cryptex/actions/runs/43334196206)
(`desktop_startup_only`; other milestone jobs were not run).
[Independent source review](../evidence/m8/m8-macos-transport-independent-review-20261112.md)
is complete through this code, with no remaining actionable finding. This is a
separate agent source review, not an external human audit.

Actual Mac execution passed owned established-socket REST, foreign-server refusal
before credential bytes, clean-stop ACK, same-identity reopen, helper loss,
missing-key refusal, eleven live profile/package/receipt drift refusals, native
Desktop pairing, clean shutdown, idempotent restart, daemon loss and unresolved
replacement refusal. The exported MAC-AKTARIM launcher itself ran and its
matching-source receipt verified identity/pairing/loss, credential cleanup and
runtime cleanup. Earlier content, Keychain, replay and startup refusal checks
also passed. Local Python tests total 90 (5 platform skips); format, diff and
strict native lint passed. The [machine-readable receipt](../evidence/m8/macos-transport-native-0d98a16-ci.json)
records jobs, markers and package SHA-256. The [transport record](../evidence/m8/m8-macos-transport-helper-20261112.md)
retains failed attempts, corrections and evidence limits.

The older passing `04fa52e` artifact was withheld after review found two P2 issues:
live profile/package continuity and crash-safe fixture credential cleanup. The
current code revalidates continuity before/after observations and fences cleanup
with the same exclusive profile lease, including a post-acquisition parent check.
Both `74b95bd` CI attempts passed the new checks but hit an older codesign fixture
deadline. `0d98a16` unlocks only the exact disposable CI signing keychain immediately
before certificate use; no locking policy, ACL or test deadline changed. The
latest entire scoped run and exported launcher passed after that correction.

The owner returned `MAC-AKTARIM-SONUC-0616b9b2.zip`: exact source matched, outcome
was passed and all five identity/pairing/loss/credential/runtime checks were true.
The [owner receipt](../evidence/m8/m8-macos-transport-owner-receipt-0616b9b2.md)
records its ZIP hash, closed-schema validation and evidence limits. This closes
the owner-Mac run for this default-off candidate; do not repeat the package or
manual signed-JSON exchange. The returned aggregate report does not independently
attest the device, OS version or architecture.

Next implementation/acceptance gate: combined daemon/helper supervisor loss in
the existing disabled scope, preserving unresolved-marker exclusion and refusal
to adopt orphan engines. Other endpoint/exec races, product publisher/OS-floor,
normal activation and final M8 protocol/platform/security/human gates remain
open. The local-engine/native-pairing evidence does not establish cross-device
replication or human WebView pairing acceptance; M8 remains open.

## Protected UI and native approval continuation

The default-off local feature now has a snapshot-backed actual Tauri Assets
provider and a native approval-domain driver. A separate candidate UI builder
routes its three owner review commands through the same native worker, with
opaque renderer handles. The synthetic TEMP-only mode uses one FakeSecureStore;
ordinary runtime stores and all product owner gates remain unchanged.

The [bounded contract](proposals/m8-local-content-approval-check.md) records
content binding, per-message private proof, session/admission ownership,
cancellation/delivery failure rules, WebKit isolation and evidence limits.
Implementation and independent source review through `efdb877` are complete
for this disabled scope. The [exact-source CI run](https://github.com/gungortarik/cryptex/actions/runs/43095500335)
ended in `startup_failure` again after its requeue on November 11 at 20:47 UTC.
GitHub created zero jobs. At 20:48 UTC GitHub reported degraded Actions
availability in its [service incident](https://www.githubstatus.com/incidents/dvy0t3mvh1lg).
The [evidence/continuation record](../evidence/m8/m8-protected-content-native-approvals-20261111.md)
includes the earlier passing synthetic markers, failed journal ordering check,
Windows passes and confirmed native_accept timeout. The latter was traced at
2638 ms: child snapshot verification was consuming the native 2.5-second HELLO
window. A bounded, non-authoritative stdout progress hint now schedules that
handshake after independent verification, under the unchanged outer deadline.
No image, native message, runtime-ready or owner check is bypassed. The
immutable frontend catalog is cached per retained Release only.

After recovery, a [fresh dispatch at the same commit](https://github.com/gungortarik/cryptex/actions/runs/43101602427)
passed both native platform jobs. The Mac job passed all four content/session
markers and all eight startup refusal fixtures. On the owner's Mac the signed
WebView opened, showed Active after relaunch, and recorded one cancellation
and one confirmation. The app exited with code 0, but the shutdown receipt was
missing. Candidate `a7b41a2` now finishes owner cleanup in the native exit
callback. Its [Mac and Windows CI](https://github.com/gungortarik/cryptex/actions/runs/43147508139)
passed. A real AppKit quit produced a receipt with the one seed event preserved.
The revised owner Mac result `MAC-ARAYUZ-SONUC-bf755756.zip` passed after
Cancel, Confirm and Cmd+Q: two events, original prefix preserved, one cancellation
with zero append attempts, one anchored confirmation with one attempt, exit 0.
The real Keychain prerequisite now passes at `4662171` in
[scoped native CI](https://github.com/gungortarik/cryptex/actions/runs/43151946676).
A test process seeds an isolated profile through daemon library code, then
two fresh processes load its existing OS-backed signing key and verify its
history. All profile bytes remain identical. Deleting only that fixture key
makes a later process report Missing without recreating the key or files.
The authenticated native candidate now has separate Keychain-create and
Keychain-resume modes. Both platform jobs at `e22e0ee` in
[run 43156563156](https://github.com/gungortarik/cryptex/actions/runs/43156563156)
passed. The Mac job verified actual signed Desktop/daemon same-profile restart, confirmed-history
preservation, previous-local-handle refusal and missing-key refusal.
The one-click owner Mac package is ready from product source `e22e0ee` and
collector tooling `39f45ab`. Both native platform jobs passed in
[package run 43158034824](https://github.com/gungortarik/cryptex/actions/runs/43158034824).
The exported `MAC-KALICI.command` was extracted and executed on the Mac runner,
producing a passing result with credential and runtime cleanup verified.
The owner returned `MAC-KALICI-SONUC-fc006133.zip` from the real Mac.
Its source matches `e22e0ee62dc8c477f25eac575371c3c563e4228e`, outcome is
`passed`, and all four flags are true for restart/confirmation, missing-key
refusal, credential cleanup and runtime cleanup. This closes the owner-Mac
automated durable native fixture check at that source. The disposable signer
expires 2026-11-13 04:59:28 UTC.
The replay extension at `5b6f863` passed Mac and Windows CI in
[run 43279002497](https://github.com/gungortarik/cryptex/actions/runs/43279002497).
It resends exact pending and completed confirmation requests over new native
connections to the same profile, requires missing-session refusals, and verifies
unchanged event/identity and journal bytes. The exported launcher also passed.
The owner returned `MAC-KALICI-SONUC-be33e9c4.zip` from the real Mac.
Its source matches `5b6f8635bca726cc6637fc47000e9972928f3826`, outcome is
passed and `wire_replay_refusal_verified=true` with all four earlier flags
true for restart/confirmation, missing-key refusal and both cleanup checks.
This closes the owner-Mac automated cross-daemon confirmation replay fixture
at this source.
The disposable signer expires 2026-11-13 18:22:46 UTC.
Publisher/OS-floor decisions, explicit activation and M8 completion remain open.

The [focused profile-boundary review](../evidence/m8/m8-native-profile-boundary-review-20261112.md)
identified linked-marker acceptance and a late synthetic-resume identity check.
Both are corrected at `b7b6f9b`. The marker must be an owned single-link regular
file, and synthetic Brain/device/probe validation now precedes lifecycle
recovery and repeats under the domain guard. Both platform jobs passed in
[run 43284483019](https://github.com/gungortarik/cryptex/actions/runs/43284483019).
The disabled native Desktop client now wires Brain identity, transport identity,
offer/proof, peer prepare/confirm/cancel and durable completion through its
content-bound worker to the existing daemon domain services. Native private
grants remain backend-only; peer snapshots are checked against the full signed
prefix and handles cannot cross owner/peer families. The new transport read
accepts no caller-supplied identity. The Mac managed-transport status backend
is still absent: transport read, offer and proof refuse with
`transport_identity_unavailable`, without creating a trust record. End-to-end
Mac pairing is not established by this wiring or its refusal tests.
Validation and exact-source CI status are recorded in the
[native pairing integration record](../evidence/m8/m8-native-pairing-wiring-20261112.md).
Both platform jobs passed at `6ee4893` in
[run 43291468456](https://github.com/gungortarik/cryptex/actions/runs/43291468456).
The actual signed Mac driver reported `native_pairing_boundary_refusals=passed`
and retained all existing owner, restart, private-grant and raw-replay checks.
An earlier signing-child timeout and the temporary CI keychain re-unlock are
recorded separately in that evidence file; no native deadline was relaxed.
The next investigation found that the Mac owned process/package/private-profile
and established-socket adapters are absent, including daemon-crash cleanup
equivalent to the existing Windows Job. A direct child plus Drop is insufficient.
The [concrete disabled transport proposal](proposals/m8-macos-transport-identity-binding.md)
recommends a separate helper role using the existing signed daemon executable,
with fresh process/socket observations and explicit loss/cleanup gates. This new
process-role/integration decision was delegated by the owner and the recommended
helper selected in [ADR-0021](adr/ADR-0021-disabled-macos-transport-helper.md).
Implementation is in progress behind the local feature; product gates remain closed.
Implement/prove supervision and the authenticated status
reader before successful Mac offer/proof and the complete native peer flow. Final independent
security/human review and publisher/support-floor/activation decisions remain
open under ADR-0020.

The Windows M8 pairing form now labels the four signed messages by sender
and recipient. The guided action assembles the role-specific prefix and opens
the daemon's owner review. Saved exchanges and completed transcripts can still
be pasted separately. Local verification covered the frontend build, 23 Node
tests and both roles in Edge with simulated IPC. Those simulated role checks
do not establish successful guided native Mac pairing. The latest native CI
above covers the current source and its bounded refusal/restart fixtures.

## Actual signed startup check continuation

The default-off `m8-local-startup-check` feature now connects the local certificate
and CMS inventory checks to the actual headless Desktop executable launching
the actual daemon's ordinary lifecycle. It shares the Desktop hold-command
constructor and readiness implementation. Its native frames bind both roles
to the same inventory and epoch; no owner-request authority is constructed.
The product gate remains closed.

See the [bounded contract](proposals/m8-local-signed-startup-check.md) for the
fresh-runtime, absolute-deadline, retained child/channel/snapshot and restart
rules. [Exact-source CI for `3d0b6c2`](https://github.com/gungortarik/cryptex/actions/runs/43067144663)
passed on Mac and Windows. The actual signed pair passed startup, forced-exit
restart, old-channel refusal, authenticated runtime-ready and clean shutdown.
All eight real-binary negative fixtures passed their exact refusal-stage checks,
and the disposable signing fixture confirmed complete cleanup without OS trust
writes. The feature-enabled library run also passed its named closed-gate tests.
Source-only independent review through `3d0b6c2` has no actionable finding after
correcting the handshake deadline and distinguishing parent-image rejection
from missing rendezvous. The final runtime-ready frame is authenticated after
ordinary lifecycle initialization. Local Windows startup passed 9 with one
explicitly disposable-account ignore, Desktop passed 46, shared owner-channel
passed 13, and all-target lint/formatting passed. Three Python runner regressions
passed. Native CI startup passed Mac 11/Windows 10, Desktop Mac 39/Windows 46,
shared owner-channel Mac 21/Windows 13 and IPC Mac 71/Windows 80. The two Mac
subprocess helpers were explicitly invoked, and four existing Mac IPC ignores
remain. Both Windows standard-user runs confirmed temporary-account/profile
cleanup. This is scoped startup evidence, not a full workspace or interactive
product acceptance.
The [startup evidence](../evidence/m8/m8-local-signed-startup-20261111.md)
records exact source, scoped tests and remaining product integration.
The previous test-binary-only continuation below is superseded for startup
integration; the default-off content/session continuation is recorded above.

## Local certificate profile continuation

Source `4138184` adds a local/development certificate profile independently of
the Developer ID distribution profile. The local profile pins exact signer DER,
role identifiers and image hashes. CMS policy must pin that same certificate.
The native channel retains the certificate and compares each kernel-attributed
sender's actual signing leaf before accepting its bytes.

This prepares fee-free signed integration. It does not activate product routes.
The scoped Mac CI also runs a certificate-positive parent/child exchange using
a disposable local identity. Its test-only selector signs a copy of the native
test executable and propagates an independent exact DER pin to both peers.
The CI-only runner owns a fresh private keychain and deletes its signing
material before reporting fixture cleanup. It checks hosted macOS runner scope
and refuses owner machines and self-hosted runners. It temporarily includes
the private keychain in identity discovery and restores the exact original
search list before deleting the keychain. It does not write OS trust settings.

The headless actual startup binding and default-off protected content/session
continuation are implemented above. Signed interactive application acceptance
on the owner's Mac remains separate. The earlier
parent/child test-binary fixture is component evidence. Native ad-hoc
processes additionally exercise missing-certificate rejection, while policy
and CMS tests check DER propagation and signer/profile consistency.

Independent source reviews covered native certificate ownership/FFI and the
disposable runner's creation-failure cleanup, hosted-runner scope and final
cleanup reporting. The final runner review of `88185c7` found no actionable
issue; this is source review, not native execution.
The initial import attempt exposed a PKCS12 compatibility issue. The fixture
uses an explicitly CodeSigning certificate and a macOS-compatible PKCS12
container. Exact native certificate and CodeDirectory checks retain their
original strength. The container's legacy MAC is test transport only.

The no-OS-trust runner source is `88185c7`. Two local Python regressions check
redacted unexpected failure and cleanup after search-list restoration failure.
The earlier `af85833` run completed the native exchange but failed fixture
cleanup; it is not passing acceptance evidence.

[Exact-source CI for `88185c7`](https://github.com/gungortarik/cryptex/actions/runs/43061253818)
passed on Mac and Windows. It passed the Mac certificate-pinned exchange and fixture cleanup without OS trust
writes. Mac startup passed 10, Desktop passed 39, owner-channel passed 20 with
two explicitly invoked helper ignores, and IPC passed 71 with its four existing
ignores. The selected approval-session and runtime-admission checks, native C
ownership checks, formatting and lint also passed.
Windows startup passed 9, Desktop passed 46, approval-session passed 17, IPC
passed 80 and shared owner-channel passed 13. Both standard-user test runs
confirmed removal of their temporary account/profile. This scoped run does not
claim a full workspace or real signed Desktop/daemon product-path acceptance.

## Signing prerequisite clarification

Paid membership is not required to continue local development or isolated
signed Desktop/daemon integration. The Developer ID Application policy is an
unconfigured distribution candidate. The next integration experiment can use
an explicitly pinned local/development identity under its own reviewed test
profile. Preserve certificate, inventory and process identity checks.
Developer ID/notarization and any membership purchase remain a separate owner
distribution decision. The previous continuation must not be interpreted as a
mandatory purchase gate for M8 development.
[Signing scope](proposals/m8-macos-release-image-policy.md).

## Signed inventory and immutable content snapshot

The disabled Mac candidate now verifies attached CMS signed-data with an
independently pinned signer certificate. Native Security checks the actual
signature and exact content, then evaluates offline CodeSigning trust with only
the pinned certificate as anchor. An outer DER filter refuses encrypted
wrappers before native decoding.

The strict inventory binds purpose, schema, expected release and architecture,
role hashes and paths, member lengths and SHA-256 digests. Rooted no-follow
reads reject changed, missing, symlinked, hardlinked or oversized listed members. Successful
reads return immutable owned bytes. Image expectations can only be obtained
from a completed verified content snapshot.

The snapshot protects the listed content consumed from its byte buffers. It
does not freeze the source filesystem, grant an executable path or establish the
resources of an existing Desktop process. Product signer provisioning, live
process/resource/session binding and activation remain open. Read the
[candidate contract and limits](proposals/m8-macos-release-image-policy.md)
before connecting this to the product. The earlier absent-verifier continuation
below is superseded by this section.

Source-only independent review of `e43b3f5` found no remaining actionable code
issue in this candidate after correcting the final verification deadline.
The fixture suite checks real CMS signature/content binding, alternate signer
refusal, malformed DER, signed duplicate keys and traversal, wrong release and
architecture, member replacement/growth/truncation, symlinks/hardlinks, expiry,
change between read and publication and immutable content after path replacement.
Local native testing was blocked by Windows Application Control4551.

[Exact-source CI for `e43b3f5`](https://github.com/gungortarik/cryptex/actions/runs/43049868298)
passed on Mac and Windows. Mac owner-channel tests passed 19, including the six
new inventory/snapshot tests, with two separately invoked subprocess helpers.
Mac IPC passed 71 with its four pre-existing ignores. Windows owner-channel
tests passed 12 and IPC passed 80. Startup, Desktop, native right-ownership
checks, formatting and all-target lint passed. Test-only self-signed CMS
credentials establish this implementation's behavior, while product publisher
acceptance remains open.

## Release-image policy candidate

The disabled candidate now has a closed role-specific policy representation.
It validates the Team ID, both identifiers and both expected image hashes before
selecting Desktop or daemon. Its proposed certificate constraint is Developer
ID Application. The product configuration remains absent. Test-only metadata
cannot select a product publisher.

[The release-policy continuation](proposals/m8-macos-release-image-policy.md)
records the remaining provenance and bundle-stability design. Mutual embedded
final CDHashes would create a signing cycle. An independently authenticated
release inventory and a stable bundle observation must be implemented and
reviewed before native product authorization. Publisher selection and product
activation remain open.

Source review of `214c234` found no remaining actionable issue in this bounded
policy change. Local Windows owner-channel tests passed 12/12. These checks
cover policy validation and the closed factory. They do not establish an
authenticated release inventory or stable signed bundle observation.

[Exact-source native CI](https://github.com/gungortarik/cryptex/actions/runs/43046353361)
passed for `214c234` on macOS and Windows. The Mac owner-channel suite passed 13
tests, including all three new policy tests and the signed native subprocess
checks. Its two explicitly invoked helper tests remain marked ignored for
ordinary direct test discovery. Windows passed 12 owner-channel tests.
Startup, Desktop, native C ownership checks, selected IPC suites, formatting and
lint passed. The four pre-existing Mac IPC ignores remain open.

## Private macOS Mach transport and process identity

The macOS build of the daemon crate includes a native backend under
[`m8_owner_channel/native`](../services/cryptexd/src/m8_owner_channel/native).
The product entry gate still rejects all three M8 owner families. Only the
test harness supplies a counterpart image policy.

The listening side registers a fresh per-launch name before spawning and accepts
only the child represented by its unreaped process handle. The child captures
its actual parent. Only the empty handshake uses the public rendezvous. After
the first message, the parent closes the named inbox and sends a fresh private
endpoint in its reply. Each direction uses its own monotonic sequence. Each
receive checks the kernel audit trailer against the retained task-name right,
live audit token, signing requirement and CodeDirectory hash before returning
bytes. Close owns the Mach rights and Core Foundation objects. Data calls take
the caller's absolute deadline. A late send can have delivered bytes, so its
timeout is an uncertain outcome.

The backend resolves the live signing identity using Apple's audit-token and
PID guest attributes. The policy must provide both the expected requirement and
the expected CodeDirectory hash independently of the peer. Synchronous Security
framework calls can overrun a deadline; the adapter checks again before
publishing received bytes or attempting the send.
[Apple guest attribute keys](https://developer.apple.com/documentation/security/guest-attribute-dictionary-keys).

The signed native tests exercise bidirectional delivery, a different live child
running the same image, incorrect signing requirement or hash, queued data
after exit or same-PID exec, caller expiry, a full queue and an old channel
retained across a fresh session. The C unit tests check replay, wrong epoch,
truncated or complex envelopes, namespace removal, reference counts after a
kernel send timeout and cleanup error reporting. Independent source review of
`a7ae022` closed the findings on right cleanup and test-process cleanup.

[Scoped native CI for `a7ae022`](https://github.com/gungortarik/cryptex/actions/runs/42967877705)
passed on macOS and Windows. Mac startup passed 10 tests, the owner-channel
suite passed 10, and IPC passed 71 with four pre-existing ignores. The two
explicitly invoked subprocess helpers also ran within the signed native test.
Windows startup passed 9, Desktop passed 46, IPC passed 80, and the shared
owner-channel suite passed 9. The native C checks, formatting and lint passed.

Before product integration, provide independently trusted release image policy and a reviewed
stable observation of both signed bundle members under concurrent replacement.
Then bind the authenticated request and reply lifetime to the domain session
and test the actual signed Desktop and daemon together. M8 remains open.

## Remaining approval domain/session boundary implemented (2026-11-11)

Within the already approved disabled ADR-0020 scope, source449dc3c extracts
prepare/confirm/cancel/complete into a private transport-neutral domain module.
Pending sessions freeze their original root/store/registry/admission allocation
and verified operation. Each finish/rejection/abandonment burns the ephemeral
grant without durably cancelling or rolling back an uncertain result. Bounded
prepared/final sends retain admission; owner think-time does not. A foreign
admission incarnation is rejected even with the same runtime path. Existing
socket peer checks and all Mac product refusals remain in place.

Final test source `e0da25e` adds11 domain/wrapper regressions, including a context
already carrying a lease, journaled signing/completion, unchanged canonical
prefixes and one-append/no-op replay. Source-only independent review has no
actionable finding. Exact native CI42963863735 SUCCESS: startup Windows9/Mac10,
Desktop46/39, registry17/17, complete IPC80pass/71pass, closed request/reply9/9,
Mac runtime-admission5 and Mac product-gate2 passed. All11 new tests passed on
both; the four existing native Mac owner IPC ignores remain open. Native lint,
format/frontend, exact selected/excluded inventories and both Windows actual
standard-user/temporary-profile cleanup checks were verified. This is scoped
domain/socket evidence, not native Mac authenticated owner-channel acceptance.
[Current evidence and continuation](../evidence/m8/m8-approval-domain-sessions-20261111.md).

Next authorized work: review product-image/stability and long-lived native-right
ownership policy, then implement the disabled backend with kernel-verified exact
request authority. Native backend, product signer/minimum-OS selection, signed
product acceptance and activation are still open; M8 is OPEN. No new device run
or repeated owner permission is needed for this completed domain extraction.
The remaining-session continuation in the older foundation snapshot below is
superseded by this section.

## Owner approved disabled Mac adapter; first boundary implemented (2026-11-10)

The owner explicitly approved the proposed initially disabled Mac M8 integration.
[ADR-0020](adr/ADR-0020-macos-m8-owner-channel-candidate.md) records that limited
development decision; no repeat approval is needed for this scope. Activation,
release-image policy and minimum-OS acceptance remain separate.

Source913a5f2 adds shared closed M8 request types/limits and default-off routing:
all10 Mac Desktop M8 calls refuse before Unix/session/native access; daemon Unix
inbound refuses M8 even for a recognized Desktop before domain/context access.
There is no runtime enable switch or fallback. Windows request serialization and
routes remain unchanged. Domain IPC fixtures are explicitly test-only, distinct
from product delivery; the new product-gate tests never use that bypass.

Sourceca5665d additionally extracts the exchange identity/offer/proof domain
executor without a Stream and shared typed replies owning admission through
actual bounded response delivery (including expiry/error/drop/unwind). Exact
CI42960387437 SUCCESS: startup Windows9/Mac10, Desktop46/39, registry17/17,
pending-stream3/3, new contract/reply9/9, Mac product-gate2 and full Mac IPC60pass/
4preexisting ignored, zero failures. Native lint/format/frontend passed; Windows
standard-user identity/cleanup and full/selected inventories verified. Independent
source review has no actionable finding. [Evidence and implementation ledger](../evidence/m8/m8-disabled-macos-owner-adapter-20261110.md).

Native Mach receive/send/process pins, product release-image policy/stable bundle
observation and a kernel-verified exact-request authority constructor are NOT
implemented. Prepare/confirm/cancel/complete domain/session extraction also remains.
Do not call this a complete native adapter or owner authorization. Next authorized
work: remaining typed domain/session boundary with epoch/grant invalidation, then
native backend against reviewed image/stability and long-lived right-cleanup policy.
No repeat permission or device run is needed yet. Mac activation stays CLOSED;
M8 is OPEN. Older decision-pending/no-integration-authorized notes below are historical.

## Product connection/session corrections passed; Mac integration decision pending (2026-11-10)

Source9dd0a40 binds approval connection objects to their exact registry allocation,
correlates Desktop confirm/cancel responses and tests one-use handle loss on the
original socket/pipe. Actual daemon crash/restart now also verifies supported
read-only roles and server rejection of an unauthenticated content client before
and after restart. [Changes, corrections and execution limits](../evidence/m8/m8-connection-session-lifetime-review-20261110.md).

Independent source review has no remaining actionable finding. Local format,
diff and PS5.1/PS7 summary-validator checks pass. Local native build is blocked by
stale target metadata and then Application Control4551 in a fresh temporary target;
no exception/policy change was attempted. Exact CI42956476071 on9dd0a40 SUCCESS:
startup Windows9/Mac10, Desktop46/39, approval registry17/17 and original-stream
client tests3/3, all zero failures/ignored. Native lint/format/frontend passed.
Actual logs/inventories were checked; Windows standard-user identity/cleanup passed.
This is scoped execution evidence, not Mac product owner authorization or a UI proof.

The [Mac M8 owner-channel integration proposal](proposals/m8-macos-owner-channel-integration-decision.md)
is concrete but NOT accepted. It recommends a bounded, initially disabled native
adapter for M8 pairing/owner request families while retaining ordinary Unix IPC.
Separate owner/ADR direction is required before implementation; original experiment
approval does not authorize product integration. Mac mutation gates stay closed.
No new owner device run or old package repeat is requested. M8 remains open.

## Personal-Mac signed bootstrap passed19/19; no device action (2026-11-10)

MAC-MESAJ-SONUC.zip matches source8e8fabb/native C/header hashes on Mac27.2/SDK27.2
arm64. Strict parser and completion recomputation confirm certified self/bundle
eligibility, two signature refusals plus17 mechanism rows, four fault controls,
forced signal9 and independent cleanup; native/runner exit0. Existing-certificate
signing succeeded. [Exact receipt, observations and boundaries](../evidence/m8/m8-macos-signed-bootstrap-owner-receipt-20261110.md).

This exact isolated signed experiment is complete. No Mac/VM action or unchanged
package rerun is needed. Next: local design/source review of bundle snapshot/change
consistency and actual product connection/session lifetime under accepted contracts.
Keep the public-message candidate isolated. Product Unix transport replacement or
integration still needs separate explicit acceptance; do not enable Mac mutations.
Owner-session restart, Unix frame attribution, concurrent bundle replacement,
Mac publication/transport parity and final M8 reviews remain open. M8 is NOT complete.
This receipt supersedes the pending owner run below.

## Reviewed hostile bootstrap passed; signed Mac package ready (2026-11-10)

Exact8e8fabb CI42952782022 SUCCESS:17/17 ad-hoc mechanism rows, required fault
controls and independent normal/forced namespace cleanup. Certified two-binary
policy compiled and explicitly refused ad-hoc self; positive owner19 cases have
NOT run. Final source-only review has no actionable finding. Local suite63 total:
58passed/five native-Mac skips. [Exact evidence and limits](../evidence/m8/m8-macos-hostile-bootstrap-signed-candidate-20261110.md).

Next owner action: same personal Mac, extract
`C:\Users\Tarik\Downloads\cryptex-mac-mesaj-8e8fabbc.zip` -> **MAC-MESAJ.command**
-> return **MAC-MESAJ-SONUC.zip**. ZIP33437 bytes, SHA256
`ea4a5d7527fd1cd6ca45bfffa6e68ac193869ffb6c44f4b9b2372a8485448c20`.
All eight payload files match source8e8fabb and the source-bound manifest. If
Keychain asks, select Allow for this operation, not Always Allow. No new
certificate, VM action or old descriptor/pairing/API rerun is needed.

On receipt verify source/native/header hashes, eligible certified policy, all19
typed observations and fault/forced cleanup before claiming signed success.
Product Unix IPC/owner gates/minimum OS remain unchanged. Bundle replacement,
actual product pins/session restart and Unix frame attribution are unproven;
Mac publication/transport parity and final M8 review remain open. This section
supersedes older pending-review/no-owner-action instructions below.

## Hostile endpoint/right negatives passed; signed candidate in review (2026-11-10)

Exact f117a3b CI42950658480 SUCCESS:17/17 ad-hoc fixture rows, fault controls and
independent normal/forced namespace removal. Root spawns the actual victim;
separately authenticated helpers substitute an endpoint, consume its moved reply
right, or use a copied service right. Specific kernel-sender WRONG_PID52 refusals
and subsequent genuine sequence2 success are required. Helper control uses
CONFIG1/TRANSFER2/REJECT3, independent from daemon ACK1/STOP2. Source review passed.
An earlier success42950152593 actually used old heade826623 and provides no
phase-two evidence. Exact-head/source/hash checks were performed on the valid run.

Next candidate adds compile-time certified two-binary/self-bundle policy and a
source-bound MAC-MESAJ one-click owner collector. Source review and exact native
CI are pending before packaging. No owner action yet; no product activation,
support-floor selection or Mac owner-gate change is authorized.

## Reviewed public message-bootstrap phase one passed; no owner action (2026-11-10)

Exact source8816c04 CI42948812911 SUCCESS on macOS26.7.2 arm64. Public transient
check_in/lookup, kernel-sender actual Child/getppid validation and retained pins
work without a secret inherited FD or task-registration window. All13 native rows
pass, plus eight injected reaper paths, real timeout/invalid-header/cleanup fault
controls and independent namespace removal after SIGKILL. Source-only review
found/corrected reaping and failed-send cleanup defects; final delta has no further
actionable finding. Actual final CI JSON/source/hash/inventory were verified.
[Evidence, limits and corrections](../evidence/m8/m8-macos-public-bootstrap-mechanism-20261110.md).

Eleven new Python tests pass Mac/Windows. Final local Mac-script suite52 total:
47pass/five native-Mac skips. Existing scoped Mac regressions pass in the same CI.
The policy is explicitly an ad-hoc fixed-ID/same-image CI fixture, not certified
two-binary/product custody. Phase-one success leaves both original security
findings and M8 open; product Unix IPC/owner gates/minimum OS remain unchanged.

Next, within the already authorized isolated experiment: actively substituted
rendezvous/reply endpoint and transferred-send-right negatives, then certified
two-binary/self-bundle policy. Review before preparing any owner package. No new
scope approval is needed for that bounded research; product integration is still
unapproved. No Mac/VM action or repeat old pipe/pairing/API package is needed now.

## Owner authorized isolated message-bootstrap research; CI pending (2026-11-10)

The owner explicitly selected the bounded replacement design/isolated experiment.
No repeat decision is needed for that scope. Product integration remains unapproved.
[Phase-one design](proposals/m8-macos-message-bootstrap-experiment.md) and
scripts/m8/mac_bootstrap_mechanism.c/.py add a disposable CI-only public
check_in/lookup and actual Mach-sender fixture. It passes no synthetic descriptor
or token through inheritance and changes no Mach registration list. Both sides
bind kernel trailer identity to their actual Child/getppid and retain a task/
generation pin across frames. The CI ad-hoc same-image policy is deliberately not
certified product/bundle acceptance. Eight local report/failure tests pass;
native CI and source review are pending. No owner package is produced.

## Second owner receipt: 5/11; direct descriptor carrier stopped (2026-11-10)

New MAC-BAGLANTI-SONUC (2).zip matches source888d68a and the native hash.
All five public declaration probes, runtime API eligibility, signed bundle
eligibility and five control/exclusion cases pass. All six intended-child handoff
cases fail; runner_exit2 and candidate_complete=false are consistent. The row
result strings name expected outcomes, not observed child exits.
[Exact receipt and source diagnosis](../evidence/m8/m8-macos-launch-owner-receipt-2-20261110.md).

Published XNU excludes CLOFORK descriptors before spawn dup2 actions. That strongly
explains the common failure, but this receipt does not record the actual errno.
The apparent fileport spawn mapping workaround is declared in a private header;
do not copy it or clear the parent's flags. Stop this direct mapping candidate.
No unchanged package rerun, certificate adjustment or VM action is requested.

Next: owner decision on the [bounded carrier reassessment](proposals/m8-macos-launch-carrier-reassessment.md)
before moving to a public sender-attributed message-bootstrap experiment. This
keeps accepted Unix product IPC unchanged; no new carrier is implemented or
accepted. Mac owner mutation gates stay closed, minimum-OS policy unchanged,
both original security findings and M8 still open. Older pending-run instructions
below are historical and superseded by this section.

## Owner launch receipt incomplete; corrected measurement package ready (2026-11-10)

Personal-Mac MAC-BAGLANTI result from b234322 matches source/native hash on
Mac27.2/SDK27.2 arm64. Compilation/signing succeeded, but only the common deadline
control ran; aggregate API gating was false and Team metadata was not obtained.
Neither custody finding closed. The Team query used flag0 rather than the required
kSecCSSigningInformation: a harness defect, not a certificate-shortage diagnosis.
[Receipt, correction and exact next step](../evidence/m8/m8-macos-launch-owner-receipt-20261110.md).

Corrected source888d68a requests signing metadata after strict self validation and
uses per-declaration public compile probes instead of a version-macro conjunction.
No copied flags/private declarations/fallbacks. Exact CI42914376385 SUCCESS;
Mac40Pythonpass/one platform skip, local Windows36pass/five native-Mac skips.
Source review passed within this bounded scope. Positive owner-native cases remain
unverified; actual production IPC/owner gates and minimum-OS policy are unchanged.

Next owner action: same personal Mac, extract updated888d68ac package ->
MAC-BAGLANTI.command -> return the new MAC-BAGLANTI-SONUC.zip. Outgoing
`C:\Users\Tarik\Downloads\cryptex-mac-baglanti-888d68ac.zip`, SHA256
`c4db8cdff70444f45989324aff5c8032533523e84037e6c43c684492cd49a088`.
Do not rerun the old b234322 package or change certificates based on its incomplete
metadata query. A genuine failed public-declaration probe ends this candidate;
never force a numeric/non-atomic fallback. No VM/pairing/Mach repeat is needed.

## Two Mac findings have a reviewed candidate; one personal-Mac run pending (2026-11-10)

Source b234322 adds a product-separated atomic launch/signed-bundle candidate.
It requires public macOS27 pipe2+CLOEXEC+CLOFORK and explicit posix_spawn mapping;
no non-atomic fallback or pre_exec. The identity root is strictly verified live
self/fixed-ID/Apple-anchor/certified TeamID, then a verified same-organization
bundle/counterpart hash, instead of trusting the sibling to define its own policy.
[Mechanism, evidence and limits](../evidence/m8/m8-macos-atomic-launch-candidate-20261110.md).

CI42911736801 SUCCESS on macOS26.7.2/SDK26.5: C compile, explicit ad-hoc self refusal,
supervisor-vs-child-deadline distinction, Python38pass/one platform skip and prior
scoped regression checks. Atomic headers/runtime unavailable there, so positive
candidate cases NOT RUN and candidate_complete=false. A native Mac27 run is needed.
Source-only review corrected false-pass/error-reporting issues; production is untouched.

Next owner action: own Mac extract -> MAC-BAGLANTI.command -> return
MAC-BAGLANTI-SONUC.zip. Outgoing
`C:\Users\Tarik\Downloads\cryptex-mac-baglanti-b2343229.zip`, SHA256
`1f0793e1ddefc137b6baced7e1dbb573d4636d87b4474f95b6f44b1668d1df01`.
Inspect exact source/native hash and all eleven observations before a positive claim.
No VM, repeat API/Mach/pairing run or new certificate is requested.

Even a positive result does not accept a macOS27 product minimum or enable Mac
owner operations. Bidirectional production pinning, persistent lifetime, concurrent
bundle replacement, Unix forwarded-FD/frame attribution and owner-session restart
remain unproven. Current task right lasts one validation. This is not release/
publisher/notarization acceptance or M8 closure. Keep the actual product gates closed.

## Real Desktop readiness corrected; Mac custody candidate refused review (2026-11-10)

Source4229585 connects the real Desktop startup/restart path to a shared helper
requiring a complete ready PID matching its owned Child. Missing readiness no
longer falls back to success; partial files wait, malformed/oversized/nonregular
signals fail, and failed children are killed/reaped. Initial local Windows eight tests passed, including actual daemon crash/replacement
and fresh public health. Refined ordinary suite passed8/0failed/1ignored; its
endpoint fixture now runs only on an explicit disposable account. Daemon all-target
clippy/fmt and script parsing passed. Latest fixture/CI source867c893 uses a real
standard-user Windows token; native CI42906355843 is SUCCESS: startup Windows9/0failed/0ignored, Mac10/0failed/0ignored;
Desktop debug Windows46/0failed and Mac39/0failed. Both platforms passed
daemon/Desktop all-target clippy, formatting and frontend build. The actual
Windows account/profile cleanup reported success.
[Change, review findings and exact continuation](../evidence/m8/m8-desktop-startup-review-20261110.md).

The new Mac token-pipe integration draft was withdrawn before activation:
non-atomic CLOEXEC creation could leak an end during concurrent spawning, and
the sibling-derived signature/hash was not a trusted Cryptex release anchor.
LOCAL_PEERTOKEN is a socket-owner sample, not exact per-frame sender attribution.
Mac owner mutation gates and accepted Unix-socket transport remain unchanged.
This readiness work does not close parent custody, signed/native Tauri acceptance,
owner-session restart semantics or M8.

Next: review a trusted packaged-image anchor and inheritance-safe custody under
the accepted adapter; resolve the proposed Mac protocol's launch ordering and
forwarded-FD boundary before activation. Then Mac profile/publication parity,
actual offline/restriction propagation, final security/platform/human acceptance.
Four work blocks remain, not a single last package. No owner VM/Mac action now;
do not repeat previously passed raw Mach/signing/API/Windows pairing tests.

## Personal-Mac raw sender-instance receipt passed8/8; production custody remains open (2026-11-10)

MAC-SUREC-SONUC.zip from the personal MacBook passed all eight synthetic cases
on source85ef4b1: existing-certificate signing/native compile, expected controls,
same-binary shared-right/forged-payload refusal, replay, same-PID exec generation
change, fresh explicit enrollment and queued-message refusal after sender exit.
macOS27.2/SDK27.2 arm64, ci_only=false, runner_exit0. Receipt SHA256:
`14df58e67aff07697ba9a52d5245b41c494eac34a5011c1437356b2f6e98fddf`.
Committed C source hash and exact case/scope consistency verified.
[Owner result and limits](../evidence/m8/m8-macos-message-instance-proof-20261110.md).

Do not call this production Desktop custody: the receiver pins its spawned worker,
not the actual Desktop-parent/daemon-child launch. XPC exact-message binding,
integration, receiver-restart owner-session semantics, actual PID reuse and minimum
product OS remain unproven; production_changed=false and M8 OPEN.

Next: review/test actual parent custody at the production adapter boundary under
accepted ADR-0007. Evaluate the existing Unix-socket audit-token route using the
now-confirmed public generation getter before recommending a new transport.
No Mach production channel or ADR amendment was selected/accepted by this receipt.
Any transport change needs a concrete reviewed proposal and explicit owner
acceptance. No new owner action is requested now; do not repeat the passed package.

## Raw Mach actual-sender experiment passed CI; personal-Mac run pending (2026-11-10)

Public SDK audit confirmed audit_token_to_pidversion is declared in public BSM
headers, while XPC audit getters are not. A separate RAW MACH experiment uses the
kernel's actual-message audit trailer, public PID/generation getters, joint kernel
code validation and a retained live task-name right. Mac CI42898425263 on85ef4b1
passed all eight cases, including same-binary/shared-send-right intrusion, forged
payload, replay, same-PID exec generation change and a queued message after exit.
Source-only independent review found no actionable issue in this bounded scope.

This does NOT solve XPC binding or change production Unix-socket IPC. The test
receiver pins its spawned worker; actual Desktop-parent custody, integration,
receiver-restart session semantics, actual PID-reuse run and minimum product OS
remain unverified. M8 stays OPEN. Any Mach production channel requires separate
ADR-0007 acceptance. [Mechanism, CI and limits](../evidence/m8/m8-macos-message-instance-proof-20261110.md).

Next owner action: personal MacBook extract -> MAC-SUREC.command -> return
MAC-SUREC-SONUC.zip. One existing-key signing prompt may require allow-once.
Outgoing `C:\Users\Tarik\Downloads\cryptex-mac-surec-85ef4b13.zip`, SHA256
`edafdd90908ef0820f2f69ebe8f7f2b0349b0eff969f0cd273dbf8db1fa49af6`.
No repeated Windows VM/pairing/API preflight. Do not claim owner-native success
until its receipt is checked; product activation remains disabled.

## Owner signed Mac observations received; exact-instance gate remains open (2026-11-10)

MAC-IMZA-SONUC.zip from the personal MacBook reports successful existing-key
fixture signing/native compilation and runner_exit0 on sourceee5d417. Positive
nonce controls passed; wrong-ID/ad-hoc senders were explicitly rejected by
message-derived SecCode validation. Their guarded connection errors remain
inconclusive; signature-removed executables exited-9 without authorization proof.
The same-requirement second process successfully used the forwarded endpoint
after origin exit: signer-only binding is insufficient in that synthetic topology.
This does not test a parent-launch custody mechanism or show all XPC designs
infeasible. Full feasibility remains INCOMPLETE, exact-instance binding NOT PROVEN.

Receipt SHA256:
`21d85852f84d59873137c08ec61ca47aabd35b9730dfe061a55c84237f8a5510`;
native source hash and case consistency checked. Launchd registration removal
reported; server exit not independently observed. No production/trust change;
M8 OPEN. [Detailed owner observations](../evidence/m8/m8-macos-signed-xpc-observations-20261109.md).

Next: continue the existing bounded source/native feasibility investigation of
exact parent-launch connection custody, stale/cross-connection and restart
refusal. Do not migrate production IPC, accept ADR changes or substitute PID/
signer-only identity. Second certified signer, actual stale-connection reuse and
product minimum OS remain unverified. No new owner package is requested now;
do not repeat successful signing/API/Windows pairing/transfer tests.

## Signed synthetic XPC package ready; native harness CI passed (2026-11-09)

MAC-IMZA.command uses the personal Mac's single existing certificate for temporary
nonce-echo fixtures, with exact certificate/identifier requirements and delivered
message-derived SecCode checks. Wrong-ID/ad-hoc/unsigned observations and a
same-requirement forwarded endpoint after origin exit are scoped separately.
Full feasibility stays INCOMPLETE: second certified signer, stale actual connection
reuse, product minimum OS and exact-instance custody remain unproven. No production
IPC/trust/transport changes. Mac CI42701476626 passed sourceee5d417; real CI ad-hoc
echo/message-refusal/forwarded-endpoint observations and job registration removal
worked. Connection-layer wrong-ID error stays inconclusive; signer-only forwarded
endpoint acceptance is an observation, not an exact-instance solution. Owner run
is NOT RUN. Next: personal MacBook extract -> MAC-IMZA.command -> return
MAC-IMZA-SONUC.zip. Outgoing file:
`C:\Users\Tarik\Downloads\cryptex-mac-imza-ee5d4176.zip`, SHA256
`b659127a06f62ca44aebba766d5d673bfb7a2691ea33d26517a75cb75874ce4d`.
The user's existing key may request allow-once access; no key export/new certificate.
[Scope and safety](../evidence/m8/m8-macos-signed-xpc-observations-20261109.md).

## Personal Mac API preflight passed; one signing candidate found (2026-11-09)

The latest MAC-KIMLIK-SONUC.zip is from the user's own MacBook, collected at
2026-11-09T21:04:04Z on source59411433. API compile/presence passed on
macOS27.2/build26B123 arm64, SDK27.2, runner_exit0. It enumerates one valid
file-Keychain entry, one distinct certificate and one observed Apple team label.
Receipt SHA256:
`e9e620e8f3749b883152f4d4ab334338c6df724a44c12a797ee2b0b51c9d706f`.
Next: select the existing identity locally for the bounded synthetic signed XPC
probe. The receipt intentionally contains no certificate selector or Team ID;
one team-label count does not establish a second signer for the negative matrix.
No signing/private-key use has been tested. Full signed-matrix prerequisites
remain unproven, exact-instance binding NOT TESTED, product minimum OS unconfirmed.
Do not request another API preflight or repeat completed Windows VM tests.
Details: [prerequisite evidence](../evidence/m8/m8-macos-identity-preflight-20261109.md).

## Earlier borrowed-Mac receipt; distinct environment (2026-11-09)

The submitted MAC-KIMLIK-SONUC.zip passed API compile/runtime presence checks on
macOS27.2/build26B123 arm64, SDK27.2, source59411433, runner_exit0. It reports zero
valid file-Keychain identities; data-protection Keychain was not enumerated.
The user clarified this was an available Mac, not their personal Mac. Its signing
inventory describes only that machine/account. Next establish access to a suitable
Mac and an existing signing identity; access to the tested Mac/Xcode team is still
unconfirmed. Do not repeat the same successful API check or Windows VM pairing.

New scripts/m8/MAC-KIMLIK.command collects SDK/API availability
and existing valid file-Keychain identity counts, exporting only sanitized counts
and fixed diagnostics. It never signs, imports/generates credentials, starts a
service or changes production IPC/trust. Full signed XPC matrix and product minimum
OS proof remain NOT RUN/unconfirmed. A zero file-Keychain count does not rule out
data-protection Keychain identities.

Source review corrected owned-group cleanup/late-reader races. Local8checks pass;
Mac CI42687722399 on59411433 passed11 applicable tests plus the actual one-click
API compile/presence flow (one non-Mac-only check skipped). No signed XPC matrix.
Outgoing `C:\Users\Tarik\Downloads\cryptex-mac-kimlik-59411433.zip` has the same
SHA256 as the CI-executed package:
`7d80c39594a7e111dbd8fb27239a1be25f6c83089e704c88d08a1907f6f08730`.
Submitted receipt SHA256:
`516f4b6c9871af6824e72fb58a6d3aa7b38cb9d3124a410c7b9c6affd97db9f1`.
This is prerequisite evidence only. Full signed matrix remains NOT RUN; do not
generate/import credentials or change production IPC to bypass the missing
prerequisite. Details and scoped limits:
[prerequisite evidence](../evidence/m8/m8-macos-identity-preflight-20261109.md).
Retain the green product-code CI below; no product Rust/frontend behavior changed.

## Current resume point: exact-code dual-OS CI green; M8 OPEN (2026-11-09)

**Tested/pushed behavior source:** `6de8dbea8b6cf0221cd114ed91c77290ce1a6165`,
branch `codex/m8-device-trust`.
[CI42564404285](https://github.com/gungortarik/cryptex/actions/runs/42564404285)
completed SUCCESS: Windows readiness, macOS readiness and Windows different-user
IPC ACL all passed. Unsigned installer job was skipped by workflow design.
Windows standard-user daemon library596passed/0failed/5ignored; binary16/16.
Mac library464passed/0failed/4ignored; binary15/15. Release Desktop tests47/47
Windows and40/40 Mac; full workflow format/lint/build/schema/Node steps passed.
Ignored explicit native fixtures remain outside ordinary CI; no new filter/ignore.
The CI launcher verified complete lib/bin inventories, actual non-admin SID and
private TEMP, and completed account/profile cleanup without failure.

The long-path persistence fixes, local offline/cutoff integration and independent
technical source review are complete for this candidate. README and
[M8 completion ledger](../checklists/M8_DEVICE_TRUST_REPLICATION_GATE.md) now map
the actual remaining gates. This section supersedes pending-CI continuation
statements in the historical retry entries below. Detailed logs/counts/limits:
[CI evidence](../evidence/m8/m8-ci-offline-review-20261108.md).

**Next:** consolidated Mac stage. The owner-authorized synthetic XPC peer/instance
feasibility probe is still NOT RUN. Complete that bounded proof/review before any
ADR-0007 production IPC amendment. Mac managed profile/worker, publication ancestor
pins/atomicity and native authenticated owner actions remain incomplete. Normal
startup feeding/production peer policy stay disabled until protocol/security
acceptance. Then verify actual cross-device offline/restriction propagation and
final security/product/human review. M8 is not closed and M9 has not started.

Do not repeat completed Windows pairing/native transfer/replay packages. VM can
remain closed; request one consolidated new device batch only for a concrete
candidate. Later documentation-only status commits do not change the tested code.

## Full writer path corrected after final hosted failure (2026-11-08)

CI42563317369 on18774b2 completed: Mac readiness + Windows IPC ACL passed;
Windows library595passed/1failed/5ignored. Remaining policy-fixture failure was
canonical writer tip placement at a long path, misleadingly reported OS0 because
cleanup ran before reading the original Win32 error.

A >260 Unicode-path full writer regression reproduced that failure, then exposed
the same unencoded path at sequence persistence. The shared Windows encoding now
lives in storage/windows_path and is reexported by the daemon platform adapter.
Writer tip, sequence initialization/counter, local/install identity and segment
seal callers use it, preserving each exclusive/replace flag. Original tip error
is captured before cleanup. Strict staging/quarantine/capture adapters untouched.

Local: fullwriter29, sequence16, localidentity11, install10 and storage44 pass;
7 daemon long-path regressions pass, strict Clippy/fmt/diff pass. Independent
source-only review found no actionable issue. New full CI is required. Mac product
gaps/next authorized bounded probe are listed in the M8 completion ledger.

## Final metadata caller audit includes Brain identity and journal (2026-11-08)

The same raw Win32 long-path defect also affected exclusive Brain identity create
and bootstrap journal replacement. Two additional >260-character Unicode-path
regressions reproduced OS3 before applying the already reviewed metadata encoder.
Identity11/11 and journal15/15 now pass; no-replace/create-winner and full journal
update/reopen semantics are preserved. Strict daemon Clippy/fmt/diff pass.
Run42563020408 onfcfb65d was superseded before completion; next CI must cover
these final callers too. Normal activation and M8 completion remain gated.

## Hosted Windows suite reached; metadata long-path correction (2026-11-08)

CI42561855133 on802f95d completed: Mac readiness and Windows cross-account ACL
passed. Standard-user Windows daemon library ran 587 passed/3 failed/5 ignored;
all three failures were public signing metadata placement returning Win32 OS3 at
long runtime paths. Full Windows readiness did not pass.

Two new >260-character Unicode-path regressions reproduced OS3 before correction.
Metadata MoveFileExW now receives extended UTF-16 drive/UNC paths through a small
Windows adapter, with no filesystem canonicalization. Existing verbatim semantics,
exclusive signing metadata placement, mutable identity replacement/write-through,
persisted seed retry and staging ownership/reparse/volume checks are preserved.
Local key14/14, metadata5/5, encoder2/2, previously failing policy test1/1,
strict daemon all-target Clippy/fmt/diff pass. New exact-candidate CI is required.
See [CI evidence](../evidence/m8/m8-ci-offline-review-20261108.md).

## CI credential-profile lookup bound to actual token (2026-11-08)

Run42560988740 passed module loading, then the CI-only environment-based profile
lookup failed its owner check before tests. Child now queries GetUserProfileDirectoryW
with its real SID/non-admin-verified WindowsIdentity token, validates the fresh
account profile name and replaces inherited profile/appdata values. Strict owner
checks on newly created private TEMP and production canonical paths are unchanged.
Local read-only native API/parser checks and independent source review pass;
next hosted CI run must establish complete daemon execution/cleanup.

## Standard-user CI child module environment corrected (2026-11-08)

Run42560323415 on92d3f5b successfully launched the credential child through
SID/non-admin/profile-existence checks, then WindowsPS5.1 Get-Acl autoload failed
because it inherited PS7 module search paths. Child now uses only its native
PSHOME\Modules before autoload. Local native5.1 Get-Acl check passes. No production
policy changed; next CI run must prove complete daemon execution and cleanup.
Mac same-run daemon lib/bin passed and progressed to release Desktop tests;
Windows different-user ACL job passed. Full CI remains unverified.

## CI retry: Mac library green; fixture allocation/Windows token corrected (2026-11-08)

CI42559236900 on34f5866: Mac library 464 passed/0failed/4ignored; binary test
failed timestamp-root collision. Test-only atomic exclusive allocation now passes
16/16 binary tests at16threads, with32-thread uniqueness regression. Windows
library512passed/78failed/5ignored, dominated by strict ownership refusal because
hosted runner is elevated. Production owner checks remain unchanged.

Windows CI now prebuilds exact daemon lib/bin artifacts, executes complete test
inventories under a verified fresh standard-user process/private TEMP/profile,
checks final summaries/exit/identity proof, and removes recorded account/profile.
Source/binaries are read/execute only for that user; scratch is writable.
Doctests remain separately checked. Source review and local guard/parser/summary
checks pass; actual CI execution of the new launcher is the next validation.
Full detail in [CI evidence](../evidence/m8/m8-ci-offline-review-20261108.md).

## Exact-candidate CI exposed two test scheduling assumptions (2026-11-08)

CI run 42558672783 on 8cfcd50: Windows different-user IPC ACL passed; Mac daemon
tests reported 462 passed/2 failed/4 ignored. Failures are a sleep-based worker
reservation assertion and an HTTP fixture waiting for an optional second request
after the client deadline. Corrected with a release channel and deterministic
test-only inter-request deadline seam; production search/owned REST unchanged.
Local targeted IPC +19 REST tests and strict Clippy/fmt/diff pass. Independent
source review found no blocking finding in these changes or the long-path/offline
candidate; its request-counter limitation is explicitly recorded. New CI required.
See [CI/source-review evidence](../evidence/m8/m8-ci-offline-review-20261108.md).

## Local offline convergence and restriction integration added (2026-11-08)

Two new persistent daemon tests cover independent offline writes from both peers,
canonical/search convergence under reversed incoming hint order and idempotent
replay; and owner-confirmed retired/compromised cutoff 3 refusing a disconnected
target's signed seq 4 while retaining authorized history. Focused checks pass.
These are signed fixtures plus real daemon publication, not native networking or
native Desktop approval. No normal feeder/activation or policy change.

The successful Windows native pilot below remains complete; do not repeat it.
Remaining acceptance work is mapped in
[M8 completion ledger](../checklists/M8_DEVICE_TRUST_REPLICATION_GATE.md), with
[local integration evidence](../evidence/m8/m8-offline-restriction-integration-20261108.md).
Immediate next step: exact-candidate Windows/macOS/IPC ACL CI, then address the
Mac product-platform/protocol boundary and real offline/restriction propagation.
M8 remains OPEN; human review and protocol activation gates still apply.

## Actual native transfer/publication/restart/replay pilot passed (2026-11-08)

Owner returned `DENETLEYICI-SONUC-45278d8e-a4a65ab5fda6.zip`. Exact six
members, source/archive binding, build/test/runner exit codes, log summary and
native/host receipts agree for candidate `c669a9a4f653fdfd759ef4c9250c834d664d26d0`.
The explicit Windows native test passed 1/1, zero failed/ignored (592 filtered),
108.15 seconds; Windows 10.0.26200.0, NTFS.

Actual owned REST readiness and cross-host Syncthing delivery, acknowledged native
job crash before publication, restart with unchanged ID, worker publication of
one segment/three events, real named-pipe busy/search behavior, unchanged existing
canonical hashes, clean native reopen, identical replay with no new segment,
clean shutdown and synthetic API credential deletion all passed. Host receipt
independently confirms 6557 bytes/100% transfer and clean relay shutdown.

This requested pilot round is complete. Do not repeat the package, request ZIPs,
pairing or manual signed-message exchange. Owner can close the runner/VM for now;
no test endpoint needs to remain live. Detailed receipt bindings and limits:
[verified native pilot result](../evidence/m8/m8-native-controller-result-20261108.md).

M8 remains open: test-only peer/folder policy and synthetic canonical identities
are still used, IPC peer authentication uses the existing test seam. This does
not enable normal startup transport feeding or prove native Desktop authorization,
Mac production publication/endpoint parity, physical power-loss durability,
remaining cross-device offline/restriction integration, required CI or human
review. Continue engineering from these explicit remaining gates, preserving
the successful native receipt; do not confuse them with a failed transfer pilot.

## Long-path candidate delivered to VM; receiver result pending (2026-11-08)

Request `DENETLEYICI-ISTEK-45278d8e-a4a65ab5fda6.zip` passed closed validation.
Fresh host relay connected to VM; 6557 bytes, 100% completion, zero needed
bytes/items observed. Host script exited 0 with peer_completion_observed and clean
shutdown after completion keepalive. Sender is no longer running.

Next dependency: owner returns `DENETLEYICI-SONUC-45278d8e-a4a65ab5fda6.zip`.
Verify c669a9a candidate/source digest and all native publication/restart/replay/
IPC/credential-cleanup assertions before claiming the long-path live flow passed.
Do not rerun pairing or send another request. Host receipt is transport evidence,
not canonical import/M8 completion. See
[host transfer evidence](../evidence/m8/m8-native-controller-long-path-host-transfer-20261108.md).

## Long Windows import paths reproduced and corrected (2026-11-08)

Diagnostic `DENETLEYICI-TANI-5b92b54889a6-1729dd82.zip` confirms the received
6557-byte segment hash matches the source/request; receiver canonical peer
directory contains only writer.lock, not the imported segment. Native log confirms
transfer and restart. It does not prove clean VM shutdown.

Reproduced publication failure locally by setting TEMP to the VM runner's long
app-data directory shape: old code returns Rejected(Io(os error 3)) before the
post-commit pause. Raw Win32 path APIs imposed MAX_PATH on generated staging paths.
Windows staging now uses checked extended local-drive paths at volume inspection,
no-replace move and directory flush. Separators normalized, no canonicalization,
no altered share/reparse/owner checks or replacement flags. Long-path publication
and busy named-pipe IPC now pass; import staging 65/65, incoming publication 14/14,
strict Clippy/fmt/diff checks pass. This fixes a reproduced Windows defect; a new
native VM receipt is still required to establish the whole flow.

Prepare/deliver corrected native package using fresh host relay session
`45278d8e00394a62b8d3607208858878` under the usual private relay root.
Fresh native sender is generated but not launched. Never reuse the old request:
its host folder/payload are already consumed and receiver test ended.
Ready package `C:\Users\Tarik\Downloads\cryptex-denetleyici-aktarim-c669a9a.zip`
binds source `c669a9a4f653fdfd759ef4c9250c834d664d26d0`. Next: owner runs
NATIVE-AKTAR.cmd in VM, returns DENETLEYICI-ISTEK-45278d8e-*.zip while keeping
the VM runner open. Use the fresh host session above for relay validation/start.
See [long-path evidence](../evidence/m8/m8-native-controller-long-path-20261108.md).

## VM confirms delivery/restart; publication pause timed out (2026-11-08)

Returned `DENETLEYICI-SONUC-d1e5c535-5b92b54889a6.zip` matches candidate
`393d0c4` and its source digest. Build exit 0; native test exit 101, 0 passed,
1 failed, 149.93 seconds. Checkpoints confirm actual delivery before publication
and acknowledged owned-job crash/restart with unchanged identity. The test then
times out waiting for the post-commit/pre-rebuild pause signal (line 403).
No publication/replay/clean-shutdown pass receipt exists. Drop reports unresolved
worker shutdown; do not assume VM native cleanup completed from host shutdown.

Cause is not established. A new shared helper reproducing that exact publication,
busy IPC and post-publication search sequence passes locally with explicitly
synthetic transport readiness (1/1). The helper now reports early poll completion
or failure rather than hiding it behind the pause timeout. This is diagnostic
test-only work; it does not fix or establish native behavior.

Next owner dependency: run the small read-only diagnostic package for existing
VM run `5b92b54889a6`, return `DENETLEYICI-TANI-5b92b54889a6-*.zip`.
It inventories only that retained synthetic run and exports bounded native logs,
not keys/config/event bodies; does not restart the test or alter processes.
Package ready: `C:\Users\Tarik\Downloads\cryptex-denetleyici-tani-6c88bb8.zip`;
owner extracts in VM and opens `TANI.cmd`.
Do not request another pairing/live retry before inspecting those records.
See [VM failure evidence](../evidence/m8/m8-native-controller-publication-timeout-20261108.md).

## Actual host-to-VM relay completed; receiver receipt pending (2026-11-08)

Request `DENETLEYICI-ISTEK-d1e5c535-5b92b54889a6.zip` passed validation.
Started the retained isolated send-only native relay. Actual VM connection,
100% completion, 6557 global bytes and zero needed bytes/items observed. Host
script exited 0 after its 120-second keepalive with `peer_completion_observed`
and clean shutdown. No sender remains running for this session.

Next owner dependency: return `DENETLEYICI-SONUC-d1e5c535-5b92b54889a6.zip`.
Check candidate `393d0c4`/archive digest, exact test exit/summary and native JSON
checkpoint assertions before claiming controller publication/restart/replay pass.
Do not repeat pairing or send another request. Host transfer alone does not
establish canonical import or M8 completion. See
[host transfer evidence](../evidence/m8/m8-native-controller-host-transfer-20261108.md).

## Native request reached VM readiness; digest casing corrected (2026-11-08)

Owner returned `DENETLEYICI-ISTEK-d1e5c535-524ee335fd33.zip`: one public
6,557-byte signed fixture segment plus its manifest. The old host validator
rejected the existing probe's uppercase SHA-256 (`segment_manifest_invalid`).
The native receiver helper also used lowercase digest text for an exact comparison,
so the waiting VM candidate cannot complete even if host validation alone is fixed.
No host sender was launched. Do not send this request or call it a transfer pass.

Fixed test digest representation to match actual `ProbeSegment`, and host validation
accepts either valid 64-hex representation while still comparing the full digest.
Real fixture digest/change regression plus three inventory regressions pass;
runner checks cover both cases and bad digest rejection. Actual returned request
now passes `-ValidateOnly`. Strict Clippy, fmt and diff checks pass.

Replacement one-click package must use `--previous-run 524ee335fd33` to request
bounded cooperative cancellation of only that waiting synthetic run before a fresh
VM test. Helper requires matching session/checkpoint and observes runner completion;
it never kills arbitrary processes or modifies canonical data. Same untouched host
relay session `d1e5c535ae0e459fb699be8f7324c149`. Await the replacement request ZIP.
Ready package: `C:\Users\Tarik\Downloads\cryptex-denetleyici-aktarim-393d0c4.zip`,
source `393d0c4dcdfc36ddd72c0d6386d067ada6634d0c`; eleven members/hash bindings
verified. Owner opens `NATIVE-AKTAR.cmd` from a fresh extracted VM directory.
See [digest correction evidence](../evidence/m8/m8-native-controller-digest-fix-20261108.md).

## Native pilot VM receipt: inventory helper corrected (2026-11-08)

Returned `DENETLEYICI-SONUC-d1e5c535-4975885cbff4.zip` matches the
`e1b2aed` source/archive binding. Build passed, but the single native test
failed before worker startup/request export: its canonical hash helper tried
to enumerate the writer's root `inventory.lock` as a device directory
(Windows error 267). This is a pilot helper defect, not a transfer pass.

The helper now excludes only the regular `inventory.lock` coordination file;
unexpected root files/directories in that slot still fail. A real signed fixture
regression reproduced the same failure before the fix and passes afterward,
including event-byte change/addition/deletion detection. Three focused tests,
strict all-target daemon Clippy, formatting and diff checks pass.
No production code or transport security checks changed.

Corrected candidate `8789ca2bb5a167ca018b5be0919397ef4e172456` is packaged as
`C:\Users\Tarik\Downloads\cryptex-denetleyici-aktarim-8789ca2.zip`.
Next: owner runs the corrected one-click package; collect its request ZIP.
The same untouched host relay session can be used: the failed VM run never
exported a request and no host transfer began. Native integration remains pending.
See [failure evidence](../evidence/m8/m8-native-controller-inventory-fix-20261108.md).

## Opt-in real native worker transfer pilot prepared (2026-11-08)

Candidate `e1b2aedcf5f94461f15e02ba49e2662bd77d62ab`; verified package
`C:\Users\Tarik\Downloads\cryptex-denetleyici-aktarim-20261108.zip` (14,282,177
bytes). Final independent source review has no remaining concrete finding.
Exact source/package hashes and retained host relay session are in the evidence
linked below. Owner execution/request ZIP is the next dependency.

The owner selected **main Windows computer ↔ existing Windows VM**. The new
one-click `NATIVE-AKTAR.cmd` package builds the exact source candidate on the VM,
creates fresh synthetic canonical identities with signed reciprocal approvals,
and prepares an actual managed native receiver. Its public request ZIP contains
only signed public test segments and public transport metadata. The host relays
those bytes using a fresh isolated send-only Syncthing profile. Existing owner
profiles/pairings are not used. Keep the VM runner open while returning the
request ZIP; the native delivery deadline is 30 minutes after build/readiness.

The ignored native harness uses actual owned REST readiness, receives through
Syncthing into `transport/incoming`, explicitly terminates its owned native job
**after delivery and before publication**, observes the acknowledged restart,
then invokes `ManagedTransportWorker::process_incoming`. It checks shared
admission and real named-pipe search during publication/rebuild, verifies imported
bytes and pre-existing canonical hashes, cleanly reopens native/runtime ownership,
and replays with zero new segments and identical canonical data. IPC uses the
existing test authorization seam, not an independent native Desktop auth proof.

The exact single-peer/receive-only-folder exception and crash trigger are
`cfg(test)` only. Normal native reopen still rejects this configuration. No
normal feeder, new product transport authority or security bypass was added.
Synthetic event keys stay in the test process; only the native API credential
uses a fresh Windows DEV credential account. Successful receipt requires its
deletion; failure cleanup logs a redacted diagnostic. Private test directories
are retained, not included in result ZIPs.

Local source/runner checks passed; actual native end-to-end receipt is **pending**.
Host preflight stopped with `Rest(Endpoint(PeerNotEstablished))`, consistent with
the previously unresolved development-host TCP state problem. It is not a native
pilot pass. Windows VM previously passed the owned HTTP checks; use it as receiver.
This pilot does not claim interruption mid-network-write, power-loss durability,
macOS production parity, automatic activation or M8 completion.

See [pilot evidence and exact resume instructions](../evidence/m8/m8-native-controller-pilot-20261108.md).

## Exact-candidate native coordination checks passed on Windows and Mac (2026-11-08)

Owner returned `MAC-TEST-SONUC.zip` and `WIN-TEST-SONUC (2).zip` for candidate
`5708cb7bad7f3bc35d43df2568414de2267a18a4`. Both reports bind the expected
source archive digest. Exact ZIP members, individual test outcomes, aggregate
summaries, build logs and exit statuses agree: Windows **106/106**, Mac
**85/85** plus strict all-target daemon Clippy, zero failed or ignored entries
in these selections. These are selected native tests, not complete suites.

Windows VM 25H2/build 26200.9711 reports NTFS for work/test-temp volumes;
all eight owned-HTTP tests pass, TCP 60 samples/zero mismatches/state five,
parser self-test passes. Mac arm64 27.2/build 26B123 reports APFS. Shared
admission tests pass on both OSes. Windows controller tests use synthetic
transport readiness; they do not prove real native transport-to-publication.

This native test round is complete for this candidate. Do not ask the owner
to rerun these packages or repeat pairing. No VM/Mac needs to remain running
for receipt review. Evidence and hashes:
[native coordination results](../evidence/m8/m8-native-coordination-results-20261108.md).

Next engineering checkpoint: isolate actual owned-transport readiness and
incoming delivery through the dormant Windows worker bridge, with real IPC
readers and replay/restart/failure checks. It must stay an explicit synthetic
pilot until activation requirements pass; do not add a normal startup feeder.
Mac production ancestor/no-replace/endpoint parity, physical durability,
remaining cross-device offline/restriction integration, development-host TCP
compatibility, required CI and human review remain open. M8 is not complete.

## Exact-candidate native coordination packages ready; receipts pending (2026-11-08)

Candidate `5708cb7bad7f3bc35d43df2568414de2267a18a4` retains the reviewed
daemon implementation and updates only the native check runners/instructions.
One-click Windows and Mac source packages are ready in the owner's Downloads.
Windows selects **106 tests**, including the four dormant-controller cases,
five admission cases, fourteen publication cases and sixty-six IPC cases.
Mac selects **85 tests**, including the five shared admission cases, followed
by strict daemon Clippy. Mac does not execute Windows-only publication/controller
code. Both report filesystem type without claiming power-loss durability.

Runner helper regressions passed locally (Windows PowerShell 5.1 and Git Bash);
these are **not native package receipts**. Source archive/member integrity and
Mac executable ZIP mode checks passed. Exact Windows filter counts were checked
against the candidate test inventory. No new daemon behavior or activation.

Next: owner extracts the Windows ZIP in the existing VM and double-clicks
`WIN-TEST.cmd`; on the Mac, extracts the Mac ZIP and opens `MAC-TEST.command`.
Collect `WIN-TEST-SONUC.zip` and `MAC-TEST-SONUC.zip`, including failures.
Do not repeat pairing or modify existing profiles. Detailed bindings/checks:
[native coordination packages](../evidence/m8/m8-native-coordination-packages-20261108.md).
Native results, real cross-device controller feeding, Mac production parity,
physical durability, CI and human review remain open. M8 is not complete.

## Coordination source review completed; dormant worker bridge connected (2026-11-08)

Implementation `67c3f8bca2e32e73e25958bde517285a7cb94bc2`, final code/test
revision `141ff83cdbc0aee8215c742b42f278decd31ccbc`. The separate reviewer
completed the source review, found a P2 coverage defect in two already-expired
lease-probe deadlines, and closed it after both probes changed to future 40 ms
deadlines. Its final byte-bound source verdict reports no remaining confirmed
correctness findings. No reviewer executable was retried or protection changed.
This is technical agent source review, not human certification or a native pass.

`ManagedTransportWorker::process_incoming` now connects a bounded internal
`IncomingController` to its own status reader and the existing instance-owned
publisher. It waits for readiness, verifies canonical local transport binding
under ordinary admission, releases that lease before exclusive publication,
defers busy work and retains held/ambiguous work. Clean rejection/binding failure
consumes only hints, preserving incoming bytes and preventing automatic costly
retries. No normal startup/watcher/CLI/renderer feeder is enabled.

Background derived refresh now rebuilds the complete verified cutoff-aware peer
union using the existing v1-content import rebuild. New local-identity incoming
tails are refused after equality replay pruning; local allocator/canonical bytes
stay unchanged. Existing local replays remain idempotent. Broader restore and
v2 retrieval semantics are not enabled.

Author checks: IPC **66/66**, controller bridge **4/4**, signed local-tail/replay
regression and peer-refresh check pass, binary **15/15**, strict all-target daemon
Clippy/fmt/diff checks pass. Final library run: **571 passed, 0 failed, 4 ignored,
8 explicitly filtered known owned-HTTP tests** (81.12 s). This is not a full-suite
pass or a host TCP compatibility fix. Exact scope and results are in
[controller/review evidence](../evidence/m8/m8-controller-bridge-20261108.md).
Transport observations in controller tests are explicitly synthetic, not native
owned-endpoint evidence. Earlier native receipts do not cover this source.

**Next boundary:** native exact-candidate transfer/publication and filesystem
durability gates before ordinary automatic feeding/activation. Mac production
parity, ordinary cross-device offline/restriction/recovery coverage, host TCP
compatibility, required CI and human review remain open. M8 is not complete.

## Runtime admission coordinates internal live imports and IPC (2026-11-08)

Owner requested live-transfer safe read/write coordination. Code commit
`a5429ea150748b64ce7ce25c618327671cdf62d2` binds one process-local admission
coordinator to each actual `InstanceLock`. Product IPC constructors require the
matching instance. Ordinary IPC/derived operations retain cloneable leases;
exclusive import blocks newcomers and drains existing work before staging.
The lease spans response serialization/write and timed-out worker completion,
then the import spans recovery through commit/cleanup/derived rebuild.
Canonical lock ordering and the staging reservation's short lifetime are kept.

Clean pre-prepare rejection reopens admission. Ambiguous recovery/publication
or committed-but-stale refresh latches a typed runtime hold. Internal verified
recovery plus cutoff-aware rebuild is required to reopen it. Panic/abandonment
fails closed; poisoned state is not reset. Prepared owner-review sessions
release admission during think-time and reacquire for follow-up. Health and
redacted transport status remain available. Shutdown now stops admission and
drains actual work (40-second bound) before claiming verified lock release.
Idle contexts use weak OS-handle references; active leases pin real ownership.

Windows author verification: **566 library tests passed, 0 failed, 4 ignored,
8 explicitly filtered known owned-HTTP tests**; binary **15/15**. Included groups:
runtime admission five, publication thirteen, IPC sixty-six. Actual pipe
backpressure, orphaned workers, owner think-time and real instance-shared
signed-import/IPC checks passed. Strict all-target daemon Clippy, fmt and diff
check pass. A narrow coordinator reader/shutdown metadata check for the Apple
target passed; this is not native Mac or whole-app verification. Earlier native
receipts do not cover this new source.

The separate reviewer performed a preliminary source pass; its executable was
blocked by Application Control (4551), and its later agent turn was rejected by
an automatic possible-cybersecurity-content filter. No protection was changed.
**Final independent review of this exact coordinator remains open.** See
[coordination evidence and detailed continuation](../evidence/m8/m8-runtime-admission-20261108.md).

**Next boundary:** finish that review before adding a managed-controller caller
to the internal publisher. Peer-aware long-lived derived refresh and local-tail
allocator scope must be addressed before broader activation. Native Mac parity,
physical durability, ordinary cross-device offline/restriction integration,
the host TCP compatibility case, required CI and human review remain open.
Normal transport publication stays disabled; M8 is not complete. No owner VM/Mac
action is needed for this code checkpoint.

## Separate technical review found two defects; corrections verified (2026-11-08)

The owner authorized a fresh-context, read-only reviewer agent. It reviewed clean
commit `2165539491733b40ff61908955823e0b3fafe514` in an isolated worktree and
reproduced two P2/medium findings using synthetic scratch sources/runtimes:

- A genuinely signed zero-cutoff restriction could name a known device ID with
  an unrelated valid key and restrict the actual device. Reproduced through the
  real internal publication/rebuild path, not only the reducer helper.
- A lower-level import policy proof from runtime A could publish in B without
  holding B's inventory/device locks. The current service replay check blocks
  this route, but the lower-level admission/final-publication invariant was
  incomplete.

Correction commit `f072ae234344bcee51b9bcb995fc4d1678aba45f` binds known restriction
targets to their exact pinned key ID/full public key for every cutoff and checks
the import snapshot's runtime at admission and again before publication. Unknown
third-target restriction metadata remains supported; it does not grant trust.
Added signed-key and two-runtime regressions; no canonical bytes are edited,
test assertions weakened, or external publication entry point enabled.

Author verification: reducer **14/14**, publication **11/11**, binary **15/15**,
strict all-target daemon Clippy and fmt pass. Broader daemon library run: **554 passed, 0 failed,
4 ignored, 8 explicitly filtered owned-HTTP tests**. Those eight are the known
development-host TCP compatibility group; this is not a full-suite pass or a
fix for that compatibility case. Earlier Windows/Mac receipts remain scoped to
their prior source revisions, not this correction.

The reviewer independently copied corrected sources to scratch, verified all
four files byte-identical to the correction commit's Git blobs and passed all
three new regression entries. No additional confirmed defect remains in the
bounded correction reviewed. This is a separate-agent technical
assessment, not independent human certification or human diff approval. See
[findings and integration continuation](../evidence/m8/m8-independent-review-findings-20261108.md).

**Next engineering gate:** implement/review the live daemon's runtime-wide
reader/failure barrier before managed-controller publication wiring. Existing
IPC/projection workers do not borrow the mutable instance guard. Recovery-required
or committed-but-stale errors must keep all guarded operations held until
verified recovery/rebuild. Preserve canonical lock ordering and the staging
reservation's short lifetime. The report records concrete acceptance checks.
Mac production parity, native physical durability, ordinary cross-device
offline/restriction integration, required CI and human review remain open.
Normal publication stays disabled; M8 is not complete. No owner VM/Mac action
is required for this review checkpoint; neither machine needs to stay running.

## Windows VM comparison passed; selected native checks recorded (2026-11-08)

Owner returned `WIN-TEST-SONUC.zip`, SHA-256
`469A07AE1593F58000DB801C2A5AC588C55A445550EDD6F9F1EA8BB56756C5E0`.
The exact seven bounded members match pinned source commit
`c55f3b70c763aef7541481e3897dba348d300c47` and archive digest
`7ef0213a8eb9045a0fb1112e3485e24a352957af4d59fceaf453e44f276fe493`.
On reported Windows 25H2/build 26200.9711 the harness built and all 26 selected
test entries passed: instance nine, persistent publication nine, owned-HTTP
eight. The TCP probe reports 60/60 before/after round trips and exact established
owners, zero mismatches, raw state five only; parser self-test passed. All five
check records, build exit, runner exit and actual log summaries agree.

The five development-host HTTP failures do not reproduce on this VM with the
same source. This supports an environment-specific TCP/endpoint issue, not a
diagnosed universal fix: host build 26300.9818 remains an unresolved compatibility
case. No endpoint check, test assertion, signing policy, firewall or antivirus
setting was weakened. Receipt reports no production profile access/publication.
VM volume filesystem type was not collected; do not infer physical durability.

Together with the earlier Mac 80-test native receipt and corrected strict Mac
Clippy result, the selected native-check round is complete. No repeat pairing,
transfer, Mac readiness, native groups or lint is required at these checkpoints.
No test sender or owner profile needs to stay running; the VM may be shut down.
See [actual Windows receipt](../evidence/m8/m8-windows-platform-checks-20261108.md#actual-windows-vm-result).

**Next engineering/review gate:** use the prepared
[independent review packet](../evidence/m8/m8-independent-review-packet-20261108.md).
It records code/evidence scope and activation blockers; it is not an independent
review verdict. Passing these tests does not enable normal publication; no new
device action is needed for this checkpoint. The live-controller reader/failure barrier,
Mac production ancestor-handle/no-replace/endpoint parity, native physical
durability and independent security review remain open. Normal publication
stays disabled and M8 remains incomplete.

## Mac compile/strict Clippy passed; Windows VM checks prepared (2026-11-08)

Owner returned `MAC-LINT-SONUC.zip`, SHA-256
`EF77822C3E9A14CCAB93B5F7C8BF1A5AC1DD7F457EB6EAF0867EE9D3F5A855BD`.
Exactly `result.txt`, `build.log`, `clippy.log` match corrected source commit
`bc5e5d0e69f9664c72b08c3d9b62a5f4c987d494` and source archive digest
`bcefd254b89310fa4c42ecc6a6796cf67acae30a9f08143c8b928e7bd3fd9228`.
Native arm64 macOS 27.2/build 26B123 reports test-harness build pass, strict
all-target Clippy pass, zero failed checks and runner exit zero. The logs show
completed test/dev profiles without compiler warnings/errors. The original 80
passed native test entries and Keychain result are retained; they were not rerun
by this lint-only follow-up. The two dead-code cfg errors are closed for this
Mac source. Do not repeat Mac readiness/tests/lint for the same checkpoint.

Next available platform evidence is the existing Windows VM: the development
host still has five owned-HTTP failures with inconsistent native TCP-state
observations. Added `WIN-TEST.cmd` / `WIN-TEST.ps1` and a pinned-source packager.
The outgoing `%USERPROFILE%\Downloads\cryptex-win-test-20261108.zip` builds on
the VM using existing Rust 1.98.1/C++ tools, runs the ten-iteration loopback TCP
probe/parser self-test and instance nine/publication nine/owned-HTTP eight tests.
Only new isolated synthetic test runtimes are used; no owner profile, pairing,
Keychain/Credential Manager record or normal transport is accessed/activated.
Initial locked Cargo dependencies may download; later tests are offline.

The package captures all failures, rejects missing/empty/wrong-count test
summaries and returns `WIN-TEST-SONUC.zip`. It changes no firewall/antivirus/
Application Control/system execution policy. Host checks covered PS syntax,
process stdout/stderr and nonzero-exit capture, report metadata and pinned ZIP
bindings; actual VM native results are pending.

**Next owner action:** extract the Windows ZIP in the existing Windows VM and
open `WIN-TEST.cmd`; return `WIN-TEST-SONUC.zip` whether pass or failure. Mac
needs no further action at this checkpoint. Check source/package binding in
[Windows VM evidence](../evidence/m8/m8-windows-platform-checks-20261108.md).
Mac production publication/ancestor-handle/transport parity, live-controller
barrier, physical durability and independent review remain open. Normal
publication is disabled; M8 is not complete.

## Mac native groups passed; test-only cfg correction awaits Mac lint (2026-11-08)

Owner returned `MAC-TEST-SONUC.zip`, SHA-256
`FBCD68F17FBA6C4C510AF575EA38A33D240867D8B08C5473AE946CBBCF8C2F0E`.
The exact eight bounded members match source commit
`74c6c400d5ff9229625c0397dab5e3ef4f8344ab` and archive digest
`6c03ac3629aec97c1904e8a216a53e21e5356ec8459fbc80b07539ad011ae771`.
Native arm64 macOS 27.2/build 26B123 compiled the harness and passed all five
selected groups: instance eight, staging/recovery 58, incoming validation nine,
trusted probe three, native Keychain two (**80 passed total**, zero failures).
Logs confirm the new Unix symlink/hardlink refusal tests and actual native
Keychain matrix ran. These are supplied runner observations, not attestation.

Overall receipt correctly remains failed because strict Mac all-target Clippy
reported two dead-code errors: test-only `Operation::Config` and
`test_read_config` in `syncthing_owned_rest.rs`. Both callers are Windows-only
test modules. Corrected the enum variant, match arm and helper to
`cfg(all(test, windows))`; no lint suppression, test weakening, product behavior
change or endpoint-ownership fallback. Strict Windows all-target Clippy and
format passed with the correction; Mac Clippy on corrected source is pending.

Prepared a lint-only launcher using the same exact-source packaging workflow:
`%USERPROFILE%\Downloads\cryptex-mac-lint-20261108.zip`. It compiles the test
harness and runs strict Clippy on the Mac; it deliberately does not rerun the
80 passed tests or touch Keychain. The full native runner remains available.

**Next owner action:** extract that ZIP on the same Mac, open
`MAC-LINT.command`, return `MAC-LINT-SONUC.zip` (success or failure). Match the
new source binding in [Mac check evidence](../evidence/m8/m8-macos-native-checks-20261108.md)
before accepting the result. Preserve the passed native receipt. The Windows
HTTP failures, Mac production publication/ancestor-pin/transport parity,
runtime-wide controller barrier, physical durability and independent security
review gates remain open. M8 is not complete.

## Mac readiness received; native checks runner prepared (2026-11-08)

Owner returned `MAC-SONUC.zip`, SHA-256
`07B009241BAFF87A542B39CBDC6768F30CEAAD6BD7C590498BF72A0DA646452A`.
Exactly one bounded `platform.txt` (497 bytes) matches the readiness scope/probe.
It reports macOS 27.2/build 26B123, native arm64 (`translated=0`), CLT/clang and
Rust 1.98.1 present. Scratch copy, hardlink count two and symlink checks pass.
Filesystem type remains `unknown`; native Cryptex/secure-store/power-loss tests
explicitly remain not run. This receipt is a runner observation, not attestation
or a platform integration pass.

Prepared `MAC-TEST.command` plus an exact-commit source packager. The outgoing
`%USERPROFILE%\Downloads\cryptex-mac-test-20261108.zip` contains source code
and five allowlisted runner/binding members total, with executable Unix mode
and LF script. It checks source SHA-256, uses existing Rust 1.98.1 and CLT to
build the daemon library test harness on the Mac, then runs instance-file,
import-staging/recovery, incoming-validation, trusted-probe and native Keychain
groups, plus Mac all-target Clippy. Cargo.lock-bound dependencies may be
downloaded for the initial build; subsequent commands are offline. No toolchain
installation, `sudo`, production profile access or security-setting change.

Success/failure logs and source/OS bindings return as `MAC-TEST-SONUC.zip`.
Empty test selections cannot count as passing. Compile failure still returns
its report/log. Keychain cases use only existing synthetic dev test accounts.
The Windows-only persistent publication service is not enabled on Mac; selected
native checks do not establish managed peer ownership, full Mac replication,
physical durability, or independent review. Existing Windows HTTP failures and
the live-controller barrier/review gates remain open.

**Next owner action:** extract `cryptex-mac-test-20261108.zip` on that Mac,
open `MAC-TEST.command`, return `MAC-TEST-SONUC.zip` even when it reports a
failure. Actual native results are pending. Check the exact outgoing source
commit/hash recorded in [Mac native check evidence](../evidence/m8/m8-macos-native-checks-20261108.md)
before evaluating the returned report; do not rerun readiness or pairing.

## Security self-review fixes; Windows limits and Mac preflight (2026-11-08)

Reviewed the dormant persistent-import integration at `2705330` against the
Master Spec, accepted platform/sync contracts and ADR-0019 §5.1.1. This is an
implementation-author self-review, not the independent security review required
for activation. Verdict for normal publication remains **NOT READY**.

Reproduced a shared lifecycle defect with synthetic data: a hardlinked
`daemon.lock` let instance acquisition truncate another file before import
admission. Fixed instance-file opening through a platform adapter: no-follow,
ordinary/single-link handle checks before PID writes/inspection, Windows
non-delete sharing, and bounded PID reads through the already-locked handle.
The immutable target is now preserved by acquisition and observation regressions.
Also made the daemon-owned publication entry require an exclusive mutable
instance guard, preventing overlapping import/recovery calls through the same
safe-Rust API. The future controller still needs the runtime-wide reader barrier.

Latest focused Windows results: instance nine and publication nine passed;
daemon CLI fifteen passed; strict all-target Clippy, format and diff checks
passed. A broader daemon library run after the lock-file fix reported **553
passed, five failed, four ignored**. All five failures were owned HTTP endpoint
checks returning `PeerNotEstablished`. On this host (Windows build
26300.9818 / 26H2, NTFS), a new synthetic TCP probe recorded six mismatches in
60 samples while all 60 before/after round trips succeeded; parser self-test
passed. This supports the known host TCP-table diagnosis but does not make the
five tests pass or close the endpoint-ownership gate. No state check was relaxed.

One intermediate unsigned test build was blocked by Windows Application Control
(4551; Code Integrity 3077/3033/3118). The later ordinary build containing the
exclusive-guard fix ran both focused groups successfully. Protection settings
were not changed. This is not evidence that an unsigned distribution is allowed.

Owner confirmed a real Mac is available. Prepared
`%USERPROFILE%\Downloads\cryptex-mac-kontrol-20261108.zip`: two allowlisted
members, executable Unix mode and LF `.command`; no binaries, keys, profiles
or network/install steps. It reports macOS/build/architecture, scratch filesystem,
existing CLT/pinned Rust readiness and synthetic scratch link/copy capability.
Bash syntax and non-macOS refusal passed here; actual Mac execution is pending.

**Next owner action:** on the Mac, extract that ZIP, open `MAC-KONTROL.command`,
return `MAC-SONUC.zip`. If macOS blocks opening, return the warning rather than
disabling protection. Use that receipt to plan native Mac tests; do not call
readiness a macOS integration pass. Normal publication/managed adapter wiring,
clean Windows endpoint evidence, macOS native integration, physical durability
and independent review remain open. Detailed findings and source/check matrix:
[security and platform review](../evidence/m8/m8-security-platform-review-20261108.md).

## Windows internal persistent-import integration implemented (2026-11-08)

Owner requested continuation from the successful validation relay into permanent
import integration. Added crate-private Windows `incoming_publication` orchestration:
runtime instance ownership, startup reconciliation before canonical inspection,
bounded incoming snapshots, the existing complete local-anchor trust/M3/M4 gate,
prepared manifest, additive no-replace publisher, receipt-bound cleanup, and
verified canonical-to-M3/M4 refresh before success. No CLI, IPC, watcher or managed
transport caller activates it. ADR-0019 §5.1.1 implementation permission and its
remaining activation gates are unchanged.

Exact replays are removed only from private staging under the successful
runtime-bound inventory/device-lock snapshot, after no-follow, single-link,
length/digest checks against existing canonical segments. All-identical batches
return zero published events; mixed old/new batches publish only a contiguous
new tail. Divergent bytes reject. Transport source files and existing canonical
event bytes are never replaced or deleted.

Errors distinguish rejected admission, recovery-required preparation/publication,
and committed canonical data with a failed derived refresh. Preparation errors
preserve transaction evidence because the manifest may already be visible;
partial publication is never cleaned as a failed unprepared import. Restart
recovery does not fabricate fresh M3/M4 watermarks: a stale derived view still
holds new admission until an explicit verified rebuild.

The daemon-owned entry point also accepts a matching existing instance guard,
refuses a guard belonging to another runtime, and never reacquires the caller's
instance lock. A future controller must propagate recovery-required outcomes
into its runtime-wide admission barrier before allowing readers to continue.

Windows verification passed: eight new integration test entries (one subprocess
worker), including peer persistence/reopen, duplicate/mixed replay, invalid
batch atomicity, owned-runtime refusal, post-commit refresh failure, and six
process-exit boundaries across prepared temp/final, two destination moves and
committed temp/final. Existing staging/recovery 63, validation-only nine and
trusted-probe three tests passed; strict daemon all-target Clippy and format
passed. Fixtures use signed mutual approval histories and FakeSecureStore on
the host's real filesystem. This is not independent review, VM publication,
physical power-loss proof, or macOS evidence.

**Next:** review the internal transaction integration and idempotency/error
boundaries before adding any external publication entry point. The managed
adapter-to-service caller remains absent; native platform durability and
independent security review gates remain open. No new VM package was issued,
and existing owner profiles remain untouched. See
[implementation and review handoff](../evidence/m8/m8-persistent-import-integration-20261108.md).

## Trusted-import relay and VM full-policy checks passed (2026-11-08)

The owner returned `GUVEN-SONUC.zip` for probe
`c223e671-3c5e-413d-a82c-419ed88a508f`, retained native session
`87a57c018d63481cb71a9a0f6abdd1f1`. Its single bounded JSON receipt matches
the exact host settings, native peer IDs, sender completion, and retained source
ZIP: 4,120 bytes, SHA-256
`04BF510CCD9A6284DF2B2AA27E9107AB4D2254BB6F13F036882AB438DF2BADD4`.

The actual VM daemon accepted one trusted segment containing three events,
verified the genuine unapproved fixture signature and rejected its admission,
rejected the corrupted incoming copy, and accepted the restored trusted batch
again. The runner reports unchanged canonical event hashes and approval journal,
no unknown canonical device directory, and clean native shutdown. Both positive
reports remain `validated_only`, `published: false`.

Host evidence records the pinned VM carrier connecting from
`192.168.177.131:55500` over TLS 1.3 at 15:49:43 local / 20:49:43 UTC and
clean sender exit at 15:49:46 local / 20:49:46 UTC. The VM receipt timestamp is
20:49:47.7054539 UTC. The exact owned sender executable is no longer running.
See [actual receipt and limits](../evidence/m8/m8-trusted-import-relay-20261108.md#actual-windows-vm-result).

**Continuation:** basic random transport, signed transport, and this full-policy
validation relay are complete; do not repeat their copy/paste setup or pairing.
No VM action or sender needs to remain active for this checkpoint. Preserve the
two original VM profiles and public transcripts. Next engineering gate is the
normal managed transport-to-staging/canonical-publication path, subject to the
authoritative specification/ADR and its existing review gates. This test used
two owner-approved logical Cryptex profiles inside one VM with a host relay;
it does not establish independent OS-level Cryptex enrollment or durable import.
Managed-adapter integration, canonical publication, macOS, power-loss and
independent security review remain open; M8 is not complete.

## Trusted preparation received; full-policy relay test ready (2026-11-08)

Historical package checkpoint; actual VM results are recorded above.

The owner returned `GUVEN-HAZIRLIK.zip` for probe
`c223e671-3c5e-413d-a82c-419ed88a508f`. The inspected 4,120-byte archive has
SHA-256 `04BF510CCD9A6284DF2B2AA27E9107AB4D2254BB6F13F036882AB438DF2BADD4`.
Its exact four members are the bounded manifest, one approved-source segment,
one same-Brain unapproved-signer segment and the latter's public descriptor.
Manifest identities/hashes match the original owner-approved pilot profiles.
The unapproved fixture's signature was independently verified using the daemon;
that signature alone grants no trust.

Prepared `%USERPROFILE%\Downloads\cryptex-guven-test-c223e671.zip`. Its nine
allowlisted members are the runner, pinned daemon, settings and supporting
scripts/instructions. It contains no source segments, payload archive or keys:
the original returned archive must travel over the retained native carrier.
The host relays public synthetic data from the VM's source profile back to its
receiver profile. This does not establish independent OS-level Cryptex enrollment.

`GUVEN-TEST.cmd` receives and hashes the payload, shuts the native receiver down,
then runs actual `cryptexd replication validate-incoming` against the original
receiver runtime. It requires trusted-batch acceptance, valid unapproved-signer
rejection, corrupted-copy rejection, repeat acceptance, and unchanged canonical
event hashes/approval journal. Reports keep `canonical_import: validated_only`
and `published: false`; no normal publication is enabled.

Local checks passed: real bundle/member/hash inspection and genuine negative
signature verification; PowerShell orchestration tests with mocked CLI outcomes
(including unrelated-error refusal and traversal rejection); the existing native
signed-transfer regression; strengthened Rust negative-policy test; strict daemon
Clippy, format, script parsing and diff checks. Mocked orchestration is not VM
policy evidence. The returned final `GUVEN-SONUC.zip` is still pending.

**Next owner action:** extract `cryptex-guven-test-c223e671.zip` in the same VM,
open `GUVEN-TEST.cmd`, return `GUVEN-SONUC.zip` or `AKTARIM-HATA.zip`.
The host sender uses session `87a57c018d63481cb71a9a0f6abdd1f1`, its retained
`vm-public-request.json`, and package directory
`trusted-package-c223e6713c5e413da82c419ed88a508f` beneath
`%LOCALAPPDATA%\Cryptex-Dev\peer-transfer\<session>`. Start it with
`scripts/m8/start-peer-sender.ps1` and `-WaitSeconds 1800` before asking the owner
to run the test; a future session must check/restart it, not assume it is live.
Review the returned receipt against the exact settings and sender completion.
Publication, managed-adapter, macOS, power-loss and independent-review gates
remain open. See [relay test evidence and scope](../evidence/m8/m8-trusted-import-relay-20261108.md).

## Trusted-import preparation package ready (2026-11-08)

Historical package checkpoint; the preparation receipt is recorded above.

The owner requested the trusted-device/import check as another one-click test.
The implementation reuses the two **previously owner-approved Cryptex pilot
profiles in the VM**, rather than automatically approving new keys. The host
will be an untrusted transport relay for their public synthetic segments; this
does not claim two independent OS installations' Cryptex trust enrollment.

New explicit daemon pilot command `trusted-import-prepare-v1` owns/reconciles
the marked source and receiver runtimes and a fresh negative fixture. It loads
both OS-held local anchors, re-verifies their signed histories, checks reciprocal
direct peer bindings and unrestricted authority, and refuses non-synthetic source
notes before capture/export. It captures one idempotent public test note and
seals through the existing writer. It prepares only an already-approved empty
writer slot at the receiver and rebuilds disposable M3/M4 from the same verified
cutoff-aware content snapshot used by the safety gate. Strict trust, signature,
continuity, inventory and M3/M4 comparisons are unchanged. No approval event,
trusted-key SQLite row or published peer event is injected.

The negative fixture signs a valid event claiming the same Brain ID using a
fresh unapproved OS-held key. Only its public descriptor/segment are exported.
All signing seeds stay in VM secure storage. The exporter whitelists relative
segment paths and verifies file hashes; the outgoing archive contains only the
manifest and known synthetic public/control event data, never executable code
or runtime key/config files. Canonical source notes are appended, never edited;
receiver canonical event bytes are not imported by this preparation step.

Checks passed: three new trusted-policy tests (including valid peer acceptance,
duplicate validation without publication, stale-watermark hold, valid same-Brain
unapproved signer rejection, corrupted-record rejection, missing-approval and
non-synthetic-source refusal), nine incoming-validation regressions and fifteen
CLI tests. Strict Clippy, format and release build passed. PowerShell package
checks covered member allowlist/daemon hash, missing-profile error reporting,
source preservation and path/hash rejection. These are local tests; VM native
preparation and transported full-policy validation remain pending.

**Next owner action:** extract
`%USERPROFILE%\Downloads\cryptex-guven-hazirlik-c223e671.zip` in the same VM,
open `GUVEN-HAZIRLA.cmd`, return `GUVEN-HAZIRLIK.zip` or `GUVEN-HATA.zip`.
Probe: `c223e671-3c5e-413d-a82c-419ed88a508f`.
Expected Brain/source/receiver remain
`99e94c1a-0c62-48a0-88d2-61b9a146e021`,
`74ab8018-fc82-4815-84c6-48d728578184`,
`a61f5092-6a37-4f7c-acc6-e470d6f438c2`.
The package pins the original `cx-m8-c152.../desktop` source and
`cx-m8-7333.../desktop` receiver. No new VM, copy/paste pairing or source key
export is needed. After the returned bundle is inspected, prepare the native
relay package and run the actual `replication validate-incoming` path with
positive/unapproved/tampered cases. Do not label preparation as the VM import
test passing; publication/managed-adapter/platform/review gates remain open.
See [scope and verification](../evidence/m8/m8-trusted-import-preparation-20261108.md).

## Host-to-VM signed transport probe passed (2026-11-08)

The owner returned `IMZALI-SONUC.zip` for probe
`c15cf5f1870648e9b6bdaa5129766cb7`, retained native session
`87a57c018d63481cb71a9a0f6abdd1f1`. The VM reports receipt of 723 bytes,
matching expected SHA-256, successful daemon signature verification, rejection
of a separately tampered copy, and clean native shutdown. Its flags explicitly
remain `trust_activated: false`, `published: false`, `canonical_import: not_run`.

The agent matched the exact probe/session/native peer IDs/length/hash to host
settings and sender completion, rehashed both the original sealed canonical
source and its transport copy, and independently reran source signature
verification. The host log records the VM's 192.168.177.131 endpoint over TLS
1.3 at 19:38:43 UTC; host native exit was recorded at 19:38:46 UTC. The exact
owned sender executable has no remaining process. See [bound signed receipt](../evidence/m8/m8-signed-transport-probe-20261108.md#actual-windows-vm-result).

**Next:** establish the appropriate authenticated Brain/peer history and exercise
the full daemon `replication validate-incoming` trust/continuity/retrieval-policy
path for a transported signed segment. The cryptographic probe's supplied test
key is not a trusted-roster entry and must not be used to bypass that gate.
Canonical publication remains inactive pending its existing review/platform
requirements. The basic random and signed native host-to-VM checks are complete;
do not repeat them. No test sender remains active; the VM may be closed for now.

## One-click signed transport probe prepared (2026-11-08)

Historical preparation checkpoint; actual VM result is recorded above.

The owner requested the next signed-data test as a portable one-click package.
Added explicit daemon pilot commands `signed-transfer-v1` (fresh marked runtime,
OS secure store, daemon canonical writer, one sealed synthetic v1 event) and
`verify-signed-transfer-v1` (bounded read-only signature/identity/frame check).
Verification uses the existing event validator and Ed25519 implementation.
Its public descriptor is an expected test key, not a trusted-roster entry;
reports explicitly keep `trust_activated` and `published` false.

The new signed package reuses the already tested host/VM native identities and
folder. The owner only extracts and opens `IMZALI-TEST.cmd`; no new public-ID
request or pairing is needed. It transports a newly named sealed event, verifies
the received hash and signature, and requires a separately modified copy to fail
with the daemon's integrity error. The original received file is rehashed to
confirm preservation. No source segment or private key is included in the ZIP;
only the daemon, public descriptor, pinned expected hashes and scripts ship.
It emits one `IMZALI-SONUC.zip`, or an error ZIP.

Validation: four probe library tests, fifteen daemon CLI tests, strict Clippy and
format checks passed. A release build and native two-process regression passed,
including the base random transfer, retained-profile restart, new signed event
transfer, positive signature verification and negative modified-copy check.
Initial packaging exposed a Windows path-length failure in public signing-key
metadata; new signer fixtures use a short protected LocalAppData/Cryptex-Dev/sp
path. Retained plain synthetic folders explicitly clear native encryption
password placeholders on reopen. Sender completion uses the actual source-file
inventory size to avoid stale index observations and fast-disconnect races.

**VM signed result passed, as recorded above.** This probe verifies cryptographic integrity under
an explicitly supplied public test key. It is not the `validate-incoming`
local-trust/continuity/retrieval-policy path. The next integration gate still
requires an appropriate authenticated Brain/peer history; no trust bootstrap or
canonical publication gate was bypassed. See [probe evidence and scope](../evidence/m8/m8-signed-transport-probe-20261108.md).

Prepared VM package: `%USERPROFILE%\Downloads\cryptex-imzali-c15cf5f1.zip`.
Probe ID: `c15cf5f1870648e9b6bdaa5129766cb7`, 723 bytes, SHA-256
`2C421940C8820F49D49569141EFEB0E70BA02960037D1526BBFA2194E1F7A77F`.
Retained native session remains `87a57c018d63481cb71a9a0f6abdd1f1`.
Agent starts the sender with its existing `vm-public-request.json` and
`-PackageDirectory <session>/signed-package-c15cf5f1870648e9b6bdaa5129766cb7`.
The owner then opens `IMZALI-TEST.cmd` on the same VM and returns
`IMZALI-SONUC.zip`. Do not recreate native identities or use regression sessions.

## Host-to-VM native file transfer passed (2026-11-08)

The owner ran the portable package in the existing Windows VM and returned
`ISTEK.zip`, then `AKTARIM-SONUC.zip`. Session
`87a57c018d63481cb71a9a0f6abdd1f1` transferred 32768 synthetic bytes from host
192.168.177.1 to VM 192.168.177.131 using distinct pinned Syncthing identities.
The returned receiver hash equals the retained original source and prepared
hash. Sender reported peer completion; both sides reported clean shutdown.
The host log independently records the VM's LAN address and TLS 1.3 connection;
the exact owned host executable has no remaining process.

This closes the independent Windows **native synthetic transport** check.
It does not exercise Cryptex's managed adapter, signed-event policy validation,
canonical import/publication, power loss, or macOS. The receipt explicitly says
`canonical_import: not_run`. See [bound receipt and evidence](../evidence/m8/m8-windows-host-vm-native-transfer-20261108.md).

**Next:** extend the automated harness toward transporting a daemon-produced
signed segment and validating it through the daemon's incoming-policy entry
point with the appropriate Brain/peer identity. Keep canonical publication
behind its existing activation gates. Do not repeat the completed same-VM
pairing/replay or basic host-to-VM synthetic transfer. No native test sender
is currently left running; the owner can close the VM for now.

## Independent Windows transport test package (2026-11-08)

Historical preparation checkpoint; the actual transfer result is above.

The owner returned the single VM preflight archive. Collection had no errors;
the two original journals still report anchored / four messages / one append
attempt. No Cryptex processes were running. The VM reported 192.168.177.131/24;
the host VMnet8 interface is 192.168.177.1/24. Connectivity is not yet proven.
The original pairing/replay exercise needs no repeat.

Added a portable two-step **synthetic native transport** harness. It uses the
existing pinned Syncthing 2.1.5 binary, isolated private test profiles, distinct
host/VM native identities, a 32 KiB random file and its expected SHA-256.
`package-peer-transfer.ps1` retains a host fixture and emits a VM ZIP with
`1-HAZIRLA.cmd` and `2-AKTAR.cmd`. First the VM generates its own private keys
locally and returns only `ISTEK.zip` with the public native ID/session. After
validating this request, the agent runs `start-peer-sender.ps1`; then the owner
clicks `2-AKTAR.cmd` in the same extracted package. The VM connects outbound to
the explicit host private address and returns a result/error ZIP. No key/config
or raw log is exported. Existing Cryptex profiles and canonical files are unused.
Only the host test sync listener binds its explicit LAN address; APIs stay
loopback-only. Discovery, relays, NAT and upgrades stay off. No firewall or
antivirus settings are modified. Sender waits at most ten minutes by default;
receiver waits three. Processes are stopped after the attempt.

PowerShell 5.1 native regression passed a two-process loopback transfer with
matching bytes/hash and clean stops, request/session rejection, and private-IP
validation. Early harness failures were corrected before packaging; in
particular folder-device records explicitly set blank encryption passwords,
and sender lifetime allows the receiver to finish verification. This is not
host-to-VM evidence yet and does not validate signed Cryptex events, canonical
publication, the managed production adapter, power loss, or milestone completion.
See [test scope and continuation](../evidence/m8/m8-portable-peer-transport-20261108.md).

**Next:** get `ISTEK.zip` from the VM, validate its exact session/native pin,
start the scoped sender and have the owner run `2-AKTAR.cmd`. Use the returned
hash/connection/shutdown report to assess actual host-to-VM native transport.
Then continue the daemon policy/import integration gates below.

Current prepared session: `87a57c018d63481cb71a9a0f6abdd1f1` under
`%LOCALAPPDATA%\Cryptex-Dev\peer-transfer`. Owner package:
`%USERPROFILE%\Downloads\cryptex-aktarim-87a57c01.zip`. Sender has not started;
wait for this session's VM public request. Do not use loopback regression
sessions for the real VM attempt.

## One-click VM preflight package (2026-11-08)

The owner chose a portable runner instead of repeated terminal commands and
screenshots. `scripts/m8/package-vm-check.ps1` packages `VM-KONTROL.cmd`, the
standalone PowerShell 5.1 collector, instructions and a file-hash manifest.
Extract in the existing VM and double-click the launcher; Explorer selects
one `SONUC.zip` to return. No Git, Rust, pnpm, administrator rights or new VM
is required. The launcher's execution-policy override is process-only.

This first package is **read-only preflight, not the peer-transfer test**.
It collects OS/PowerShell versions, local IPv4 addresses, relevant process IDs
and executable hashes, and marked TEMP pilot journal counts/states. It exports
no event bodies, signed messages, keys, full logs or full process command lines.
It reports missing/invalid data, does not create or restart pilot profiles,
does not alter trust/network/protection settings, and uploads nothing.
Reports live outside the repo under LocalAppData/Cryptex-Dev/vm-checks.
File states are observations, not fresh signature or connectivity verification.

Windows PowerShell 5.1 regression covers summary counts, private-content
exclusion and file preservation, malformed/unknown journals, invalid IDs,
oversized journals, wrong markers and junction refusal. The collector also
completed a host smoke run and produced its report archive. VM execution now
also completed, as recorded above; publication activation gates remain unchanged.

## Approved cleanup receipt and Windows startup integration (2026-11-07)

The owner approved the narrow cleanup-receipt amendment. ADR-0019 section 5.1.1
now records that approval. Committed cleanup writes a bounded, closed receipt
containing the exact manifest and commit decision before deleting companions.
Recovery validates all destinations and remaining evidence, resumes cleanup,
and removes the receipt last. Without a receipt, missing companions still hold.
Unknown, malformed, conflicting and aliased evidence is never silently removed.

Windows startup now passes its live instance guard to policy-bound recovery
before identity/materialization, transport or IPC starts. Unmarked partially
visible imports finish under the pre-/post-import policy token. Temporary commit
markers require that same complete policy verification and every destination;
the context-free API now holds them instead of promoting by hashes alone.
Final commit/receipt cleanup does not re-admit under a newer trust policy.
Normal transport publication remains inactive; this is startup recovery wiring.

Verification: the import-staging suite passed **63 tests**, including 15 complete
cleanup interruption boundaries, an incomplete-receipt hold, 26 conflict cases,
signed startup branches and marker-promotion interruptions. Canonical inventory,
device and importer-lock contention checks passed. Daemon binary tests:
**14 passed**; incoming-validation regression: **9 passed**. Strict Clippy,
format and diff checks passed. Windows Application Control intermittently blocked
an intermediate test build (`4551`); subsequent normal builds/tests ran without
changing protection settings. See [current recovery evidence](../evidence/m8/m8-cleanup-receipt-startup-20261107.md).

**Next:** obtain Windows package/native peer-stream, power-loss/platform, macOS
and independent-review evidence before activating transport publication. The
native nonzero-cutoff peer review still depends on supported publication;
completed pairing/replay needs no repeat. This closes the implemented Windows
process-interruption recovery slice, not the whole M8 milestone.

## Cleanup interruption gate and protocol decision (2026-11-07)

Historical checkpoint; superseded by the approved implementation above.

Three new real-process interruption checks expose a startup availability gap:
after committed staging or manifest cleanup, the remaining marker lacks a
required companion and recovery returns a typed hold on every retry. Canonical
bytes and surviving evidence remain unchanged. After the last marker deletion,
recovery succeeds repeatably. Existing production behavior was not changed.

The focused regression passed; strict daemon Clippy passed. The subsequent full
import-staging suite could not execute because Windows Application Control
blocked its test executable (OS error 4551). The previous 57-test pass below
does not cover this change. See [audit evidence](../evidence/m8/m8-cleanup-interruption-20261107.md).

**Next decision:** review the [bounded cleanup-receipt proposal](proposals/m8-import-cleanup-receipt.md).
ADR-0019's normative Windows table explicitly requires a hold for absent
companions; automatically resuming our own interrupted cleanup needs the
proposed protocol amendment. It is not implemented or approved. After approval,
implement/test receipt-bound cleanup, then complete policy-bound startup
integration (including temporary-marker validation). Keep publication inactive
until its remaining gates pass. No repeat of the completed VM pairing is needed.

## Prepared import recovery finisher candidate (2026-11-07)

Added a private, inactive finisher for unmarked prepared imports with some or
all destinations already visible. It requires the matching runtime instance
lock, reloads local secure-store identity and the read-only projection, and
retains the existing pre-/post-import policy token's canonical locks across
remaining no-replace moves and committed-marker creation. It rechecks the
manifest, journal namespace, exact file locations/digests, aliases and active
file conflicts before moving. No-visible-destination transactions still belong
to the existing abort path.

Windows import-staging suite: **57 passed**, including eight rejection cases
and four subprocess interruption boundaries for the new finisher. Strict Clippy,
format and whitespace checks passed. Production startup still holds unmarked
visible imports; neither the finisher nor normal publisher was activated.
See [candidate recovery evidence](../evidence/m8/m8-prepared-import-finisher-20261107.md).

Next audit the complete startup decision table before integration, especially
temporary-marker policy revalidation and interruption during cleanup. Obtain
the required durability/platform and independent-review evidence before
activating transport publication. Native peer restriction remains downstream;
the completed pairing/replay checks need no repeat.

## Incoming validation ownership and crash checks (2026-11-07)

The validation CLI now acquires runtime instance ownership before opening the
import staging session. Existing-root checks and the staging module's platform
directory inspection precede that acquisition. A running daemon therefore
causes refusal before staging directories or an import slot are created.
The instance lock remains held through discard. Interrupted/unrecognized import
state still requires recovery; validation does not silently remove it.

Windows targeted validation tests: **9 passed**, including the subprocess worker
entry and a parent test that interrupts validation after staging and after policy
verification. Repeated startup recovery leaves canonical segment bytes and
incoming source bytes unchanged. Strict Clippy, format and whitespace checks
passed. These are process-interruption checks with synthetic fixtures, not
power-loss, native peer import, or macOS evidence. Publication remains disabled.
See [test scope and continuation](../evidence/m8/m8-incoming-validation-crash-checks-20261107.md).
Next implement and test policy-bound recovery of unmarked prepared imports with
partially visible destinations, retaining the existing publication activation
gates. The one-VM pairing/replay results below remain valid and need no repeat.

## Incoming-validation integration (2026-11-07)

Added `cryptexd replication validate-incoming --runtime-root <absolute-path>
--environment <dev|prod> <relative-segment>...`. It exercises bounded staging,
local secure-store trust, the existing read-only projection/retrieval gate and
post-import policy verification, then discards staging. Its report explicitly
uses `validated_only` and `published: false`. Five library tests and one binary
argument test passed, along with strict Clippy and formatting checks.

The publisher remains behind its documented recovery/durability activation
gates. The native nonzero-cutoff restriction test still requires a peer's sealed
stream to reach the reviewing profile through verified daemon publication.
See [entry-point evidence and next prerequisites](../evidence/m8/m8-incoming-validation-entrypoint-20261107.md).

## Latest native replay check (2026-11-07)

The owner reopened the initiator on build `19d94b1` and submitted the original
four-message transcript again through **Resume a signed pairing**. The UI
reported canonical pairing recorded. A before/after SHA-256 comparison of
canonical event files (excluding lock files and the derived writer tip)
returned unchanged files. The transaction remained `anchored` with exactly one
append attempt. The owner then repeated the reopen/replay comparison on the
responder and supplied the same `True / anchored / 1` result. Both role checks
passed. Evidence: [two-profile pairing and replay](../evidence/m8/m8-windows-two-profile-pairing-replay-20261107.md).
Next prepare native approved-peer restriction review at a nonzero exact cutoff.
First establish a peer event in the reviewing profile through a supported
daemon import/replication route. Empty-history first pairing and append-unknown
crash recovery remain separate open items.

## Current Windows pilot checkpoint (2026-11-06)

The responder journal showed four verified messages, state `transcript_ready`,
and zero recorded append attempts. The relaunched pilot subsequently confirmed
a public test note was saved. Restart cleared the Desktop's in-memory approval,
which previously hid the completion form. Desktop now exposes **Resume a signed
pairing** after restart. Paste the original four signed lines there to invoke the
existing daemon completion verifier against the same journal. On build
`19d94b1` the owner did this in the resumed responder profile. Desktop
reported “Pairing recorded in canonical local history.” This confirms the
responder-side completion via the UI. After instructions to switch to the
original initiator profile, the owner supplied the same success message for
the initiator completion attempt. The owner then read both journals in the
Windows VM. Initiator and responder each reported `anchored` and four transcript
messages for the same transaction. This closes the one-VM, two-profile signed
pairing/local trust-recording exercise. Next verify restart reconciliation and
duplicate-safe completion, then resume native nonzero-cutoff restriction review.
Production sync, empty-history first pairing, and independent-device coverage
remain open gates.

The original signed inputs passed responder review. The VM created the fourth
signed message and its signature was independently verified. The subsequent
**Add approved device to local trust** request returned a rejection. The
earlier UI hid the daemon's error code. Desktop now displays known completion
codes. That rejection preceded the successful responder retry described above.

## Latest interactive continuation — responder pairing (`2026-11-05`)

The one-VM Windows pilot pairing resumed with the initialized responder
profile, but responder owner-review verification still returns
`invalid_transcript`. No responder approval or durable trust record was
created; no synchronization started. The role check after reopening has not
yet been confirmed. Source inspection shows the responder prefix is assembled
as offer, proof, initiator approval, while the daemon intentionally returns a
generic error for any signature, ordering, or binding failure. Diagnose the
exact message failure before asking the owner to retry. Preserve signed JSON
exactly and keep raw device identifiers/messages out of this repository.

See [`M8_WINDOWS_RESPONDER_PAIRING_HANDOFF.md`](M8_WINDOWS_RESPONDER_PAIRING_HANDOFF.md)
for the full sequence, verified and unverified state, code pointers, safe next
actions, and remaining M8 gates.

## Windows managed transport pilot continuation

The Windows Desktop pilot accepts `--cryptex-pilot-transport <ready-profile>
<pinned-package-dir> <api-port> <sync-port>` alongside the existing marked
runtime option. It forwards the four values to the Desktop-owned daemon on
launch and restart. The daemon validates the package, profile and ports.
The Windows daemon now exposes `pilot-transport --confirm-isolated-pilot
<fresh-marked-root> <pinned-package-dir> <api-port> <sync-port>`. The marked
provisioning root is separate from the Desktop runtime. The command creates
its private child with a random 32-character hex name using package validation,
profile provisioning, and Windows credential store.
The host Release daemon provisioned the corrected profile and passed a
bounded managed lifecycle run. That run reached ready and exited with code
0. The standalone daemon reported health-only IPC because no Desktop peer
was present. Windows Application Control blocked the staged Desktop launch
on the host. The corrected `cryptex-m8-ready-eb9bc4c.zip` is ready for transfer
to the one Windows VM for Desktop adapter identity verification. Host-side
pairing still needs a working second endpoint.
The installed host Syncthing binary matches the repository's Windows SHA-256
pin. After building both Release binaries on Windows, run
`scripts/m8/start-windows-transport-pilot.ps1 -SyncthingExe <pinned-exe>`.
Use `-JoiningDevice` for the endpoint that will join the initiator's Brain.
The runner creates fresh roots, waits for the daemon ready file, and opens
the pilot window for the adapter identity check. PowerShell parsing passed.
The owner launched the corrected Release pilot in the Windows VM and selected
**Read verified adapter identity**. The screenshot showed a verified adapter
DeviceId and the refresh button. This verifies the VM's native Desktop adapter
identity read. The owner then supplied the signed offer JSON produced by
the VM pilot. Responder verification, two-endpoint pairing and restriction
review remain pending. A host Release
attempt exposed the original fixed profile name as invalid. The daemon now
generates a valid leaf and the launcher consumes the reported path.
The VM responder displayed `bootstrap exchange operation mismatch` while
creating proof. Desktop now preserves the daemon's typed exchange rejection
instead of masking it. Two targeted proof tests passed. The underlying
responder rejection still needs to be observed in a rebuilt Desktop.
Source inspection also found that the responder textarea submission removed
the mandatory final LF from the canonical Brain identity. Desktop now
restores that LF and converts pasted CRLF to LF before submitting the public
identity. The daemon still compares the result with its canonical form.
The VM responder subsequently created a signed proof and the initiator
resumed its saved offer. The initiator review exposed a missing journal
transition: preparation expected a previously stored proof. The journal now
records the verified proof against the exact saved offer and promotes the
exchange in the same update. The targeted regression passed. A rebuilt VM
retry passed. The VM initiator signed its approval and the owner supplied
the signed message. The Windows transport pilot now supports explicit
`--cryptex-pilot-resume` for the initialized, marked temp layout so the
responder can be reopened with its original identity on the same VM.
Desktop pilot tests passed 11/11. Native responder resume and approval remain
pending.
The targeted daemon test passed: provisioning rejects a reused marked root
before opening the package and preserves existing files.

## Latest continuation — Windows owner-review renderer crash isolated (2026-11-03)

The owner repeated the native self-retirement review preparation twice on the
standard-user Windows 26200.9711 / 25H2 VM, using the marked synthetic runtime
`%TEMP%\cryptex-m8-owner-638f41528100470eb48ccac291b9b4ce`. Each **Prepare owner
review** click blanked the Desktop WebView. Desktop and daemon processes stayed
alive; `gui-proof.snapshot` reported a visible window and the expected runtime
root, with no `start.error`, `window.error`, or `restart.error`. `Ctrl+R`
restored the initial form. The owner did not click **Confirm security change**.

Root cause: Tauri returned the journal's `ApprovalSnapshot` directly. Durable
snapshot structs serialize snake_case, while React reads camelCase and
dereferences `snapshot.localIdentity.deviceId`; the mismatch throws during
render and blanks the WebView. The candidate adds dedicated camelCase Desktop
snapshot, identity, and transport DTOs at the IPC boundary. The persistent
journal serialization remains unchanged. A Rust regression test asserts both
the Desktop camelCase response and the unchanged durable snake_case shape.

Verification on the development host: focused regression **1/1**, frontend
TypeScript/Vite build PASS, Desktop Rust `cargo check` PASS, daemon library
Clippy with `-D warnings` PASS, format and diff checks PASS, and unbundled
Tauri release package smoke PASS. The package smoke used Node **26.13.0** and
pnpm **11.28.0**; it emitted the expected engine mismatch warning because the
repository pins Node 24.19.0. Current daemon library run: **519 passed / 2
failed / 4 ignored**. Both failures are existing owned-process HTTP checks
rejecting the development host's inconsistent native TCP state as
`PeerNotEstablished`; no ownership/state checks were relaxed. Built Desktop
SHA-256 `B9AFA1D304B8D99567237AFDE1AE822F35FB2411F43ADAA46FD397FEF220FEBC`;
sidecar SHA-256 `DFE5C16E5206DCDAF0BB675D5EA7A7ED2A9057844A30F8CE64A32FBCB4462CAA`.

**Post-fix second-VM verification:** source `b733e436b0f9f69ded13699aa012fc6b2901650d`,
Windows `26200.9711 / 25H2`, standard user, Node `24.19.0`, Cargo `1.98.1`,
pnpm `11.28.0`. The first package attempt under the long temp path failed in
MASM with `A1009: line too long`; re-extracting under `%TEMP%\cx-b733e43` fixed
the build path. The unbundled Tauri Release package smoke then passed. Owner
reported Desktop SHA-256
`2ADB6443E58A0B8CA893BB6EE17423311C8897A3D42E0B73C68189A295BAC6CD` and daemon
SHA-256 `2B8431464AB8C94CE22AAA03D9C2F54A0E245D16DBC285DC0B7FA5525C556E1A`.

Using a fresh marked synthetic runtime and fixture Brain
`18b2f76c-eae5-46ff-b7b9-8ac3f110ff46` / event
`08fb17ca-f651-4e08-82c5-2ae718128062`, the corrected UI rendered the full
self-retirement review. It showed operation `self_retirement`, transaction
`8effe1659f080bb983c85445076ea395f4d390484e56717d16c0d3db19ae5c1e`, issuing
device `102ca9af-4077-4de2-bc58-3bf436c9cf02`, signing key
`d49ab5902c64736d82b654986afbdaac`, last-authorized sequence `1`, and hash
`9f142876c5969a1f5bdbb41ec48c8b53dc8a8f9c441960a04aeb07a236f1db06`. The
owner selected **Cancel**. Cryptex displayed **“Operation cancelled. No
canonical event was written.”** The owner then prepared self-retirement a
second time. It produced a different transaction ID (prefix `eebbae8c`),
displayed the same exact local device and sequence-1 boundary, and was
cancelled with the same no-append status. No confirmation was performed. This
is a scoped native review, cancellation, and post-cancel retry PASS;
The owner then submitted a restriction request for syntactically valid but
unapproved device `00000000-0000-4000-8000-000000000001`, reason `compromised`,
cutoff `1`. The
UI displayed **“Cryptex could not verify this owner operation against current
canonical trust history. Nothing was changed.”** No review or confirmation
followed. This is a scoped negative owner-policy check. The two pre-fix
preparations were refreshed without cancellation, so their old synthetic
journal may retain `OwnerPending` records; it was not reused. Owner-operation
session deadline is **5 minutes**; 15 seconds is only the Tauri-to-daemon
connection timeout. Targeted local Rust checks also passed: exact approved
peer cutoff/hash and rejection of a later boundary (`restriction_request_binds_approved_peer_and_exact_verified_cutoff`), empty-prefix sentinel validation (`owner_operation_boundaries_require_exact_empty_prefix_hash`), the synthetic peer-bootstrap plus nonzero-cutoff restriction IPC flow (`bootstrap_approval_ipc_freezes_displays_and_journals_before_signature_release`), and self-retirement append recovery (`confirmed_self_retirement_append_recovers_after_durable_append`), each **1/1**. These tests do not constitute native Desktop review of an approved-peer restriction or a native restart/recovery pass; those remain open.
See [the detailed rendering evidence](../evidence/m8/m8-windows-owner-review-rendering-20261103.md).

## Latest continuation — second-Windows Desktop smoke PASS (2026-10-30)

The owner supplied a successful unbundled release Desktop/daemon package-smoke
log on Windows 26200.9711 / 25H2, standard-user session, source archive `a5f859a`.
Node 24.19.0 and pnpm 11.28.0 were used. This adds a build PASS to the 520/0/4
library and reported 12 binary passes; it does not verify an interactive owner
flow. Next: record executable hashes and launch a fresh marked, synthetic
pilot outside the source tree through the supported daemon fixture and Desktop
pilot flag. Verify startup before reviewing/cancelling self-retirement.
Evidence: [`m8-second-windows-desktop-native-verification-20261030.md`](../evidence/m8/m8-second-windows-desktop-native-verification-20261030.md).
No installer, native owner-flow, transport, macOS parity, or M8 completion claim.

## Latest continuation — native TCP state diagnostic (2026-10-29)

**Latest second-host standard-user verification:** daemon library **520 passed
/ 0 failed / 4 ignored**, exact result supplied by the owner (32.52s); binary
**12 passed**, separately owner-reported. Source archive `a5f859a`, Rust 1.98.1,
Windows 26200.9711 / 25H2. The earlier 477/43/4 run was elevated; a probe
confirmed `Elevated=True`, `OwnerMatchesUser=False`,
`OwnerIsAdministrators=True`. A standard-user rerun passed without changing
ownership checks or assertions. Elevated-launch compatibility and the
development host's TCP-state anomaly remain open; neither is fixed by the
standard-user pass. The four ignored native fixtures, integration test
executables, and native owner UI are not covered by this verification.

The owned HTTP failures have been narrowed to native TCP-state attestation.
A standalone Win32/C# probe (no Cryptex, HTTP, child job, or credentials) completed
round trips before/after all 18 snapshots, yet 6 IPv4 samples returned exact
server tuples in state 0 or 6 instead of established state 5 on build
26300.9818. Root cause inside the host remains unproven. Added the precise
`PeerNotEstablished` rejection and IPv4/IPv6 regression coverage; ownership and
established-state requirements are unchanged. The HTTP failures are not fixed.
Fresh daemon library run: **516 passed / 4 failed / 4 ignored**; all five TCP
parser tests passed. Strict release all-target daemon Clippy, format, and probe
self-tests pass. The owner has now supplied the second Windows/VM probe result
from pinned source `13e8d02`: **26200.9711 / 25H2; 60 samples, all round trips
passed, zero mismatches, state 5 only**. The corresponding development-host
ten-iteration probe had 3/60 mismatches. This supports a host/build-dependent
anomaly but does not establish its underlying cause.

**Second-host HTTP follow-up:** the owner reports **8/8 owned-process HTTP
tests passed** on the same Windows/VM, source archive pinned to `a5f859a`,
Rust 1.98.1, using the unchanged serial HTTP-group command. This is a scoped
owner-reported PASS; a full successful HTTP-group transcript was not supplied.
The later standard-user daemon result is recorded above; native owner UI
verification is still pending. The development-host
failures remain open; no state/job check or HTTP assertion was relaxed.

Evidence and reproducible commands:
[`m8-windows-tcp-state-diagnostic-20261029.md`](../evidence/m8/m8-windows-tcp-state-diagnostic-20261029.md).
Next: check Node/pnpm prerequisites on the second Windows/VM and prepare the
Windows owner UI gate using a standard-user session and a fresh synthetic profile
below. No installer, native owner UI, macOS parity, production transport,
security review, or M8 completion is claimed.

## Earlier owner-operation snapshot — historical

**M8 Windows owner-security confirmation, append, and recovery (2026-10-29):** After owner acceptance of ADR-0019 §2, the daemon now derives immutable restriction/self-retirement snapshots from complete verified trust history, journals the exact snapshot before approval, and requires a same-connection, single-use confirmation capability. Windows revalidates the live connected Desktop process before confirmation. Confirmed operations append under canonical locks; startup reconciles an append-unknown owner operation against exact canonical history and anchors it without duplicate append. Desktop now presents the full snapshot for explicit owner review. Synthetic regression tests cover self-retirement confirmation/recovery and peer-admission followed by restriction. Verification: daemon library **517 passed / 0 failed / 4 ignored**; the separate binary test executable was blocked by Windows Application Control (OS 4551), so binary tests are not claimed. Strict daemon/Desktop release Clippy, fmt, TypeScript/Vite build, `tauri:package-smoke` (no bundle), and `git diff --check` pass. Built Desktop SHA-256 `EA86B015F747BAF9D63FC366923958D260F9AE38E26381902E6ADA90C0A62A32`; daemon sidecar SHA-256 `B05039386DE7F234021FBBEC2EF3DD27A6BC8354EA5D38FCF80D864BA690929B`. This is not an installer or interactive native Windows pass: UI, nonzero cutoff boundaries, production Syncthing-to-publication flow, macOS authenticated peer identity/parity, and independent review remain open. No Actions, commit, push, or M8 completion.

## Published checkpoint summary — 2026-10-29

Current milestone: **M8, in progress and not complete**. Branch:
`codex/m8-device-trust`. See the root [README](../README.md#implementation-status--2026-10-29)
for a compact implemented/open matrix. This checkpoint packages the accumulated
Windows trust, owner-approval, import-validation, and recovery work for review;
it does not authorize production activation or claim cross-platform readiness.

**Fresh checkpoint checks:** daemon library 515 passed / 3 failed / 4 ignored;
isolated owned-process HTTP rerun 4 passed / 4 failed, with
`Endpoint(PeerNotOwned)`. This supersedes earlier green-test summaries for
current readiness. Daemon binary tests now pass 12/12; workspace all-target
release Clippy, format, frontend build, and 18 Desktop policy tests pass.
See [publication verification](../evidence/m8/m8-checkpoint-publication-20261029.md).

**Exact next task:** investigate the owned-process HTTP test failures without
weakening ownership validation. Then run the Windows native Desktop owner flow against a fresh
synthetic profile outside the source repository. Verify confirmation, cancel,
denial, retry/error behavior, nonzero exact cutoff boundaries, and recovery
without duplicate canonical append. The latest package smoke built the app and
sidecar but did not launch the interactive owner dialog.

**Remaining M8 gates:** production Syncthing-to-staging/publication and safe
replay; remaining marker/directory durability and crash-boundary evidence;
macOS authenticated peer identity and owner-operation parity; independent
security review; and the remaining milestone/Definition-of-Done checks.
The macOS owner approval route currently fails closed.

**Reading the history:** dated entries below (and the latest entry above this
heading) retain their original verification scope. "No commit, push, Actions"
describes the individual evidence run. The user has now requested committing
and pushing the accumulated checkpoint; that publication does not complete M8
or turn historical test results into new verification.

**M8 Windows owner-operation snapshot construction (2026-10-29):** The
lock-scoped, complete canonical trust scan now retains its verified local
approval roster for building immutable owner-visible restriction and
self-retirement snapshots. Restriction requests require a currently trusted,
directly approved peer; bind the peer event key and both verified transport
IDs; and accept only an exact canonical cutoff boundary no later than the
current effective cutoff. Self-retirement binds the exact latest local
sequence/hash (or the defined empty-prefix sentinel). Both builders reject a
reused canonical security transaction ID and validate the closed snapshot
digest. This produces request data only: it does not issue a confirmation
capability, sign an event, or append canonical bytes. Daemon library **515
passed / 0 failed / 4 ignored**, binary **12/12**, strict release Clippy, fmt
check, and `git diff --check` pass. Native owner UI/IPC integration, operation
signing/locked append/recovery, Windows native release proof, macOS parity, and
independent security review remain open. No commit, push, Actions, or M8
completion.

**M8 Windows owner-operation confirmation binding (2026-10-29):** Following
the owner's acceptance of the §2 bootstrap model, the approval snapshot and
canonical digest now include the restriction reason (`retired`/`compromised`)
and the self-retirement last-authorized sequence/content hash. The bounded
journal validates and persists these immutable payloads before confirmation;
the restriction path is no longer rejected solely because its reason was
unbound. This does not enable canonical append: locked-history revalidation,
operation-specific event construction/append/recovery, native Windows UI, and
macOS parity remain open. Full verification passed: daemon library **514 passed
/ 0 failed / 4 ignored**, binary **12/12**, strict release Clippy, format check,
and `git diff --check`. Windows Application Control initially blocked one test
launch (OS 4551); a subsequent full run completed without changing security
settings. No commit, push, Actions, or M8 completion.

**M8 Windows marker durability contract check (2026-10-29):** Confirmed the
existing no-replace `MoveFileExW` adapter uses `MOVEFILE_WRITE_THROUGH`. The
Microsoft API documentation says the call returns only after the file is
actually moved on disk; it separately guarantees flushing for copy/delete
moves. The directory-handle `FlushFileBuffers` call is not treated as the
documented guarantee. This host's test temp volume is NTFS (C:), and existing
cross-process interruption tests exercise the canonical move and committed
marker temp/final boundaries there. This is process-crash evidence only: no
power-loss test or other filesystem is covered, so Windows committed
durability remains a release gate. Source:
https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw.
No code, commit, push, or Actions in this documentation check.

**M8 Windows post-import trust/cutoff gate (2026-10-29):** Staged and
prepared-recovery verification now rebuilds the complete authenticated
security-history union from the locked canonical frames plus exact verified
staged record bodies. The local-anchor verifier and ADR trust reducer run over
that union before a policy token is returned. The gate checks every existing
and candidate device remains in the resulting trusted cutoff set, admits
staged v1 content only when it falls within the post-import cutoff, rejects
unintegrated content versions, reduces the resulting cutoff-aware content, and
holds unresolved or unmaterialized assertions. V3 records are evaluated as
policy metadata; they never enter materialized content. Regression coverage
verifies an imported self-retirement lowers the effective cutoff in both the
normal batch and prepared-recovery paths while its control event stays out of
content state. `cargo test -p cryptexd --lib --bins`: library **513 passed / 0
failed / 4 ignored**, binary **12/12**; strict release Clippy, fmt check, and
`git diff --check` pass. This is a validation token only; it does not activate
transport admission or authorize production publication/replay. Windows
native directory-durability proof, real transport-to-staging wiring, macOS
parity, and independent security review remain open. No commit, push, Actions,
or M8 completion.

**M8 Windows read-only recovery regression fix (2026-10-29):** The prepared-
recovery startup verifier now uses the projection crate's read-only schema
validator through the shared retrieval safety gate. The writable FTS capability
probe attempted temporary writes on a read-only connection and incorrectly
held valid materialization as stale. Regression coverage reopens a persisted
synthetic projection read-only, verifies the matching visible destination, and
confirms the transaction still remains held with its evidence preserved. Daemon
library **513/513** and binary **12/12** tests pass; strict daemon/projection
release Clippy, fmt, and diff checks pass. The lifecycle integration fixtures
and projection test executable were blocked by Windows Application Control
(OS error 4551); those suites are not claimed on this run. No publication or
cleanup was enabled. No commit, push, Actions, or M8 completion.

**M8 Windows prepared-recovery startup policy gate (2026-10-29):** Startup now
invokes the lock-bearing verifier only when a prepared transaction has one or
more visible canonical destinations. It opens the existing projection DB
read-only and loads existing Brain/device trust state without creating or
repairing either. The verifier reconstructs the pre-import M3/M4 and
local-trust snapshot from matching manifest-bound tail files, then checks each
source identity, digest, signature, Brain/device binding, sequence/hash-chain,
trust, and cutoff. A successful check still ends in a typed hold: it does not
publish, write a commit marker, or clean up. Missing local policy state and
imported v3 trust-control effects also fail closed. Evidence:
`evidence/m8/m8-windows-prepared-recovery-policy-verifier-20261029.md`.
Windows marker durability, native interruption coverage, transport wiring,
macOS parity, and independent security review remain open. No commit, push,
Actions, or M8 completion.

**M8 Windows unmarked destination classification (2026-10-29):** Startup
recovery now accepts only a manifest-listed staged file or its exact canonical
destination for each segment, and validates identity, length, and SHA-256.
When no commit marker exists, matching partial and complete destination sets
are explicitly classified and held; they are never resumed without the full
locked trust/cutoff and `RetrievalSafetyState` gate. Missing, mismatched, or
duplicate staged/destination copies also hold with evidence preserved. The
focused regression exercises a moved subset, all moved destinations, then
restores the synthetic staging state to test the pre-publication abort. It
does not enable replay, transport publication, or M8 completion. Evidence:
`evidence/m8/m8-windows-unmarked-destination-classification-20261029.md`.
No commit, push, Actions, or M8 completion.

**M8 Windows committed-temp recovery (2026-10-29):** Startup now completes a
recognized temporary committed marker only after its exact prepared-manifest
digest, remaining staged files, and every canonical destination have been
revalidated under canonical inventory/device locks. It moves the marker with
no-replace semantics, syncs and reads it back, then performs the same
transaction-local cleanup as a final marker. A missing or mismatched
destination, bad companion, existing final marker, or unrecognized journal
entry remains held with evidence preserved. Cross-process interruption tests
cover the recovery marker move and post-sync boundary; they are not power-loss
or Windows directory-durability proof. Partial-publication replay and the
production transport-to-publication path remain disabled. Evidence:
`evidence/m8/m8-windows-committed-temp-recovery-20261029.md`. No commit, push,
Actions, or M8 completion.

**M8 Windows committed-final cleanup (2026-10-29):** Startup recovery now
finishes cleanup only for a valid final committed marker whose exact prepared
manifest companion, any remaining staged files, and every canonical
destination pass identity/length/SHA-256 checks under the canonical inventory
and device locks. It removes only that transaction's staging, manifest, and
marker, in that order; canonical files are untouched. A temporary marker is
now completed only through the additional gate described above. Missing or
mismatched destinations, incomplete companions, and conflicting journal
states still hold and preserve available evidence. Windows process-crash tests
do not prove power-loss or directory durability. The production
transport-to-publication path remains disabled. Evidence:
`evidence/m8/m8-windows-final-marker-cleanup-20261029.md`. No commit, push,
Actions, or M8 completion.

**M8 Windows regression verification (2026-10-29):** Re-ran the complete
`cryptexd` package on the Windows host: **507 library tests, 12 binary tests,
9 lifecycle tests, 3 supervisor tests, and 2 rollback tests passed**. The
workspace library suite excluding `cryptex-desktop` passed **764 tests with 4
ignored**. Strict release Clippy for all `cryptexd` targets, workspace format
check, and `git diff --check` pass. Downloaded the official pinned Syncthing
2.1.5 Windows amd64 archive, verified its executable against the repository's
SHA-256 pin, then passed all four explicit loopback-only native fixtures:
owned lifecycle, Windows Credential Manager profile, package ownership, and
native provisioning/reopen/collision recovery. The synthetic Credential
Manager entry was deleted and no native process remained. The first lifecycle attempt collided with
an already-running synthetic `m8-win-native` pilot holding the global Windows
IPC endpoint; that exact pilot was stopped through its runtime stop file and
reported `clean shutdown released instance lock`, after which all lifecycle
tests passed. This is host/native-fixture evidence only: it does not exercise a
peer transport or complete M8. The Windows Desktop UI build, capture/search
policy tests, Tauri Rust check, and unbundled Tauri release package smoke also
pass. The smoke produced a release Desktop executable and pinned daemon
sidecar; it did not run a native owner dialog. Evidence:
`evidence/m8/m8-windows-regression-verification-20261029.md`. No commit, push,
or Actions.

**M8 Windows owner-operation journal gate (2026-10-29):** Added durable
`owner_pending`/`owner_approved` journal states for issuer-side security
operations. Approval can advance only against the exact digest-checked
snapshot already stored by the daemon. At that point non-admission append was
closed because the snapshot omitted restriction reason and self-retirement
boundary. This owner decision was resolved on 2026-10-29: the accepted §2
bootstrap implementation basis now binds those fields in ADR-0019 §2.4 and the
journal snapshot/digest. Full daemon verification after that change is recorded
in `evidence/m8/m8-windows-owner-operation-journal-gate-20261029.md`.
Operation-specific locked-history revalidation, event append/recovery, native
Windows interaction, and macOS parity remain open; no native owner operation
or M8 completion is claimed.

**M8 Windows v3 restriction/self-retirement signing primitives (2026-10-29):**
Added typed `security.device_restricted` and `security.device_self_retired`
event signers using the existing v3 envelope, canonical hash/signature path,
and strict reparse before bytes are returned. Restriction signing binds the
target public key to its signing-key ID; self-retirement rejects a cutoff at
or after its control event. The helpers are byte constructors only: owner
approval, exact target/cutoff verification against canonical history, journal,
sequence reservation, lock-bearing append/replay, and activation remain
unimplemented. `cryptex-events` tests **86/86** and compile-fail doctest **1/1**
pass; strict crate Clippy and diff check pass. Evidence:
`evidence/m8/m8-windows-security-event-signing-primitives-20261029.md`. Next:
wire one owner-confirmed restriction operation through a frozen approval
session and lock-scoped canonical append/reconciliation, or hold if the
existing ADR does not fully define that transition. No native package, Mac
parity, Actions, commit, push, or M8 completion is claimed.

**M8 Windows §2 journaled message-exchange/UI slice (2026-10-29):** Added
authenticated Desktop IPC and Tauri commands to create/resume a signed offer
and verify an incoming offer before generating/resuming the signed proof.
Transaction IDs come from the OS CSPRNG before the request so an interrupted
Desktop request can be retried against the same journal key. The daemon
persists the exact offer before releasing it and persists the received offer
before generating/releasing proof bytes. Added a copy/paste exchange panel
before the existing immutable owner-review and local-trust flow. The responder
can explicitly join using the exact canonical public Brain identity copied
from the initiator; a mismatched existing Brain or unbound local canonical
state is refused without replacement. Exact metadata is preserved and the new
DeviceId/key enrollment resumes idempotently. The initiating Desktop can
display the canonical public Brain identity for transfer. Synthetic tests
cover exact request shape, offer/proof verification, journal-before-response,
exact replay, identity read/join, and refusal of mismatched or occupied state
(**11 focused IPC tests pass; cryptexd library: 506 passed, 0 failed, 4
ignored**, including active/ready transport identity gating and rejection of
missing/mismatched adapter identity); `cryptex-events`
**84/84** plus doc test **1/1**, `cryptexd`
Clippy, Desktop Clippy, frontend build, and diff check pass. Pairing no longer
accepts a manually entered transport DeviceId. Desktop reads the ID only from
the daemon's active managed worker status, and the daemon rechecks it against
the current worker before signing an offer/proof. The worker publishes it only
after the pinned Syncthing instance answers authenticated local REST status
with the same ID as its validated private profile receipt. Inactive, stopping,
unavailable, or mismatched adapter state fails closed. No transport or sync
starts from pairing. Windows packaged/native review, macOS peer-identity/parity,
transport-to-staging wiring, publication replay/durability, and independent
security review remain open. The active worker is currently reachable only
through the development `lifecycle hold` harness; normal Desktop startup does
not yet own/start the managed worker. Thus the new identity path fails closed
until product lifecycle wiring exists, and does not establish an interactive
pairing pass. M8 is not complete.

**M8 Windows §2 exchange-message construction (2026-10-29):** Added public
`cryptex-events` builders for the initiator `offer` and responder `proof`.
The proof builder accepts only a cryptographically verified signed offer and
copies its exact Brain, transaction, initiator identity/transport, and message
hash into the response. Both builders bind the local DeviceId and transport
association, sign the exact protocol domain/JCS bytes, and re-parse their output
through the bounded closed-schema signature verifier before returning bytes.
The daemon caller must still obtain fresh transaction/nonces from the OS CSPRNG
and durably journal exact messages before exposing them; no IPC/UI exchange
creation, transport activation, or sync was added. Windows verification:
`cryptex-events` **84/84**, compile-fail doc test **1/1**, strict crate Clippy,
workspace fmt check, and `git diff --check` PASS. Next Windows §2 work is the
daemon-owned journaled exchange state and authenticated Desktop handoff; the
macOS completion gate, Syncthing/import wiring, publication replay/durability,
native packaged review, and independent security review remain open. M8 is
not complete.

**M8 Windows bootstrap-to-local-trust flow (2026-10-29):** The authenticated
Desktop completion IPC and Tauri review action now connect the owner-approved
bootstrap transcript to local trust creation. Completion requires the exact
four-message transcript to match the journaled owner approval and frozen peer
snapshot, verifies the transcript, scans canonical history and trust policy
under the writer locks, and appends a locally authored
`security.peer_approved` v3 record. The stable operation digest makes exact
replay idempotent; an explicit owner retry after a complete scan may reserve a
new sequence only when both event and transaction IDs are absent. Startup
reconciliation checks `append_unknown` and `anchored` entries before transport
or IPC, accepts only an exact canonical match, and never appends. Missing,
mismatched, or unverifiable records hold startup and preserve evidence. A
negative IPC test proves a transcript without the journaled owner approval
cannot create trust. The Desktop UI presents the peer identity/fingerprint and
transport association before the final local-trust action; this action does
not start transport or sync. Current Windows checks: `cryptexd --lib` **495
passed / 0 failed / 4 ignored**, events **82/82**, knowledge **36/36**, strict
Clippy, format, `git diff --check`, frontend build, and daemon binary check
pass. Still open: native Windows packaged UX/interruption evidence, macOS
native parity and peer-identity transport, transport exchange and importer
wiring, publication replay/durability, and independent security review. M8 is
not complete. A read-only Cursor security review found the macOS completion
IPC still lacks process-instance binding; `bootstrap.approval.complete` now
fails closed on macOS with `platform_peer_identity_unavailable` until that
platform gate is implemented. Windows positive and negative completion IPC
tests, daemon Clippy, formatting, and diff checks pass after the guard change;
the macOS build/native test has not been run.

**M8 scope boundary:** On 2026-10-29 the owner accepted ADR-0019 §§2–4 as the
Windows implementation basis and authorized Windows implementation. The
existing §5.1.1 Windows flow approval remains in force. This does not accept
ADR-0019 as a whole or waive independent security review. The explicit
Windows owner-approved local-trust path is implemented under the accepted
protocol/reducer rules; it does not enable peer transport or publication. A
local-anchor trust/cutoff validator now checks staged batches. An internal
no-replace publisher consumes its M3/M4 lock-bearing token in synthetic tests,
but managed transport does not call it and publication recovery/durability
gates remain open. Native platform evidence and independent review are still
required before claiming M8 readiness.

**M8 Windows bootstrap/trust follow-up (2026-10-29):** Added a 64 KiB
bounded bootstrap JSON parser and a closed four-message transcript verifier
covering signatures, identity/key bindings, role/order, hashes, and transport
associations. A valid locally signed `security.peer_approved` anchor derives
the peer key, fingerprint, and transport binding; imported peer anchors do not
grant local trust. Policy issuer authorization now runs after the complete
local roster is derived, independent of cross-device sort order. Added a
read-only lock-scoped canonical policy scanner that first authenticates the
local stream with the supplied local key, then verifies the complete snapshot
only with that key and bindings from local anchors. Unknown device records or
unapproved device directories fail closed. Events tests **79/79**, trust
reducer tests **13/13**, and the focused scanner regression pass. The scanner
does not load the key from the OS secure store itself and is not wired to
retrieval/import authorization. The R1 authenticated approval-session
capability is now connected to authenticated Windows Desktop IPC and an
explicit Desktop review/approve/cancel panel. The daemon freezes and returns
the display snapshot, requires the same connection and unchanged signed prefix
for confirm/cancel, and journals the local approval signature before returning
it to Desktop. At this checkpoint, a successful signature was only a signed
bootstrap message. The later completion/anchor integration is recorded in the
current status at the top of this handoff; transport activation remains off.
See `evidence/m8/m8-windows-bootstrap-approval-ipc-20261029.md`.

**M8 Windows v3 anchor preparation follow-up (2026-10-29):** The versioned
reader now strictly authenticates schema-v3 security records and validates
their per-device chain position across v1/v2/v3 boundaries. The knowledge
materializer excludes control records from memory-note projection, while the
trust-history/reducer and cutoff-aware retrieval path consume authenticated v3
policy records. Added a signer primitive that can form a local
`security.peer_approved` record only from a fully verified four-message
transcript and the matching participant key; this produces bytes only. The
bootstrap journal can now persist the exact completed transcript plus stable
append-attempt metadata. Its `anchored` cache transition has no direct state
machine path and remains unavailable without a matching recorded candidate;
canonical history is still authoritative. Cross-version writer-chain and
journal regression tests pass. At this checkpoint, production anchor
append/reconciliation and the local-trust action were not yet enabled; the
later completion integration is recorded at the top of this handoff.
Transport, publication recovery/durability, and native review gates remain
open.

**M8 verified-transcript type hardening (2026-10-29):** Made every field of
`VerifiedBootstrapTranscript` private and exposed read-only accessors, so
callers cannot fabricate transcript verification evidence by constructing the
public struct. Added a compile-fail doctest for external construction and
updated the daemon trust/journal consumers to use accessors. This tightens the
proof boundary only; it does not append trust anchors or activate peers.

**M8 cutoff-aware content view foundation (2026-10-29):** Added a helper on
`VerifiedSecurityHistory` that yields only authenticated v1/v2 records whose
device has a supplied effective cutoff and whose sequence is at or below that
cutoff. It never yields v3 policy records; absent cutoffs exclude a device.
The history can now be converted into the existing verified content-history
type, and `cryptex-knowledge::reduce_verified_security_history` runs the M4
reducer only over that filtered set. The lock-scoped trust-policy snapshot
derives its cutoff map from the local-anchor reducer before invoking this
content reducer. Local secure-store key resolvers now expose a read-only
`LocalTrustAnchor` to retrieval scanning. `RetrievalSafetyState` uses that
anchor to enumerate and lock the complete canonical inventory, verify peer
keys only from local approval transcripts, derive effective cutoffs, and
construct a filtered content snapshot while retaining all locks through M3/M4
comparison and query. A missing/unapproved inventory member, invalid trust
history, unresolved dependency, or stale projection holds retrieval. V2
semantics remain held by the existing retrieval gate. The generic M4 rebuild
and import-tip scanner remain unchanged; this does not enable publication or
transport. Events **80/80**, knowledge **36/36**, and the focused local-anchor
guarded-search regression pass. Windows native Desktop interaction and
independent security review remain open.

**M8 Windows local-policy import validation (2026-10-29):** Added a separate
staged-batch validation entry point that scans the complete canonical inventory
under inventory/device locks, derives peer keys only from locally signed
approval transcripts, rejects devices outside the reduced trust set, derives
the candidate starting tip from the authenticated full history, and rejects
both current tips and staged segment ends beyond effective cutoffs. The
lock-scoped snapshot and derived resolver cannot be supplied by the transport
caller. Import validation now consumes a successful M3/M4
`RetrievalSafetyState` and returns an unforgeable `ImportPolicyVerifiedBatch`
that retains its exact canonical snapshot locks. For a first-stream candidate,
the scanner checks locally reduced trust before creating a missing device
directory, then acquires the full sorted device-lock set under inventory lock
and rescans; an unapproved candidate does not create canonical state. Windows
regressions cover the unapproved-device refusal, a contiguous signed approved
segment through M3/M4 safety binding, lock retention against another writer,
and inventory-guarded first-stream lock creation. While the token owns those
locks, a read-only publication preflight rechecks staged identity/hash, rejects
canonical file-ID aliases, and refuses an occupied sealed destination or
matching `.open` path. A test-only call path then moves exact staged bytes with
no-replace placement, sync/read-backs each destination, and writes/reads back
the committed marker while retaining that token. Marker recovery now accepts
the ADR's moved-source state only when the corresponding destination hash
matches. Cross-process interruption tests cover after a canonical move, after
marker-temp sync, and after final-marker placement/read-back. Startup remains
held at the partial-move and marker-temp boundaries; after a valid final marker
and complete matching destinations, it now cleans only that committed
transaction's staging, manifest, and marker. Windows-host full daemon tests
pass with 490 library, 12 binary, 9 lifecycle, 3 supervisor, and 2 update
tests (4 ignored); strict release Clippy, fmt, and `git diff --check` pass.
The publisher remains unconnected to managed transport or a production IPC
path. Recovery still does not finish partial publication or complete marker
temps. Windows directory durability is not established by these process-crash
tests. See `evidence/m8/m8-windows-local-policy-import-validation-20261029.md`
and `evidence/m8/m8-windows-final-marker-cleanup-20261029.md`.

**M8 Windows approval IPC/UI slice (2026-10-29):** Loopback IPC tests cover
strict role/capability/request shape, rejection before transcript parsing for
unauthenticated peers, a synthetic prepare→frozen snapshot→confirm→journaled
signature round trip, verification of the returned signed message, and
cancellation without signature generation. Full daemon library tests pass
**485 / 0 failed / 4 ignored**; Desktop frontend
build and Windows Desktop `cargo check` pass. Native owner-dialog interaction
remains unverified: the isolated packaged pilot launch was blocked by Windows
Code Integrity event 3077 (Enterprise signing level); no policy change or
bypass was attempted. The panel accepts compact signed transcript messages for
review; it does not exchange messages with Syncthing or another device.
Transport identity remains a transcript association label, not proof of a
live peer. The operator must compare the full IDs and fingerprints over a
separate trusted channel. This is not M8 completion: canonical anchor
append/idempotency, startup recovery/reconciliation, trusted importer
publication, transport activation, independent security review, Windows native
interaction and macOS parity remain open.

**M8 Windows bootstrap journal foundation (2026-10-29):** Added a bounded,
closed-schema daemon journal model for the exact immutable owner snapshot,
transcript bindings, transaction states, stable event IDs, and append-attempt
metadata. Writes are serialized under a process lock, use unique same-directory
temporary files, sync before replacement, reopen/sync the destination, and use
the Windows replace/write-through primitive. Reads reject duplicate JSON keys,
invalid schemas, duplicate transaction/event IDs, linked/non-file entries, and
unexpected leftover files while preserving evidence. Transcript verification
now exposes both approval-message hashes so the journal can check the exact
R1 role-specific preceding-message binding. Focused journal tests **9/9** and
bootstrap parser/transcript tests **4/4** pass. Full Windows library suites
also pass: events **78/78** and cryptexd **467 passed / 0 failed / 4 ignored**.
Release Clippy, format, and diff checks pass. The later Windows approval IPC/UI
slice consumes this journal for local signature release; canonical append,
startup reconciliation, peer trust activation, and publication remain
disabled. Windows process-interruption/durability probing and macOS native
parity remain open. Evidence:
`evidence/m8/m8-windows-bootstrap-journal-foundation-20261029.md`.

**M8 Windows R1 approval-session capability foundation (2026-10-29):** Added
an ephemeral, process-memory-only registry for unguessable 256-bit approval
capabilities. A grant binds the frozen approval snapshot digest to an opaque
per-connection identity; its display snapshot is derived from a verified
bootstrap prefix. Wrong-connection use, digest mismatch, cancellation,
disconnect invalidation, replay, and expiry after five minutes of daemon
monotonic time fail closed. Confirmation consumes the capability before
checking the full request binding and invoking the signer. A read-only event
signing-key loader now refuses missing state without creating directories,
locks, or key material; focused tests cover existing and missing state.
Capacity is bounded globally and per connection, and Debug output redacts the
bearer token. Windows daemon library tests **481 passed / 0 failed / 4 ignored**,
focused approval/key tests **14/14 pass**, release Clippy, fmt, and diff check
pass. The journal records verified initiator/responder prefixes before
approval, accepts exact byte-identical replay, and rejects altered bytes for
the same transaction. A journaled confirmation helper now consumes the grant,
invokes its signer callback, durably records the signed message, and only then
returns it; tests cover initiator signature release and responder transcript
completion.
At the time this foundation note was written, the helper was not yet called by
production IPC. The later integration is recorded in
`evidence/m8/m8-windows-bootstrap-approval-ipc-20261029.md`. Canonical append,
startup reconciliation, peer trust activation, and publication remain
disabled. Evidence:
`evidence/m8/m8-windows-approval-session-capability-20261029.md`.

**M8 bootstrap approval-prefix verification and signer primitive (2026-10-29):**
Added role-specific verification of the signed offer/proof prefix (plus the
initiator approval for the responder role), deriving the exact Brain,
transaction, local/peer full key identities, transport associations, agreement
hash, and required preceding-message hash. A signer emits the role-specific
approval only when the supplied private key matches the verified local full
key and key ID. The daemon converts this verified context into the exact owner
snapshot, compares every binding during confirmation, consumes the capability
before signing, and burns it on mismatch or signing failure. Tests recreate
both approval messages byte-for-byte from the existing complete transcript,
verify a completed transcript, and reject altered prefixes, snapshot mismatch,
and mismatched keys. Events library **79/79** and daemon library **474 passed /
0 failed / 4 ignored**; release Clippy, fmt, and diff check pass. This daemon
method is not called by IPC or Desktop UI. No product approval/signature, journal
transition, or canonical append is enabled; no trust is activated.
Evidence: `evidence/m8/m8-windows-bootstrap-approval-signing-20261029.md`.

**M8 Windows trust/cutoff reducer and verified-history bridge (2026-10-29):**
Added a deterministic SCC/cutoff reducer plus a v1/v2/v3 verified-history
builder. The history reader sorts by device ID and sequence, validates the
cross-version chain independent of arrival order, rejects unsupported
semantics/forks/conflicting event IDs, and supports cancellation without
returning partial history. The builder admits reducer restrictions only from
the local device or caller-supplied directly approved peers, binds each
nonzero cutoff to the exact target record hash and signing-key ID, binds
self-retirement to the immediately preceding record, rejects later records
after retirement, and rejects duplicate security transaction IDs per author
stream. Tests cover a v1→v2→v3 chain, arrival reordering, duplicate sequence,
exact cutoff mismatch, terminal retirement, and cancellation; reducer vectors
still cover all seven required SCC/cutoff cases and fail-closed histories.
Earlier results: events library **74/74** and daemon trust-reducer tests
**11/11** before the bootstrap follow-up. This bridge trusts its caller to provide an exhaustive lock-scoped
canonical snapshot and a locally trusted key resolver; it does not verify
approval transcripts or derive the direct-approval roster. It is not wired to
the production canonical scanner, `RetrievalSafetyState`, content filtering,
publication, or transport. It cannot authorize retrieval or import yet.
Independent review and native platform gates still precede activation.
Evidence: `evidence/m8/m8-windows-trust-history-bridge-20261029.md`.

The existing lock-scoped M4 canonical scan now also verifies the exact scanned
v1/v2 frame set through the M8 security-history reader and retains that
authenticated history in its snapshot. Its reduction helper now takes the
local DeviceId/key and derives local peer bindings from locally anchored
approval transcripts; it does not yet resolve the full canonical union with
those bindings. A synthetic scanner test covers a single local v1 stream and
confirms local-only trust. The existing M4 versioned reader still rejects v3
before this bridge can consume a v3 canonical stream; it is not called by
retrieval or import paths. This adds a second verification pass for v1/v2 and
is not an activation or M8 completion claim.

**M8 Windows strict v3 event-reader slice (2026-10-29):** Added an isolated
strict reader for the three proposed `security.*` event payloads. It enforces
the closed envelope/payload fields, record limits, duplicate-key rejection,
canonical lowercase IDs/hex, internal classification, target key-ID/full-key
binding, canonical JCS, the v3 content-hash/signature domains, and Ed25519
verification. `VersionedChainVerifier` has an explicit v3 advance method for
records produced by that reader. The ordinary dispatcher intentionally still
classifies v3 as unverifiable, keeping existing retrieval consumers fail-closed.
Ten focused reader/chain tests and the full events library suite passed at
**71/71** before the verified-history bridge added three more events tests.
This slice is superseded by the current bridge summary above for test totals and
remaining trust/history limitations. The ordinary dispatcher still holds v3,
so existing retrieval callers cannot accidentally treat these records as
memory. Evidence:
`evidence/m8/m8-windows-security-v3-reader-20261029.md`.

**M8 Windows transport-path batch staging (2026-10-25):** Added
`ImportStagingSession::stage_batch_from_transport_paths`, which derives each
candidate's `DeviceId` and segment index from the exact canonical
`<lowercase-device-id-hex>/<20-digit-index>.seg` relative path, rejects
noncanonical paths and duplicate identities before staging, and copies a batch
in deterministic DeviceId/index order through the existing handle-checked,
bounded staging path. The focused `import_staging::tests` group passes **45/45**,
including a second-candidate failure check that confirms the first staged
snapshot is rolled back while the transport source remains retryable.
This remains a callable staging primitive: no Syncthing event/poller invokes it,
no trust/cutoff or `RetrievalSafetyState` gate is wired, and it grants no
canonical publication, marker completion, cleanup, or transport activation.
Evidence: `evidence/m8/m8-windows-import-path-batch-staging-20261025.md`.

**M8 Windows committed-marker parser/startup hold (2026-10-25):** Added the
closed v1 final/temp-marker parser with bounded no-follow reads, identity
recheck, canonical JSON/schema/UUID/hash/name validation. Startup recognizes
valid final and temporary markers, binds each to the exact prepared-manifest
digest, verifies the complete staged companion, then re-reads the marker and
manifest and rechecks the complete staging tree after acquiring canonical
inventory/device locks before classifying destinations as absent, matching, or
conflicting. Every marker state still returns a typed hold; no
marker completion, publication, or cleanup is enabled. Added subprocess
interruptions after marker-temp sync and final-marker placement; both restart
probes hold and preserve marker evidence. At the committed-marker checkpoint,
Windows verification was `import_staging::tests` **40/40**, daemon library
suite **440 passed / 0 failed / 4 ignored**. The later transport-batch rollback
regression brings the focused group to **45/45**; a full Windows
`cargo test -p cryptexd` refresh on 2026-10-29 passed: library **445 passed / 0
failed / 4 ignored**, binary **12/12**, lifecycle **9/9**, supervisor **3/3**,
rollback **2/2**. Strict release Clippy, fmt, and diff checks also pass on the
current tree. Tests include
marker/manifest replacement rejection and exact byte-preservation for a valid
committed marker with prepared companion, a valid temporary marker with
companion, malformed final and temporary markers with staging present, and
final+temporary conflict.
The earlier full `cargo test -p cryptexd` package run also passed: 12 binary, 9
lifecycle, 3 supervisor, and 2 rollback integration tests (in addition to the
440-test library suite; 4 ignored).
The non-Desktop workspace library suites passed **671/671** (4 ignored).
Full workspace tests are blocked by Windows Application Control 4551 on the
generated Tauri WebView2 macro DLL; excluding Desktop still blocks the generated
events schema-generator test binary. Security policy was left unchanged, so
Desktop/UI and generated-tool tests remain unverified here.
The prior 4551-blocked launch is superseded. `cargo check -p cryptexd --tests`
also passed earlier in this slice.
Evidence:
`evidence/m8/m8-windows-committed-marker-parser-20261025.md`.
Marker interruption probes also passed 2/2 on D:, E:, and F: (fixed NTFS
volumes); this is not power-loss or cross-filesystem proof.
No Actions, commit, push, trust, or transport activation.

**M8 Windows pinned peer-process liveness (2026-10-25):** Added a Windows-only
native test of the existing `DesktopProcessIdentity` retained process handle.
It accepts a synthetic child while the exact process object is live and rejects
the same numeric PID after that child exits. `ipc::tests` passed **47/47**; the
full daemon library suite passed **438 / 0 failed / 4 ignored**; strict release
Clippy, fmt, and diff checks passed. The test exercises the retained-handle
predicate together with kernel peer-PID retrieval over a real local named
pipe; it does not test actual PID reuse, executable replacement, or full
Desktop authorization. It is not macOS parity or trust evidence. Details:
`evidence/m8/m8-windows-pinned-peer-process-liveness-20261025.md`. No Actions,
commit, push, trust activation, or transport activation.

Independent Cursor read-only review of this parser/hold slice found no
Critical/High issue and confirmed that neither final nor temporary marker can
authorize a commit in the current code. It flagged stale ADR implementation
status wording and a stale recovery rustdoc comment; both are now corrected.
The review noted missing explicit preservation assertions for conflicting
marker combinations; regression tests cover malformed recognized marker bytes,
final/temp coexistence, manifest digest mismatch, and destination mismatch while
preserving evidence. Matching-subset classification and a temporary marker with
a full companion are also tested. Marker and manifest are re-read and compared,
and the complete staging tree is rechecked after canonical locks are acquired;
the current observation path still always returns a hold. Process-interruption
tests still do not exercise future replay/cleanup rows or prove power-loss
durability.
Replay and cleanup remain unimplemented. These are review-scope limitations,
not M8 readiness evidence.

**M8 macOS XPC peer-identity feasibility (2026-10-25):** Owner chose a bounded
synthetic XPC feasibility spike. Apple's public XPC APIs can enforce signed
peer-code requirements on messages (C API since macOS 12; NSXPC since macOS
13); endpoint forwarding and PID reuse mean signer/PID alone do not yet prove
the exact Desktop process instance. Spike design and negative-case matrix:
`evidence/m8/m8-macos-xpc-peer-identity-feasibility-20261025.md`; the bounded
Mac Cursor execution prompt is
`evidence/m8/m8-macos-xpc-cursor-execution-prompt-20261025.md`. Native Mac
build/run is **NOT RUN** on this Windows workstation. Production Unix-socket
IPC remains unchanged; any XPC migration would require a separate ADR-0007
amendment. No Actions, production IPC changes, trust activation, commit, or
push.

**M8 Windows committed-marker placement probe (2026-10-25):** Added a
Windows-only native test for the proposed marker temp/final naming under
`canonical/import-journal`. It syncs the marker temp, exercises no-replace
placement through `MoveFileExW(MOVEFILE_WRITE_THROUGH)`, flushes the journal
parent using the scoped directory-handle helper, and reads back exact bytes.
Focused placement and collision probes **1/1 each**, `import_staging::tests`
**21/21**, fmt, release Clippy
(`--lib --tests -D warnings`), and diff check PASS. The `%TEMP%` fixture ran on
the host's `C:` NTFS volume only. Microsoft Learn documents
the move flag but does not list `FlushFileBuffers` among functions supported
with directory handles; its success is observed on this host, not a documented
cross-filesystem guarantee. This is not a production marker writer,
process-crash/power-loss test, or portable durability proof. Evidence:
`evidence/m8/m8-windows-prepublication-crash-tests-20261025.md`. Trust and
transport remain disabled; no Actions, commit, or push.

**M8 publication/recovery contract clarification (2026-10-25):** Expanded
proposed ADR-0019 §5.1.1 with one deterministic marker/destination recovery
table. Any path that finishes uncommitted publication or creates/completes a
marker requires fresh locked trust/cutoff, `RetrievalSafetyState`, chain, and
destination revalidation before its next filesystem mutation. A valid final
marker is treated as the durable commit decision and only its manifest and
destination integrity are checked before cleanup; current retrieval policy is
still applied by the reducer. This is still a proposed contract, not accepted protocol
or implementation. Windows marker-directory durability and native crash tests,
publication/replay code, independent re-review, and the macOS peer-identity
architecture gate remain open. `git diff --check` passes. No source, tests,
Actions, commit, or push.

**M8 canonical writer lock coverage audit (2026-10-25):** Read-only search and
inspection at source HEAD `ae61603` found production segment append/seal/open
recovery calls centralized in `canonical_writer.rs`; daemon capture acquires
the inventory-first per-device lock before append, and multi-device locks are
ordered by DeviceId bytes. Projection and knowledge-materialization production
paths read under the corresponding canonical locks. Other direct segment
mutation matches are tests. Storage mutation APIs remain public, and importer
publication does not yet exist, so this narrows (but does not close) the shared
lock gate: any future importer publisher must join the same lock set and
revalidate while holding it. No publication, policy, replay, trust, or
cross-platform security claim follows from this audit. Details:
`evidence/m8/m8-canonical-writer-lock-coverage-audit-20261025.md`. No source
changes, tests, Actions, commit, or push.

**M8 Windows pre-publication crash-test slice (2026-10-25):** Added two
test-only subprocess interruption points for prepared-journal creation: after
the temporary manifest is synced, and after the prepared manifest plus parent
directory are synced. Parent recovery assertions check that the source remains
retryable and pre-publication staging/journal state is safely removed. The
initial test launch was blocked by Windows Application Control **4551**; a
subsequent local retry passed both interruption tests (**2/2**), the complete
`import_staging::tests` group (**19/19**), and the daemon library suite
(**417 passed, 0 failed, 4 ignored**). Strict daemon Clippy, fmt, test-target
check, and diff check pass. This is Windows process-crash coverage for the two
pre-publication states only, not power-loss proof or publication replay.
Details: `evidence/m8/m8-windows-prepublication-crash-tests-20261025.md`.
No Actions.

**M8 protocol addendum independent review (2026-10-25):** Owner authorized a
read-only Cursor review of R1–R5 and §5.1.1 at `ae61603`; recommendation was
**NOT READY**. The reviewer found the product direction consistent with the
Master Spec and accepted ADR-0019, but owner acceptance remains blocked on
concrete Windows/macOS connection-bound process credentials, independent
review of R1–R5 and the §4 cycle-hold vectors, and a complete publication,
commit-marker, and partial-replay decision matrix. The review also found stale
Windows durability/test-count wording; the proposal’s implementation status
has now been corrected to distinguish the scoped staging flush from pending
journal/publication guarantees. Findings:
`evidence/m8/m8-protocol-addendum-independent-review-20261025.md`. Trust and
transport remain disabled. No Actions ran.

**M8 protocol proposal follow-up review (2026-10-25):** Owner-authorized
read-only Cursor review of the updated, uncommitted proposal at Git HEAD
`ae61603` returned **NOT READY**. Critical: the macOS inherited pipe does not
itself authenticate parent-supplied audit-token bytes, and full-token equality
is not established as stable or sufficient for PID-reuse resistance. High:
Windows peer binding does not implement R1 approval sessions; pipe inheritance
and parent-liveness checks need a fail-closed contract; reducer and
`RetrievalSafetyState` must bind under canonical locks at publish and recovery
finish; subset replay must rerun the locked validation; marker ambiguity and
Windows marker-parent durability need explicit rows/evidence. These proposal
clarifications were recorded. Follow-up against Apple-published XNU confirms
the peer-token/task-token constants, warns audit-token representation can
change, and shows XNU's internal PID-version validation. A supported user-space
generation-validation API and token-bound code-signature check are still
unresolved, as are native tests. Proposal remains PROPOSED; no source,
trust, or transport activation. Details:
`evidence/m8/m8-protocol-addendum-independent-review-20261025.md`. No Actions.

**M8 peer-identity API follow-up (2026-10-25):** A further owner-authorized
read-only Cursor review of the revised peer-pin/recovery text returned
**NOT READY**. The macOS process-generation and token-bound code-sign API
composition is still unnamed; Windows marker-parent durability and
shared-mutator lock participation remain evidence gates. The proposal now
specifies intended pipe-end ownership and exact committed-marker names/schema;
these are target text only, not implementation evidence. Apple docs confirm
public audit-token-based Security APIs and XPC peer code-signing requirements,
but do not establish a public Unix-socket process-generation accessor. Apple's
XPC PID getter itself warns PID may go stale/reuse. XPC is only a feasibility
candidate and switching from accepted ADR-0007's `interprocess` sockets needs
explicit architecture approval; no migration was made. The proposal remains
PROPOSED, with trust and transport disabled. Details and sources:
`evidence/m8/m8-protocol-addendum-independent-review-20261025.md`. No source,
tests, Actions, commit, or push in this documentation follow-up.

**M8 Windows staging/journal directory-flush follow-up (2026-10-25):**
Replaced the Windows `sync_staged_parent` no-op with a fail-closed directory
handle flush scoped to M8 staging and prepared-journal placement/removal. The
opened parent must be an ordinary non-reparse directory; `FlushFileBuffers`
failure is returned. Windows `import_staging::tests` pass **16/16**, full
daemon library tests pass **414/414** (4 ignored), strict daemon Clippy, fmt,
and diff check pass. Evidence: `evidence/m8/m8-windows-staging-directory-flush-20261025.md`.
This verifies API operation on the current Windows filesystem, not power-loss
durability across all supported filesystems or journal/publication crash
points. Canonical publication/replay, approval/bootstrap, trust/cutoff policy,
macOS parity, and M8 completion remain open. No Actions ran.

**Independent-review prompt refreshed (2026-10-25):** Updated
`evidence/m8/m8-independent-security-review-prompt-20261025.md` to pin the
current source commit `304cb96` and review the Windows directory flush,
prepared-temp recovery, and newly added prepublication interruption tests. The
new crash tests remain NOT RUN due local Application Control 4551; the review
is required before changing M8 readiness/security claims.

**M8 independent security review + prepared-temp recovery (2026-10-25):** Owner supplied Cursor Security Review `b6f68bf7-93d5-4674-9286-e2221cdf5e52`, pinned to source `2fe8639`; recommendation **NOT READY**. Critical findings: no canonical publication/replay, no owner-approval IPC/bootstrap parser, and no trust/cutoff/`RetrievalSafetyState` validation in import. High findings: Windows parent-directory durability, macOS containment parity, publication-time alias rescan, transport-to-staging wiring, and serialization with appenders. Full supplied findings and scope boundary: `evidence/m8/m8-independent-security-review-followup-20261025.md`. Fixed the review's M1 crash window: an exact, bounded `.<uuid>.prepared.tmp` with a matching staging transaction is now treated as unprepared and safely cleaned so the still-present incoming source can be retried; malformed/unknown names and mixed temp/final journal states remain fail-closed. `cargo test -p cryptexd --lib import_staging::tests` passes **15/15**; full daemon library suite passes **413/413** (4 ignored); strict clippy, fmt, and diff check pass. This does not address C1–C4/H1–H5 or accept the protocol proposal. A follow-up review prompt now pins source `3e83307` in `evidence/m8/m8-independent-security-review-prompt-20261025.md`; trust activation remains gated on fresh review and the unresolved critical/high findings. No Actions ran.

**M8 identity-entry recovery gate (2026-10-25):** `cryptexd identity enroll-local` previously bypassed the import-recovery gate even though local enrollment inspects canonical device packs. It now takes the runtime single-instance lock and reconciles import state before identity setup; an unknown journal fails closed before Brain/local-device metadata can be created. The focused regression test passes, the complete daemon CLI suite passes **12/12**, strict daemon Clippy, fmt, and diff checks pass. This closes the CLI entry-point bypass only; publication/replay, trust and cutoff policy, approval IPC, durability, macOS parity, and independent review remain open. No Actions ran.

**M8 prepared-import recovery continuation (2026-10-25):** Added startup reconciliation for a canonical prepared manifest. It validates the closed/canonical manifest, requires it to cover the entire plain staging tree, then acquires canonical inventory and ordered device locks before checking destination absence, staged hashes, and aliases. Because canonical publication/replay is not implemented, an already visible destination remains a typed fail-closed conflict; otherwise startup removes the prepared manifest and staging tree. The recovery alias scan compares held `inventory.lock` and `writer.lock` identities through handle-relative opens with compatible sharing. Recovery tests prove a byte-identical visible destination remains a typed conflict until replay exists, and a tampered staged file preserves the manifest and staging evidence until the source is restored. This earlier checkpoint passed `import_staging::tests` **13/13** and full daemon library **411/411** (4 ignored). A Windows child-process lifecycle assertion now waits boundedly for nested-job teardown instead of sampling immediately after owner exit. fmt, test-target check, strict daemon Clippy, and diff check passed. No committed marker, partial-publication replay, or canonical move path; trust remains disabled. No Actions ran.

**M8 verified staged-batch continuation (2026-10-25):** Added `ImportStagingSession::verify_batch`, which deterministically orders staged candidates by bytewise DeviceId and segment index, requires exact per-device starting-tip coverage, enforces contiguous sealed segment indexes, and verifies the cross-segment sequence/hash chain on the exact staged snapshots. `verify_batch_against_canonical` now derives those tips from a complete versioned canonical scan while inventory/device locks remain held; the importer-only slot must be released first, while its unprepared transaction tree continues to reserve the staging slot. This remains a validation primitive and does not authorize publication or replace trust/cutoff/`RetrievalSafetyState` checks. The focused test is included in the Windows `import_staging::tests` result (**13/13**). fmt, daemon test-target check, strict Clippy, and diff check pass. Next: implement publication only after complete trust/cutoff validation and the required durability/review gates. Trust remains disabled; no Actions ran.

**M8 staging alias/startup gates (2026-10-25):** Incoming opened-file identity is now compared against ordinary files in every `canonical/events` device directory before staging. A same-file hard-link alias is rejected by identity even before the link-count check; symlinks/reparse points and unexpected canonical directory entries fail closed. This admission-time defense is not race-free publication proof; alias and destination checks must run again under canonical locks before any move. The synthetic hard-link alias test passes 1/1. The `pilot-fixture` canonical writer now also runs import recovery after taking the instance lock and before identity/event writes; its unknown-journal fail-closed test passes 1/1. fmt, `cargo check -p cryptexd --tests`, strict daemon Clippy, and diff check pass. The M8 importer still has no prepared/committed journal or canonical publication path. No Actions ran.

**M8 prepared import manifest/recovery slice (2026-10-25):** After releasing the importer-only slot and verifying the batch against the locked canonical snapshot, a session can write an immutable, no-replace, versioned canonical-JSON prepared manifest containing exact staged/destination relative paths, per-file lengths, and SHA-256 digests. Immediately before writing, it reopens and rehashes every staged file, checks object identity/metadata, and retains read handles through manifest placement. Closed-schema validation rejects unsupported versions, malformed IDs, non-canonical ordering, segment gaps, and configured count/byte-limit violations. The session will not stage or discard after preparation. Startup now parses the closed schema and reacquires inventory/device locks: if every staged hash matches and every destination is absent, it aborts the prepared transaction; if any destination is visible or staged state differs, it preserves the evidence and fails closed. There is still no committed marker, partial-publication replay, or canonical move path. Windows placement uses the existing `MOVEFILE_WRITE_THROUGH` adapter, but parent-directory durability and crash points are **not verified**; no durability PASS is claimed. fmt, test-target check, strict Clippy, and diff check pass. The updated batch/manifest recovery test target compiled, but Windows Application Control blocked execution with 4551; the latest test version is **NOT RUN**. No Actions ran.

**M8 independent Windows staging review follow-up (2026-10-24):** Owner supplied Cursor review `503403fb-76d0-4ae5-b467-031677cef57f`; recommendation **NOT READY**. The review confirms staging direction is sound within its narrow tested boundary, but C1–C5 and H1–H7 remain open gates; trust stays disabled. Fixed M1 rollback pin lifetime, M2 explicit duplicate `(DeviceId, segment_index)` rejection, and M3 incoming path binding. The incoming relative format is now `<device-id-hex>/<20-digit-segment-index>.seg` in ADR-0019 §5.1.1. A first test launch hit Windows Application Control 4551; ordinary Cargo retry ran. Fixed a stale fixture-path assertion. Added a validation primitive for the exact staged snapshot: complete supported v1/v2 frames, trusted-key signature checks, device/Brain identity, sequence and hash-chain continuity from a supplied verified tip. It is not connected to publication or RetrievalSafetyState/cutoff policy. Focused staging tests pass 10/10; full daemon library tests pass 408, with 4 ignored. Strict Clippy, fmt, and diff checks pass; App Control was unchanged. Full findings and remaining gates: `evidence/m8/m8-independent-windows-staging-review-20261024.md`. Next: bind this validation to the complete verified stream and trust/cutoff state, then implement the immutable prepared/committed journal and crash recovery. No GitHub Actions were run for this verification.

**M8 Windows staging/recovery slice 2 (2026-10-24):** Added a bounded, no-follow staging snapshot path in `services/cryptexd/src/import_staging.rs`, with SHA-256 readback, single-flight transaction lock, same-volume checks and explicit cleanup. `hold`/`login-hold` now runs startup reconciliation after the single-instance lock and before identity setup, managed transport or IPC; it removes only unprepared staging-only trees and returns typed `ImportRecoveryConflict` for unknown journal state. Follow-up changes made staging-name validation explicitly byte-based, made rollback idempotent, bounded recovered device-directory names, and added Windows owner checks plus a component-by-component `NtCreateFile` walk rooted at held directory handles. Native Windows tests reject a junction and show ancestor rename is blocked while handles are held. Focused staging tests pass (7/7); daemon library suite (405 passed, 4 ignored), strict Clippy, Windows-target library check, daemon bin check, fmt, and `git diff --check` pass. macOS still has only static path checks. The importer still does not parse/validate segment events, persist prepared/committed journal records, reconcile prepared transactions, or publish canonical segments; trust remains disabled. Evidence: `evidence/m8/m8-windows-staging-recovery-slice2-20261024.md`. Next: independent security review of the exact Windows adapter and continue with semantic validation and the durable import journal only after its results.

**M8 Windows import admission bounds (2026-10-23):** Pinned importer-only limits in `docs/adr/ADR-0019-m8-protocol-addendum-proposal.md`: 16 sealed files, 1 GiB/file, 4 GiB/batch, and one prepared transaction. Added checked budget accounting and five unit tests in `services/cryptexd/src/import_limits.rs`; Windows fmt, focused tests (5/5), full `cryptexd --lib` (398 passed, 4 ignored), and strict Clippy pass. This does not read transport or persist staging/journal data; there is still no importer, startup reconciliation, publication, or trust activation. Evidence: `evidence/m8/m8-windows-import-admission-limits-20261023.md`. Next implementation is the Windows handle-based bounded staging copy and durable journal/recovery gate, subject to the existing independent-review and platform durability gates.

**M8 Windows implementation slice 1 (2026-10-23):** Owner accepted ADR-0019 §5.1.1 as the Windows implementation direction. `canonical_writer.rs` now routes local append/seal/recovery lock acquisition through one shared inventory→device lock-set primitive; multi-device sets use ascending bytewise DeviceId order, reject duplicate IDs before filesystem changes, and release device locks before inventory. Added existing-stream serialization, held-inventory, ordering, duplicate, and reader/writer exclusion tests. Windows verification: `cargo fmt --all -- --check`; `cargo test -p cryptexd --lib` (393 passed, 4 ignored); `cargo clippy -p cryptexd --all-targets -- -D warnings` — PASS. Evidence: `evidence/m8/m8-windows-lock-set-slice1-20261023.md`. This is a foundational lock slice only: no incoming importer, journal recovery gate, trust activation, or macOS verification. Continue the Windows slice with bounded staging and durable import journal/publication; keep trust disabled until startup recovery, approval IPC, v3 RetrievalSafetyState integration, Windows durability, macOS parity, and independent security review gates pass. No GitHub Actions were run for this verification.

**M8 Windows source-path audit (2026-10-22):** Static inspection found no current Syncthing-to-canonical importer. The pinned managed-profile XML accepts one local device and rejects active folders/peers (`services/cryptexd/src/managed_profile/native_xml.rs`); the canonical writer only exposes local daemon append and verified reads. Therefore no imported canonical event path currently participates in locks, and no trust activation or incoming event publication is implemented. Windows daemon IPC currently uses a current-user named-pipe DACL, obtains the connection peer PID via `interprocess::peer_creds`, and compares it to the daemon's recorded parent PID held by a live process handle after checking its executable path (`services/cryptexd/src/ipc.rs`). This is useful existing process binding, but is not yet the M8 approval-IPC design: the proposal still needs explicit per-connection identity evidence, process replacement/reuse analysis, and adversarial tests. Evidence: `evidence/m8/m8-windows-importer-and-ipc-source-audit-20261022.md`. Static source review only; no native Windows runtime or race claim. Next: specify the daemon-owned staging/import publication boundary and the M8 approval IPC identity contract, then implement only against the reviewed protocol. Trust remains disabled.

**M8 importer publication design (2026-10-23):** Three independent Cursor reviews returned **NOT READY**. Critical gates included local writers not retaining inventory ownership, no startup import-journal gate before transport/readers, and no owner-approval IPC with connection-bound identity and one-time sessions. The proposal fixes staging/journal roots outside `canonical/events`, specifies handle-based identity/link checks and atomic staging, and separates import single-flight from canonical-lock waits. The owner then accepted §5.1.1 as the Windows implementation direction; this does not accept ADR-0019 as a whole or enable trust. Windows lock-set slice 1 is implemented and verified below. Remaining gates: incoming staging/publication and startup recovery, Windows and macOS durable-write strategies, platform-bound approval IPC, segment continuity, v3 security-state integration, macOS peer binding, numeric batch limits, bounded bootstrap parser, and independent reducer-vector review. Threat-model correction: the Master Spec does not require OS sandboxing against arbitrary same-user malware. Trust remains disabled. Evidence: `evidence/m8/m8-import-publication-boundary-audit-20261023.md` and `evidence/m8/m8-windows-lock-set-slice1-20261023.md`; reviews are summarized in Cursor chat “Independent security review findings.” Continue with Windows staging/journal/publication, then verify macOS parity.

**M8 independent security follow-up (2026-10-22):** The owner supplied a read-only follow-up review of the protocol candidate at `5d5076e` (review baseline `7fc6089`), recommending **NOT READY**. The review found two remaining reducer wording defects: step 3 did not explicitly carry eligible global incoming cutoffs across SCC filtering, and step 2e did not define mixed acyclic/cyclic splits. The candidate now defines each effective cutoff over the whole verified graph and requires a Brain-wide hold if any cycle remains after recomputing the affected subgraph. Golden vectors cover both cases. Evidence: `evidence/m8/m8-independent-security-rereview-followup-20261022.md`. This is a documentation correction, not another independent review or acceptance. Still blocking owner acceptance/activation: a real daemon-owned importer publication barrier that Syncthing cannot bypass; concrete Windows/macOS connection-bound peer-credential mechanisms; `security.*` to `RetrievalSafetyState` integration; and the bounded bootstrap parser implementation. Protocol remains PROPOSED / NOT ACCEPTED, with trust activation disabled. No code, tests, Actions, or trust state changed.

**M8 independent security re-review + reducer fix (2026-10-22):** Re-reviewed `docs/adr/ADR-0019-m8-protocol-addendum-proposal.md` at `5723cf1`/descendant; recommendation **NOT READY**. Evidence: `evidence/m8/m8-independent-security-rereview-20261022.md`. Critical: (1) component-wise cycle filter could use an ineligible outside-SCC breaker; (2) importer lock participation still unimplemented. Documentation-only follow-up pinned SCC + condensation topo order, eligible-only external cutoffs, self-retirement as a per-identity cap, a competing-fork predicate, and required reducer vectors in the proposal. It also corrects the proposal to label its multi-device lock order as a required M8 target; current `canonical_writer` does not implement it. These changes are not a new independent review or acceptance. Still open before acceptance: re-review of the corrected reducer, concrete Windows/macOS IPC peer-credential design, importer publication barrier implementation/tests, v3→`RetrievalSafetyState` integration. Protocol PROPOSED / NOT ACCEPTED; no trust activation, code writers, Actions, or tests in this docs follow-up. Next: obtain independent re-review of the corrected §4 procedure and lock-order statements.

**M8 independent Cursor security review (2026-10-22):** The completed read-only review of the current ADR-0019 protocol candidate recommends **NOT READY** and reports 2 Critical, 5 High, 4 Medium, and 1 Low finding. Findings are recorded in `evidence/m8/m8-independent-security-review-20261022.md`. The proposal now clarifies known-but-ineligible issuer handling, connection-bound owner approval, conflicting proofs, issuer/target lock sets, v3 policy integration, uncertain-append lookup, platform-specific journal durability, UUID canonicalization, importer publication locking, and required vectors. A follow-up aligns cycle handling with Master Spec §9 and accepted ADR-0019: unresolved mutually affected authority produces a deterministic Brain-wide hold; M11 recovery-root remains limited to all-ordinary-device loss and is not an M8 override. This alignment is still a candidate requiring independent re-review. Importer lock participation and retrieval-safety integration remain unimplemented. Protocol stays PROPOSED / NOT ACCEPTED; no trust activation, Actions, or tests in this documentation follow-up.

**M8 R1/R2 wording clarification (2026-10-22):** A local consistency pass made approval-token consumption explicitly atomic across concurrent confirm/cancel/replay, with consumption before signing or append and a fresh confirmation after later failure. Parser bounds now define root-inclusive container depth and decoded UTF-8 byte measurement for all strings. This is still proposal text, not independent review or acceptance; trust-changing implementation remains gated. No code/tests/Actions or Git publication.

**M7 completion-gate audit (2026-10-22):** Consolidated current evidence against the M7 prerequisite, scope, cross-platform, and DoD requirements. P1 physical containment remains a demonstrated blocker; edit/event, loop-suppression, runtime baseline, real-daemon unload, proposal authorization, CI, and review gates remain partial/open as detailed in `evidence/m7/m7-completion-gate-audit-20261022.md`. Managed-note I/O stays disabled; no M7 completion claim.

**M7 P1 current API contract check (2026-10-22):** Re-read current official Obsidian Vault/API docs against the accepted M7 gate and prior native Windows junction/macOS symlink observations. `Vault.process()` documents protection from concurrent content changes, not physical root containment; the docs also describe plugins as having Node-like filesystem access. This bounded review does not rule out a future supported capability. Managed-note I/O stays disabled. Evidence: `evidence/m7/m7-p1-current-obsidian-api-docs-review-20261022.md`. Resume only on an upstream supported containment contract or an owner-accepted, separately reviewed design that preserves Obsidian and meets the existing boundary matrix.

**M7 P1 platform-sandbox feasibility (2026-10-22):** Reviewed OS-level isolation options for stock Obsidian. Windows' documented AppContainer launch APIs are experimental and Electron compatibility/reparse behavior is unverified; macOS App Sandbox entitlement is sealed into the vendor app signature and cannot be added by a Cryptex plugin. Wrapper/re-signed builds, FUSE/WinFsp, VM, and separate-identity workflows change integration or deployment and are not selected by the accepted plan. No supported cross-platform containment route is established under the current architecture. Keep managed-note I/O disabled; resume M7 P1 on an Obsidian containment contract or an owner-approved, separately reviewed design. Evidence: `evidence/m7/m7-p1-platform-sandbox-feasibility-20261022.md`.

**M7 observation fixture rerun (2026-10-22):** On `codex/m8-device-trust` @ `03d447d`, `node --check` passed and the current observation-only source passed 18/18 local Node tests. `main.js` SHA-256 matches the earlier Windows/Mac observation source. A focused Rust IPC test was also attempted: shared-target build failed on `yoke_derive`; fresh external target was blocked by Windows Application Control 4551 executing a Cargo build script. No policy was changed. Evidence: `evidence/m7/m7-p1-windows-observation-fixtures-rerun-20261022.md` and `evidence/m7/m7-completion-gate-audit-20261022.md`. These checks are not native runtime, real-daemon, proposal, or physical-containment proof. M7 remains open and managed-note I/O stays disabled. No product code, Actions, commit, push, or merge changed.

**M8 scoped protocol review (2026-10-22):** Reviewed `docs/adr/ADR-0019-m8-protocol-addendum-proposal.md` against the Master Spec, accepted ADR-0015–0019 decisions, and current versioned event reader. Product direction is consistent; protocol remains NOT ACCEPTED and trust activation is not ready. Required closure findings: bind owner confirmation to an immutable transaction digest; set parser depth/container/string bounds before JCS; define durable pending-state/idempotency transitions; finish exact v3 cross-version chain/old-reader contract; canonicalize identifier wire forms. Full review: `evidence/m8/m8-protocol-addendum-scoped-review-20261022.md`. No code, tests, trust state, Actions, or Git publication changed. Next: resolve these protocol-format details in the proposal, complete reducer/lock/crash review, then obtain the required independent security review and explicit acceptance before implementation. M7 remains deferred and managed-note I/O remains disabled pending the physical-containment gate.

**M8 protocol review findings draft closure (2026-10-22):** Added candidate resolutions for M8-R1 through R5 in `docs/adr/ADR-0019-m8-protocol-addendum-proposal.md`: immutable approval digest/session binding, bounded pre-JCS parser, durable pending/idempotency transitions, exact v3 cross-version reader/key contract, and canonical identifier equality. The proposal remains NOT ACCEPTED; the draft is not an independent security review and does not authorize schema or trust activation. Evidence: `evidence/m8/m8-protocol-addendum-r1-r5-draft-closure-20261022.md`. Next: obtain independent security review, finish issuer/reducer and lock/crash review, then seek explicit protocol acceptance before implementation. No code/tests/Actions or Git publication. M7 stays deferred and managed-note I/O remains disabled.

**M8 protocol lock-order reconciliation (2026-10-22):** Compared the candidate with current `canonical_writer.rs`, `sequence_store.rs`, and `knowledge_materialization.rs`. The proposal now reflects the lock lifetime: inventory/device locks remain held; the sequence-store locks durably reserve a sequence and are released before append/sync; canonical locks remain held through append. It also requires post-confirmation boundary revalidation. Out-of-protocol event-file mutation is not race-free; Syncthing/import integration and platform durability are not proven. Pending-journal persistence/crash handling remain unimplemented and require review/tests. Details are in `evidence/m8/m8-protocol-addendum-r1-r5-draft-closure-20261022.md`. No tests/code/Actions; no trust activation. M7 stays deferred.

**M8 reducer/chain consistency follow-up (2026-10-22):** Clarified that sequence-number gaps after durable reservations are allowed only with a valid predecessor hash link, and cycle hold refers to the pre-prefix-filter candidate graph check. Source review is static and not independent; compromised-issuer, convergence, importer, durability, and crash behavior remain open review gates.

The local reducer sanity pass checked topological issuer-cutoff resolution, minimum eligible cutoff, self-retirement caps, union-based recomputation, and the conservative pre-filter cycle hold. It found no further internal ordering contradiction; the independent reviewer must still challenge order permutations, conflicting key history, compromised issuers and liveness.

**M8 cutoff/sequence-gap correction (2026-10-22):** A static consistency pass caught that the initial self-retirement draft used `n-1`, despite the existing allocator permitting skipped reservations. The candidate now binds retirement to the immediately preceding verified record's actual sequence/hash, and requires restrictions to name an existing event boundary; cutoff zero remains the empty-prefix sentinel. This is documentation-only and remains unreviewed. Evidence is included in `evidence/m8/m8-protocol-addendum-r1-r5-draft-closure-20261022.md`. No tests or trust code. M8 acceptance/independent review and M7 containment remain open.

**M8 reducer draft follow-up (2026-10-22):** Added the valid self-retirement boundary as a cap on that identity's effective authority prefix before evaluating its outgoing restrictions in the issuer-to-target DAG. This blocks post-retirement restriction authority in the proposal; cycle, cutoff, and convergence analysis still requires independent review. No reducer implementation or tests. M8 remains NOT ACCEPTED and M7 remains deferred.

**M8 reducer cycle behavior draft (2026-10-22):** The proposal now checks cycles among structurally/cryptographically valid restrictions from locally approved issuers before prefix eligibility filtering; any such cycle causes a Brain hold. The availability/denial-of-service tradeoff is explicitly unresolved pending independent review. No reducer code or tests.

**M8 Windows startup/status guest artifact follow-up (2026-10-22):** Verified the owner-supplied `Cryptex-M8-StartupStatus-20261022.zip` against staged SHA-256 `1c9ba03b…814016`; all 33 normalized source-manifest entries match both the package and current checkout. Supplied `results.json` reports all 8 runner steps exit 0; `native-provision.log` reports the targeted managed-profile/daemon startup-stop test 1/1; `native-provision-result.json` confirms loopback isolation, credential cleanup, identity preservation, collision refusal, clean managed start/stop and instance-lock release. This closes the staged Windows startup/status guest execution gate only. Authenticated native Desktop status, peer admission, incoming replication, convergence, cutoffs and full M8 remain open. Cursor's independent protocol review is blocked by its local hook (`Cortex privacy guard is unavailable; protected read blocked`); no hook/security change was made. Trust activation remains gated on independent review and explicit acceptance; M7 write-back remains disabled. No Actions or Git publication.

**M8 independent review handoff (2026-10-22):** Prepared `evidence/m8/m8-independent-security-review-prompt-20261022.md` for a read-only Cursor review of the current proposal, accepted authority files, event reader, writer/sequence locks, and review helpers. This agent's static review is not independent and does not satisfy the security gate. Next: run that prompt in Cursor and bring back its findings; do not accept ADR-0019 or implement trust/schema writers before findings are resolved and owner acceptance is explicit.

**M8 protocol candidate drafted and consistency-corrected (2026-10-22):** `docs/adr/ADR-0019-m8-protocol-addendum-proposal.md` specifies bounded signed bootstrap messages, direct local approval anchors, proposed v3 immutable security records, deterministic restriction reduction, and crash/rebuild boundaries. Consistency review bound exact Syncthing association IDs into the signed approval snapshot/anchor, defined self-retirement's control-event boundary, fixed transcript hashing over the complete four-message array, excluded policy records from user-memory projections, and pinned key-ID derivation plus role/link checks to ADR-0016. It remains PROPOSED for scoped security review, not accepted. Schema/domain labels and causal rules have not had independent security review; no trust-changing implementation or tests were run. Existing permission decisions remain in the accepted ADR-0019 records. Next: independent scoped security review of this candidate, resolve findings, then obtain explicit protocol acceptance before any writer/resolver/transport trust activation. Windows lifecycle gates remain scoped only; consolidated Mac later, M7 off, no Actions.

**M8 retry/idempotency schema correction (2026-10-22):** Added `transaction_id` to restriction and self-retirement payloads, specified a domain-separated operation digest over stable event identity/payload fields while excluding retry-varying chain placement and signature fields, and required duplicate matches to hold the Brain. Required vectors now cover shifted-chain retry, changed payload, cross-type ID reuse, and duplicate records. This is proposal-only; generated schemas, tests, independent review, acceptance, and trust activation remain pending. `git diff --check` is the only validation for this docs-only adjustment; no code/tests/Actions or Git publication.

The M8 pending journal now also records Brain/operation, frozen confirmation snapshot+digest, and whether a local approval signature was generated/released so restart behavior can re-present the precise candidate. A candidate atomic-write protocol now follows existing Unix/Windows durability patterns; rollback protection and fault-injected platform verification remain open gates.

**M8 issuer-permission choice accepted (2026-10-22):** Owner explicitly selected allowing a trusted installation to restrict a different lost/stolen device with local owner confirmation. Decision and limits: `docs/adr/ADR-0019-m8-restriction-authority-addendum.md`. This resolves the product permission choice in TRUST-04 only; it does not close compromised-issuer/causal reduction, exact bootstrap/consent, approval anchors, security schema/golden vectors or transaction/restart gates. No writer/resolver/IPC mutation activated. Primary workspace remains cleanly managed at `D:/github_repos/cryptex` on `codex/m8-device-trust`; Mac later, M7 off, no Actions or new runtime tests for this documentation-only decision.

**Workspace correction after owner-authorized cleanup (2026-10-22):** Primary working checkout is now `D:/github_repos/cryptex` on `codex/m8-device-trust`, corrected from the accidental GitHub Desktop main checkout. Auxiliary `C:/Users/Tarik/.codex/worktrees/m8-device-trust/cryptex` is clean/detached at `9e8d684`; use the primary path for new work. All 145 Desktop-stash paths were reconciled with the pushed checkpoint: 31 identical, one handoff superseded without missing lines, 113 obsolete scratch files. Complete stash/M8 Git bundle verified outside Git before dropping the exact stash; old untracked generated daemon binary likewise backed up/hash-verified before removal. Six fully merged obsolete branch refs deleted locally/remotely with ancestry/open-PR/exact-tip checks; default foundation, main, M5 and active M8 retained. Audit/recovery location: `evidence/m8/m8-local-workspace-cleanup-20261022.md`. No product source/runtime behavior changed, no new tests/Actions/merge/release claim. Existing M8 trust/Mac gates and known issues remain open.

**Owner-authorized Git checkpoint (2026-10-22):** Owner requested review, commit and push of pending work. Publishing scoped M6/M7 evidence/observation source and the M8 Windows transport/worker/startup/status candidate plus reviewed artifacts on existing `codex/m8-device-trust`. Current fmt/strict daemon Clippy PASS, preserved M7 observation tests 18/18 PASS and all 33 M8 candidate source hashes still match the VM-reviewed inventory. Original checkout/scratch files preserved. Newer unrelated upstream MCP/license/docs commits remain on their existing branches; no merge/rebase/main update requested. CI configuration triggers push only on main; branch-only publication opens no PR or workflow dispatch. Review and limitations: `evidence/m8/m8-git-checkpoint-review-20261022.md`. Trust activation/Mac/M8 completion and prior known issues remain open; this is a preservation checkpoint, not release approval.

**M8 trust activation readiness review (2026-10-22):** Completed scoped agent source/document review against Master Spec, ADR-0018 and ADR-0019; no trust activation authorized. Current resolver remains local-only, future event schemas unverifiable, review receipt in-memory only and transport status protected=false. Five explicit blockers remain: exact bootstrap/possession exchange; durable approval/bootstrap anchors/rebuild; security bytes/schema/domains/reader-first compatibility; issuer permissions/causal conflicts; transaction/lock/crash semantics. Review: `evidence/m8/m8-trust-security-review-20261022.md`. Focused concrete anchor/reinstall and restriction-conflict proposal: `evidence/m8/m8-authority-anchor-review-proposal-20261022.md` (PROPOSED / NOT ACCEPTED). Existing product direction is already accepted and should not be re-asked; exact remaining protocol choices still require a complete reviewed addendum before trust-changing code. No implementation, tests, independent crypto review or security gate PASS claimed in this document-only step. Windows lifecycle gate stays scoped closed; M8/Mac/security gates remain open, M7 off, no Actions/commit/push/merge.

**M8 Windows guest artifacts reviewed (2026-10-22):** Read owner-supplied results.json (8/8 steps exit 0), native-provision.log (exact native fixture 1 PASS / 0 failed) and native-provision-result.json. Receipt corroborates controller clean stops 7072/8436; actual injected whole-job loss -> replacement 7544 -> 2084, cancellation and finite four-start exhaustion; preserved native identity; worker clean stop PID 936; duplicate-profile refusal; missing receipt not recreated; actual daemon managed startup/stop PID 4964 and instance lock release after worker join; wrong-key 403x3, foreign endpoint refusal before API bytes, loopback isolation and synthetic credential deletion. Scoped Windows startup/worker/native lifecycle verification gate closes on reviewed owner artifacts. This is not independent guest operation, guest ZIP-hash linkage, full per-target count review, packaged Desktop status native round-trip, Mac or M8 completion. Review: `evidence/m8/m8-startup-status-guest-artifact-review-20261022.md`. Host 4551 and prior intermittent credential/worker failures remain open/separate. Next: exact ADR-0019 protocol/schema/issuer review before peer trust activation, incoming/convergence/cutoff gates and later Mac consolidation. No Actions, commit, push, merge or new tests in this artifact-review step; original dirty checkouts preserved.

**M8 startup/status owner Windows VM receipt (2026-10-22):** Owner screenshot reports `Scoped Windows verification PASS` and `Verification exit: 0` for the staged startup/status review. Guest report `C:/Users/Tarik/AppData/Local/Cryptex-Dev/m8-verification/profile-batch-68fe7065fe614873a77481d7b6b97de6`. Record as owner-reported aggregate batch PASS; raw results/logs/native PIDs and guest ZIP hash have not been imported or independently reviewed. The early “No transport was launched” message concerns preparation, before the runner's explicit native tests. Evidence: `evidence/m8/m8-startup-status-windows-guest-receipt-20261022.md`. Host 4551 remains separate/unresolved; no global policy resolution claimed. Next: inspect guest receipts/logs for exact startup/status native coverage, then remaining ADR-0019 trust-protocol/security gates and Windows incoming/convergence/cutoff work; Mac later. M8 not complete, M7 off, original dirty checkouts preserved; no Actions/commit/push/merge.

**M8 startup/status VM candidate staged (2026-10-22):** Exact development archive `D:/VMs/Win11/Cryptex-M8-StartupStatus-20261022.zip` (13,734,246 bytes), SHA-256 `1c9ba03b53801ce4b0a5cbc9a49fbdb82a3bb12d12339bb10ffc8bd27b814016`; actual ZIP source entries match all 33 hashes and native executable retains accepted pin. Host test launchers remain blocked by OS 4551; no policy bypass/Actions. Guest handoff: extract whole ZIP in the authorized Win11 development VM and run `Run-Windows-Review.cmd`, retain aggregate result/report path. Launcher verifies runner hashes, uses process-only RemoteSigned and removes only those scripts' download marks; no machine policy change. Syntax parsed but not executed on host. Guest tests/native startup/status remain NOT RUN, M8 open. Detailed packaging boundaries: `evidence/m8/m8-startup-status-development-vm-bundle-20261022.md`. Original dirty checkouts preserved; no commit/push/merge.

**M8 explicit startup/Desktop status candidate (2026-10-22):** Prepared opt-in Windows lifecycle hold options for an already-ready private transport profile/pinned package/fixed ports, worker startup readiness under the instance lock, and worker shutdown/join before clean lock-release reporting. Defaults remain transport-off. Added read-only nonblocking StatusReader and authenticated Desktop-only `transport.status` via existing process identity + exact status.read/no-body checks; fixed labels/error categories only, protected=false. No new Tauri capability, trust or peer/folder activation. Three IPC tests, two CLI parser tests and a real daemon start/stop native fixture were added and compile under strict Clippy. Host fmt/Clippy/build PASS; events library 61 PASS, but broad schema-generator and separate final daemon lib/CLI test launchers are blocked before execution by Application Control 4551. New runtime/native checks are NOT VERIFIED; prior worker/native PASS must not be carried onto this candidate. No policy changes/bypass/Actions. Evidence: `evidence/m8/m8-startup-status-windows-20261022.md`, new 33-file manifest; runner updated to use freshly built daemon in native fixture. Next: run exact candidate in the authorized Windows development VM, review new startup/status gates, then remaining trust-protocol review and consolidated Mac. Original dirty checkouts preserved; M7 off; no commit, push, merge or M8 completion.

**M8 Windows explicit daemon worker progress (2026-10-21):** Added `ManagedTransportWorker::spawn_existing`, owning package/profile/controller inside one blocking worker with monotonic cancellation, bounded explicit shutdown/join, retained ownership after deadline, and fixed-size in-process typed status. Added a separate `reopen_native` validation path with no provisioning/generation fallback. Real worker PID 35728 reached authenticated local readiness and cleanly stopped/joined; duplicate worker refused exact Profile(Busy), repeated shutdown was idempotent, missing synthetic receipt refused without recreation, profile reopened after join, exact synthetic credential deleted and no Syncthing remains. Seven new worker tests include timeout/rejoin, sticky cancellation, panic and poisoned diagnostics. Final fmt, strict Clippy, build, events 61 / daemon library 386 (4 ignored) / CLI 8 / lifecycle 9 / Mode B 3 / update 2, all four native fixtures and PS5.1 helper 3/3 PASS; 32-file source guard PASS. Evidence: `evidence/m8/m8-worker-windows-verification-20261021.md` and adjacent contract/manifest. Explicit backend worker only: production startup selection, lifecycle owner call sites, authenticated status IPC and whole-daemon-death coverage remain open. Earlier intermittent credential/worker failure observations unresolved; no assertion weakened. ADR-0019 protocol/schema/issuer review still gates trust activation, Mac later, M7 write-back off; no Actions, commit, push, merge or M8 completion.

**M8 Windows real owned-job fault checks (2026-10-21):** Extended the explicit synthetic native fixture with test-only owned-job termination through retained handles, using the real native driver/controller and actual backoff. PASS: one forced exit -> authenticated replacement -> requested clean stop (24384 -> 36884); cancellation during backoff launches no replacement (26304); four healthy-then-forced attempts exhaust the budget without a fifth start (35824/31472/26536/39620). Identity/credential preserved; all profile in-use flags released, no unresolved cleanup, exact synthetic secret deleted and Missing verified; final native process count zero. Forced exits are unclean; this is injected whole-job loss, not organic crash/root-only/power-loss proof. Final fmt, strict Clippy, build, events 61 / daemon library 379 (4 ignored) / CLI 8 / lifecycle 9 / Mode B 3 / update 2, all four native fixtures and PS5.1 helper 3/3 PASS. Evidence: `evidence/m8/m8-controller-native-faults-windows-20261021.md`; new 31-file faults manifest, prior receipts/manifests preserved. Earlier credential/worker flake remains OPEN. Next Windows scope: production worker ownership/cancellation/status integration within no-peer/no-trust boundaries; exact ADR-0019 protocol/schema/issuer review still gates pairing/security writes. Mac later; original dirty checkouts untouched; no Actions, commit, push, merge or M8 completion.

**M8 Windows isolated controller progress (2026-10-21):** Added explicit blocking `PreparedNativeProfile::supervise` over the validated private native session, with local-only typed status, authenticated health/stop, finite 1/2/4-second backoff after confirmed full owned-job exit (four starts maximum), cancellation and terminal refusal on start/ownership/key/health/shutdown errors. No production startup/IPC or peer configuration is activated. Final Windows PASS: events 61; daemon library 379 / 4 ignored; CLI 8; lifecycle 9; Mode B 3; update/rollback 2; all four native fixtures; fmt, strict all-target Clippy, build and PS5.1 helper 3/3. Two real controller start/health/clean-stop cycles passed (PIDs 20432/13732), no Syncthing remains. Eight new deterministic policy tests cover bounded restart/cancel/failures; actual native crash-driven restart remains unverified. Initial broad run hit existing abrupt-profile worker exit 101 instead of 97; stderr was previously discarded, so cause unknown. Only helper stderr diagnostics were retained; final isolated/broad pass does not resolve this or the earlier read-after-delete flake. Evidence/contract/new 31-file manifest: `evidence/m8/m8-controller-windows-verification-20261021.md` and adjacent files. Next: native crash/restart/cancellation and production worker/status integration within the no-trust boundary; ADR-0019 exact protocol/security review remains required before pairing activation. Mac consolidated later; no Actions, commit, push, merge or M8 completion.

**M8 trust implementation review input (2026-10-21):** Prepared `evidence/m8/m8-peer-trust-implementation-review-draft-20261021.md`, explicitly PROPOSED / incomplete / NOT ACCEPTED. It maps the pairing states, bilateral binding requirements, durable authority/reader boundary, inclusive cutoffs, persistence/lock-order constraints and a required failure matrix to ADR-0019 section 8. Exact bootstrap bytes/channel, local approval-anchor rebuild semantics, security schema/golden vectors, causal issuer policy and transaction crash/lock design remain unresolved; no cryptographic/security review or trust activation is claimed. No runtime source changed and no new tests were run for this documentation-only step. Next executable Windows scope remains controller/restart/status integration within the existing no-peer/no-trust boundary, while completing and reviewing that addendum before remote authority work. Prior native provisioning evidence and the open credential-deletion flake remain unchanged. Original dirty checkouts preserved; no Actions, commit, push, merge or M8 completion.

**M8 Windows native provisioning/session progress (2026-10-21, latest):** Added explicit pinned native generation into a private quarantine child, strict bounded version-52 XML isolation policy, separate CREATE_NEW prepared home, immutable initialization/completion receipts and identical-byte interrupted-staging recovery. Missing/corrupt/changed ready credentials, identity, settings or ports refuse; unconfirmed generation never regenerates. Managed sessions retain profile/home/package pins, prevent active/unresolved replacement, attest bounded local readiness, and require owned-tree exit plus native stop-log checks for clean shutdown. No production startup/IPC is wired. Final Windows PASS: events 61; daemon library 371 / 4 native ignored; CLI 8; lifecycle 9; Mode B 3; update/rollback 2; all four explicit native fixtures separately PASS; fmt, strict Clippy, build and PS5.1 helper 3/3 PASS. New product-provisioning fixture verified three clean starts (PIDs 25596/15736/39560), same identity across reopen, actual wrong-key 403, live loopback/isolation settings, foreign API-port collision refusal before HTTP bytes, explicit retry after release and synthetic OS-credential deletion. No Syncthing process remains. Evidence: `evidence/m8/m8-native-config-windows-verification-20261021.md`; contract/new 30-file manifest adjacent; runner updated, prior sealed packages unchanged. Earlier separate credential-deletion flake remains OPEN despite current passes; no test/store behavior was weakened. Next Windows scope: automatic controller/restart/status integration plus concrete ADR-0019 bootstrap/security-event/issuer-authority design and security review before pairing/trust activation; then incoming/convergence/cutoff gates, with Mac consolidated later. No owner interaction needed now; no Actions, commit, push, merge or M8-complete claim.

**M8 Windows pinned package progress (2026-10-21, latest):** Added backend-only exact Syncthing 2.1.5 Windows amd64 executable + LF notice pins, top-down non-reparse directory/file guards retained through owned-job lifetime, and a fixed owned offline generation role. Real fresh protected-profile generation PASS (PID 34436, exit 0); same-size binary/notice tampering, missing notice and package replacement refuse. Abandonment cleanup observed before pins release. No server started from generated defaults. Final broad Windows command PASS: events 61; daemon library 356 / 3 native ignored; CLI 8; lifecycle 9; Mode B 3; update/rollback 2. Explicit private-profile/Windows Credential Manager/owned-endpoint native fixture also PASS (PIDs 2660/31840, clean exit 0, wrong key 403, synthetic credential deleted). Fmt, strict Clippy, build and PS5.1 runner-helper 3/3 PASS. Earlier credential read-after-delete failure remains OPEN: unchanged exact test 12/12 and broad reruns pass, cause not established. Evidence: `evidence/m8/m8-pinned-package-windows-verification-20261021.md`; boundary contract and new 27-file manifest adjacent. Runner updated, sealed prior ZIP unchanged; no new owner step needed. Next: strict native config provisioning/recovery BEFORE any generated-config server launch, then controller lifecycle integration; ADR-0019 trust activation and consolidated Mac remain gated. No Actions, commit, push, merge, production startup/IPC activation or M8-complete claim.

**M8 Windows owned endpoint progress (2026-10-21, latest):** Added established-connection verification (exact tuple, live retained peer handle, actual owned-job membership including monitor children, live root) BEFORE any API bytes, then fixed-operation hyper HTTP on that same socket with no URL reconnect/proxy/pool. Legacy URL REST is test-only; reqwest is dev-only. New HTTP/table entries 8+3 PASS, legacy REST 19 PASS; final broad recheck events 61 and daemon library 352 PASS / 2 native fixtures ignored, then CLI launcher blocked by Application Control 4551. Explicit real private-profile/Windows Credential Manager/owned-endpoint native test PASS: root PIDs 22900/30040, exit 0, forced false, actual wrong-key 403; synthetic credential deletion verified. Fmt, non-test check, strict Clippy and build PASS. Initial fixture-sharing cleanup race was fixed only in bounded test cleanup. A separate existing abrupt-ready credential test failed once on read-after-delete; unchanged isolated 8/8 and final parallel rerun pass, but cause remains OPEN (no assertion weakened, no SecureStore behavior change). Evidence: `evidence/m8/m8-owned-endpoint-windows-verification-20261021.md`; contract/source manifest adjacent, 22 files. No new owner interaction is needed for this result. Next Windows work: package/native-config provisioning, controller startup/health/restart/shutdown integration, and investigate that credential-deletion failure; keep Mac consolidated later and ADR-0019 trust activation gated. No Actions, commit, push, merge, production IPC/startup activation or M8 completion.

**M8 Windows guest managed-profile receipt (2026-10-21, latest):** Owner's final invocation with the stderr-repaired downloaded runner and an absolute Resolve-Path native executable argument reports `Scoped Windows verification PASS`; report `C:/Users/Tarik/AppData/Local/Cryptex-Dev/m8-verification/profile-batch-e3f006113b5b42b6b9e72a42331c2d6c`. This closes the scoped guest package/profile/owned-process/OS-credential verification receipt, not all Windows M8 or the milestone. It is owner-supplied aggregate terminal evidence; raw results/test counts/native PID records have not been imported or independently reviewed. Guest script bytes differ from the later internally path-normalizing worktree scripts, whose separate PowerShell 5.1 regressions pass; Rust product source is unchanged. Evidence: `evidence/m8/m8-managed-profile-windows-guest-pass-20261021.md`. Continue Windows native config/package provisioning, established-endpoint ownership and controller integration, then the gated ADR-0019 trust/bootstrap/security-event work. Keep Mac consolidated after the Windows stage, M7 write-back off, Actions off and original dirty checkouts preserved. No commit, push, merge or M8 completion.

**M8 Windows native-fixture path correction (2026-10-21, latest):** Owner's stderr-repaired VM runner reached fixture preparation after the four guarded Cargo steps, then Process.Start failed because a relative executable path was resolved against .NET's different current directory. Resolve an existing filesystem FileInfo.FullName in both the runner and preparer before hashing/launching; two script manifest entries refreshed. Actual Windows PowerShell 5.1 relative-path regression PASS with a deliberately different .NET working directory and real pinned native config generation (no server launch); stderr/exit regression remains 3/3 PASS. Evidence: `evidence/m8/m8-windows-native-fixture-relative-path-fix-20261021.md`. The owner can rerun the already downloaded, repaired bundle unchanged by passing a Resolve-Path absolute native binary argument. No guest native/batch PASS is claimed until the result arrives; Rust source is unchanged. Actions remain off; no commit, push, merge or M8 completion.

**M8 Windows guest runner correction (2026-10-21, latest):** Owner verified the original development ZIP hash and reached Cargo in the VM; Windows PowerShell 5.1 treated ordinary native stderr (`Downloading crates ...`) as terminating under the runner's Stop preference. Fixed only the native command helper and its source-manifest entry: resolve Cargo first, initialize/read the actual global native exit status, temporarily capture native stderr with Continue and restore Stop in finally; actual nonzero exits still fail. Added a real Windows PowerShell 5.1 regression: stderr/exit 0, exit 7 and absent executable all PASS 3/3. New runner LF SHA-256 `3cfa2b7000910c613b20f3af0ae21d6fd78e441f435c1175b0e67755a722e213`. A deterministic two-file owner repair was checked against the original ZIP; the sealed original ZIP/proof remain unchanged. Evidence: `evidence/m8/m8-windows-powershell51-runner-fix-20261021.md`. Full guest batch/native verification remains OPEN pending rerun. No Rust/native product change, Actions, commit, push, merge or M8-complete claim.

**M8 managed-profile Windows progress (2026-10-20, latest):** Added explicit backend-only `ManagedProfile` creation/reopen with creation-time user-only protected DACLs, pinned non-reparse ancestor/root handles, exclusive session ownership, bounded strict metadata, independently random OS-account namespaces and interrupted API-key enrollment through SecureStore without replacement of ready missing/conflicting secrets. During implementation, 20 profile entries passed (including real Windows Credential Manager and abrupt-exit recovery), and a broader run passed events 61, daemon lib 341 / 2 opt-in native fixtures ignored, CLI 8 and lifecycle 9 before Application Control blocked the Mode B launcher. An early real native/private-profile/OS-store fixture passed two clean starts/stops; final test-only cleanup/account-report refinements were subsequently made. Final fmt, strict Clippy and build PASS, but final-source daemon/native test launches are blocked by OS error 4551; do not carry the earlier native PASS onto the final candidate or call the package verified. No security policy was changed. Evidence and 16-file source manifest: `evidence/m8/m8-managed-profile-windows-verification-20261020.md`; boundary contract: `evidence/m8/m8-managed-profile-boundary-contract-20261020.md`. A Windows batch runner is prepared at `scripts/m8/verify-managed-profile-windows.ps1`; its execution is not claimed. Continue final Windows verification in the authorized development environment, then native config/package provisioning, established-endpoint ownership and controller integration. ADR-0019 activation gates remain; no production startup/IPC, peer trust or canonical schema/writer is enabled. Keep Actions off, Mac consolidated later, M7 deferred and original dirty checkouts preserved.

**M8 owned-process Windows progress (2026-10-20, earlier):** Added the Windows owned-process foundation with atomic job assignment, explicit NUL-only handle inheritance, bounded waits and owned-tree cleanup. Final normal package test invocation PASS: events 61; daemon library 321 PASS / 1 native fixture excluded by default; CLI 8; lifecycle 9; Mode B 3; update/rollback 2; schema-generator and doc targets also executed. The explicit native fixture was separately run and PASSED: real pinned Syncthing 2.1.5 started/stopped twice through the new job owner (PIDs 36984/24028), both exit 0, no forced cleanup, wrong keys rejected 403, live isolation/folder flags checked and no API keys in generation/native logs. Fmt, strict Clippy, build and diff check PASS. Exact logs, source/binary hashes and failures/dispositions: `evidence/m8/m8-owned-process-windows-verification-20261020.md`; contract: `evidence/m8/m8-owned-process-boundary-contract-20261020.md`. The earlier exact-candidate auxiliary-launcher blocker did not recur in this final invocation; no global security-policy resolution or settings change is claimed. This code is not wired into production startup/IPC; package/profile/permission validation, OS secret provisioning, endpoint binding and lifecycle/controller integration remain. Non-Windows spawn refuses until the later Mac stage. Next within the Windows batch: integrate the protected managed profile/secret/owned-endpoint boundary and reviewed trust bootstrap/security-event design; do not enable remote trust or claim M8 complete. Keep Actions off and M7 deferred.

**M8 Windows implementation progress (2026-10-20, current):** Owner authorized the recommended ADR-0019 product direction subject to repository rules. Direction accepted; exact bootstrap/security-event/issuer-authority protocol and activation are still gated. Added in-memory `peer_review` policy (25 tests) and bounded read-only `syncthing_rest` (19 entries, including a worker helper) in the isolated M8 worktree. Final Windows host verification: event library 61/61, daemon library 305/305, daemon CLI 8/8, fmt, strict daemon Clippy, build and missing-runtime status smoke PASS. A broader invocation was blocked when Application Control refused the auxiliary events schema-generator test launcher; do not claim all targets ran. No security settings were changed. Source remains uncommitted on base `74bc3b1`; source hashes, binary hash and full logs are recorded in `evidence/m8/m8-review-rest-windows-verification-20261020.md`. Contract: `evidence/m8/m8-peer-review-and-rest-implementation-contract-20261020.md`. No real peer trust, canonical schema/writer, transport process/controller or incoming admission is enabled. Next is the reviewed durable trust/bootstrap/security-event design and managed transport integration, within the Windows-first batch; consolidate Mac work later. Actions remain unauthorized, M7 deferred, original dirty checkouts preserved. This supersedes the proposal-only status and blanket Windows-build-blocked statements below for the checks actually observed here.

**M8 Windows receipt and design boundary (2026-10-20):** Owner screenshot records guest `verify-m8-windows.cmd` exit 0 after Rust setup, report `setup-27583-27640`. This is the diagnostic package's aggregate PASS, not full M8 or independently imported command logs. Receipt: `evidence/m8/m8-slice1-windows-guest-verification-20261020.md`. The host Application Control restriction remains separate. ADR-0019 is now PROPOSED, not accepted: bilateral owner-approved identity/key pinning, no transitive grants, immutable versioned security events and conservative sequence cutoffs. Owner selection plus the complete reviewed bootstrap/schema/issuer-authority addendum are required before activating peer trust. Keep Windows-first delivery, Actions off, M7 deferred and original dirty checkouts preserved.

**M8 delivery order (2026-10-19):** Owner requested two large platform stages: complete Windows development and verification first, then one consolidated Mac verification/hardening stage. Stop issuing per-slice Mac handoffs. Keep meaningful internal tests and all security/DoD gates. The host still blocks generated dependency build scripts with Application Control 4551; the owner opened the existing Win11 VM, whose desktop is confirmed but VMware guest CLI operations still require the encryption password. A one-time owner-started guest verification batch is needed if no authenticated guest CLI is available; do not disable security settings or automate terminal/authentication UI. Plan: `evidence/m8/m8-platform-batch-delivery-plan-20261019.md`. The current status-only diagnostic is not a completed Windows M8 stage.

**Current milestone — M8 (2026-10-19):** Owner explicitly accepted the temporary M7 deferral / M8 sequencing exception ("tamam devam edebilirsin"). M8 preparation and implementation are authorized within the existing M8 contract. M7 remains OPEN / DEFERRED with managed-note write-back disabled; M6 Markdown/Obsidian visualization stays available. Existing M8 technical prerequisites and completion gates remain mandatory; Actions are not authorized. Readiness/source-gap map is recorded at `evidence/m8/m8-readiness-and-slice-map-20261019.md`. Decision: `evidence/m7/m7-temporary-deferral-and-m8-sequencing-proposal-20261019.md`. This entry supersedes older M7-only continuation and milestone-order wording below.

**M8 Slice 1 verification (2026-10-19):** Implementation is isolated on `codex/m8-device-trust` at `C:/Users/Tarik/.codex/worktrees/m8-device-trust/cryptex`, based on product `74bc3b14127962579ae03838bafea9336612a033`. New read-only `cryptexd replication status --runtime-root <path>` reports public metadata counts and explicitly unconfigured/unimplemented replication components; stored `Trusted` declarations never grant peer trust. Owner-reported Mac verification PASS at exact patch `f5cf1dda729d2329fe140cc809e926e6270622e52d3dc020b469390fd92cd2ab`: source checksums 4/4, library tests 12/12, CLI tests 3/3, fmt, strict daemon Clippy, build and read-only missing-runtime smoke. Local source reinspection matches the same four checksums. No identity/key creation, network connection, canonical mutation, or transport launch is added. Windows build/test/Clippy verification remains blocked by Application Control (4551), so cross-platform closeout is still open. Next: prepare the peer-authentication/key-binding and cutoff design before adding a durable trust mutation; requirements and unresolved decisions are in `evidence/m8/m8-slice2-peer-admission-requirements-20261019.md`. Keep the designated M7 verification worktree and dirty main checkouts unchanged. Evidence: `evidence/m8/m8-slice1-macos-verification-20261019.md` and the Windows blocker record. No Actions, commit, push, merge, or M8-complete claim.

**Owner interim direction (2026-10-18):** Until the pending Obsidian forum/API response arrives, continue with the accepted M6 model: Cryptex materializes deterministic Markdown as a local derived vault and Obsidian is used to display/visualize it. Keep plugin-controlled managed-note write-back disabled; do not use direct filesystem writes as a substitute for plugin Vault API containment. This is an interim operating direction, not an amendment to the accepted M7 scope or a completion claim. M6 is already complete within its accepted scope; M7 write-back remains blocked on the P1 physical-containment prerequisite and the forum/API response.

**Mac worktree organization (2026-10-18):** Owner-reported read-only inventory found the original `/Users/tarikgungor/github_repos/cryptex` at `87e9c4b` dirty and two clean detached verification worktrees at `74bc3b14127962579ae03838bafea9336612a033`. Designate `/tmp/cryptex-m7-verify-20261018` as the isolated read-only verification worktree; leave the original dirty checkout untouched. No sync, branch switch, stash, reset, cleanup, move, test, or source edit was performed.

**M7 P1 macOS Obsidian version inventory (2026-10-18):** The apparent 1.13.7/1.14.4 mismatch is explained by Obsidian auto-update: `/Applications/Obsidian.app` plist, `mdls`, and bundled ASARs are 1.13.7, while the running payload opened from `~/Library/Application Support/obsidian/obsidian-1.14.4.asar` reports runtime/API 1.14.4. The temporary plugin has `minAppVersion=1.5.0` and no API-types package (plain CommonJS). This resolves the inventory discrepancy only; no Cryptex support range is selected. Report: `evidence/m7/m7-p1-macos-obsidian-version-inventory-20261018.md`.

**M7 P1 macOS health IPC (2026-10-18):** Owner-reported status-only `health.ping` from the observation-only plugin in the synthetic vault returned `health.pong` from isolated daemon PID `19061`; pilot Desktop PID `19035` was built from exact source `74bc3b14127962579ae03838bafea9336612a033`, then clean-quit and released its lock. The installed plugin hash matched the verified observation source. This closes the exact-source status-only health IPC check. It proves neither Obsidian peer authentication, proposal authorization, real-daemon pending-request cleanup, nor physical containment. Managed-note I/O remains disabled; M7 is not complete. Record: `evidence/m7/m7-p1-macos-health-ipc-20261018.md`.

**M7 P1 macOS observation lifecycle (2026-10-18):** Owner-reported synthetic-vault check matched the observation-plugin hash and observed one disposable `Cryptex/notes/` edit (`modify: 0→1`). The attempted soft disable plus Cmd+R did not unload it (`modify: 1→2`); after hard Quit/reopen with the plugin disabled, the count stayed at 2 during a second edit. Only `cryptex-m7-spike` was restored; the disposable file was removed through Obsidian UI, and the `managed-conflict-probe.md` sentinel hash remained unchanged. This is event-observation/lifecycle evidence only. It does not resolve physical containment, proposal authorization, or real-daemon in-flight cleanup. Managed-note write-back remains disabled; M7 P1 and M7 remain incomplete. Evidence: `evidence/m7/m7-p1-macos-observation-lifecycle-20261018.md`.

**M7 P1 forum status (2026-10-16):** The submitted Obsidian API containment request remains unpublished pending moderator approval. The forum system notified the account that it is temporarily on hold until staff reviews recent posts; creating topics or replies is unavailable during review. No follow-up or appeal was sent. This does not change the M7 technical gate: managed-note I/O remains disabled, and independent native/IPC checks may continue where their required isolated host/runtime is available. Record: `evidence/m7/m7-p1-obsidian-api-containment-request-20261016.md`.

**M7 P1 public Vault API containment review (2026-10-16):** Reviewed the current official Obsidian public declarations and Help. Vault/adapter operations take vault-relative or normalized paths; the reviewed API contract documents no no-follow or handle-bound physical-root containment guarantee. Obsidian Help says symlinks and junctions can expose files outside a vault, consistent with the synthetic escape observations. This is a bounded documentation review, not proof no future supported mechanism can exist. Under accepted M7 requirements (Vault API, physical containment on both OSes, no direct-filesystem fallback), no compliant write design is currently established. Managed-note I/O remains disabled per owner choice. P1 next requires a supported Vault API contract or owner-approved design with a testable physical-boundary guarantee, then synthetic escape/race validation on Windows and macOS. No product code or architecture changed. Health IPC, proposal authorization and real-daemon cleanup remain separate NOT RUN gates. Details and official sources: `evidence/m7/m7-p1-vault-api-containment-review-20261016.md`.

**M7 P1 API request (2026-10-16):** Submitted “API request: physically contained Vault operations for managed paths” to the official Obsidian `Devs: Plugins & API` forum. The forum accepted it into moderator review and reported one pending post; it is not yet publicly visible, so no topic URL is available. The detailed cross-platform acceptance matrix remains internal in `evidence/m7/m7-p1-obsidian-api-containment-request-20261016.md`. This changes no product architecture; managed-note I/O stays disabled pending a supported API guarantee and physical-boundary proof.

**M7 P1 Mac portability follow-up (2026-10-16):** Owner-reported Mac observation run verified the synthetic vault, unchanged fixture/sentinel hashes, and runtime metadata (app/API 1.14.4, installer/plist 1.13.7, Electron 43.3.0, Node 24.18.1). The first transferred source passed syntax but local fixture tests were **16/18** because `socket.end()` retained half-closed sockets when the peer kept its write side open. The disposable observation probe now destroys each socket after consuming a complete status response; corrected Mac syntax and local fixtures passed **18/18**. The first ZIP manifest used CRLF so native `shasum -c` failed on a trailing carriage return; the owner normalized it and verified **6/6** file hashes. A packaging-only replacement ZIP now has LF manifest lines, local integrity and **6/6** payload hashes PASS; its source is identical to the Mac-verified package. Two reloads reported `create: 3`; disable and subsequent re-enable passed after re-enabling following Quit; no unload event occurred, so listener silence is not claimed. Fixture/sentinel hashes remained unchanged. Health IPC, real-daemon cleanup, and proposal authorization were NOT RUN; physical containment remains unresolved; managed-note writes remain disabled. See `evidence/m7/m7-p1-observation-portability-macos-20261016.md` and `evidence/m7/m7-p1-observation-portability-windows-20261016.md`. No Actions, commit, push, or merge.

**M7 portable observation-only follow-up (2026-10-16):** Diagnostic source and meaningful fixture checks live under `scripts/spikes/m7-obsidian-observation/`; this is not the product plugin or a write-back implementation. Windows syntax and 15 VM plus 3 local named-pipe fixture checks pass (**18/18**). The synthetic Windows Obsidian inventory recorded API 1.14.4, Electron 43.7.7, Node 24.21.0, `win32/x64`, and no API-types package (plain CommonJS); the temporary manifest minimum does not select product support. Managed-note/sentinel hashes stayed unchanged. The owner ran the initial package on the synthetic Mac vault: source hashes matched, runtime/startup/lifecycle observations were collected, fixture/sentinel hashes stayed unchanged, and local socket fixtures returned **16/18**. Two held-open-peer cases exposed half-close cleanup behavior; the observation probe now destroys response-complete sockets. Corrected Mac rerun is pending. The Mac endpoint mirrors `/tmp/cpx-cryptex-<uid>/s`; neither local fixtures nor optional status health prove peer authorization or real-daemon cleanup. Physical containment remains blocked; write-back stays off and M7 remains preparation only. See `evidence/m7/m7-p1-observation-portability-windows-20261016.md` and `evidence/m7/m7-p1-observation-portability-macos-20261016.md`. No Actions, commit, push, or merge.

**M7 owner reconfirmation and temporary-spike restriction (2026-10-16):** The owner explicitly selected "Mevcut planı koru; yazma kapalı kalsın". Retain the accepted Obsidian integration and containment gate; the proposed Desktop-only replacement was not selected. The existing Windows synthetic-vault spike still exposed old mutation commands, so its local `main.js` was restricted to event metadata observation, counts, and health IPC. Former managed-note/path probes now refuse even direct invocation; event observation no longer reads note bodies or classifies modifications as user proposals. Ordinary pending health sockets now participate in unload cleanup, early peer closure settles the request, and the response bound counts bytes. Syntax and **11/11** Node VM fixture checks PASS. A later owner-requested verification reran those checks and passed **3/3** additional actual Windows named-pipe fixture checks (held-open response peer, ordinary pending-health unload, and dedicated pending-probe reply closure); these are not real-daemon in-flight results. The installed source hash stayed unchanged during that verification. After an actual Obsidian 1.14.4 reload, accessibility showed `M7 spike: observation only; note IO disabled` and only three observation/status commands. Managed-note and sentinel hashes were unchanged. This is a temporary local spike correction, not an M7 product implementation or containment solution; real-daemon in-flight cleanup and macOS parity remain open. Artifacts, hashes, and limits: `evidence/m7/m7-p1-windows-writeback-spike-partial-20261014.md` (2026-10-16 observation-only restriction). No Actions, commit, push, or merge.

**M7 P1 update (2026-10-15):** Windows synthetic-only checks detected and preserved a manual edit between `cachedRead()` and `Vault.process()`; a Unicode/space/case-only create/rename/delete/missing-path API probe also passed. Exact path lookup worked while alternate-case lookup did not in Obsidian 1.14.4. `Vault.create()` followed a Windows junction from `Cryptex/` into an external synthetic directory; metadata-only `fs.lstatSync()` detected it but did not prove race-free containment. A standalone Windows Node preflight prototype rejected a static junction before its simulated Vault call and allowed a normal target. The actual temporary Obsidian plugin then ran its preflight on a fresh static junction and produced the block decision (reported target absent, external target empty). The probe contained no managed-note Vault mutation call; its vault_api_calls field is an explicit result value, not an intercepted-call counter. The plugin was disabled, exact fixture removed, and sentinel hash unchanged. Neither static check proves race-safe containment. Owner-reported macOS synthetic results also show manual-edit preservation and `Vault.create()` following a symlink out of `Cryptex/Entities/`. The Mac report is at source `87e9c4b0a49c5b670bdd7c6812d5a7c3fa768219`, an ancestor but not the same checkout tip as current M6 base `74bc3b14127962579ae03838bafea9336612a033`; its Obsidian version is unresolved (plugin plist 1.13.7, window title 1.14.4). Therefore path-prefix checks are insufficient on both reported platforms, and race-safe containment remains unresolved. Windows now has scoped startup and pending local-socket unload observations; the latter used a mock named pipe. P1 remains open for safe cross-platform containment, daemon peer/auth and real-daemon in-flight cleanup, macOS startup/unload parity, API-types/version inventory, and remaining repeatability/failure coverage. Windows startup now has scoped evidence: two pre-layout create events (Cryptex/Entities and Cryptex/Entities/synthetic-entity.md) appeared on consecutive reloads; attaching normal edit listeners at onLayoutReady excludes them. See the Windows evidence. Mac details: `evidence/m7/m7-p1-macos-writeback-spike-partial-20261015.md`; Windows details: `evidence/m7/m7-p1-windows-writeback-spike-partial-20261014.md`.

**M7 P1 transport robustness update (2026-10-15):** A local Node VM fixture loaded the temporary spike plugin and passed a valid response, malformed JSON, mismatched request ID, and no-response timeout. This is client transport robustness only, not daemon interoperability or peer-auth proof; details are in `evidence/m7/m7-p1-windows-writeback-spike-partial-20261014.md`. No product code or Actions changed.

**M7 P1 containment decision (2026-10-15):** Owner selected the conservative policy: managed-note write-back remains disabled until a Vault-API-compliant design proves physical containment on Windows and macOS, including symlink/reparse-point cases. Metadata-only preflight is insufficient; direct filesystem operations are not an allowed fallback. The policy is decided, but the technical proof is still open; M7 product implementation has not begun. See `docs/13_OBSIDIAN_INTEGRATION.md` §10, `docs/23_REQUIRED_TECHNICAL_SPIKES.md`, `docs/milestones/M7_OBSIDIAN_CONTROLLED_WRITEBACK.md`, and the P1 evidence plan.

**M7 public API review (2026-10-15):** The current official Obsidian API declarations/documentation expose path-based Vault/adapter operations but no documented no-follow or file-handle-bound containment guarantee. This does not prove a compliant design is impossible; no supported method is established yet. Keep the P1 gate open and write-back disabled pending a tested proof. See the P1 evidence plan for sources and limits.

**M7 daemon authorization review (2026-10-16):** Source review confirms `health.ping` is status-only and does not authenticate an OS peer PID; sensitive current IPC operations require the authenticated Desktop process and exact capability. No Obsidian role or proposal operation exists. The temporary plugin's health PASS is not a write-back route. No IPC architecture or daemon capability was added; see the P1 evidence plan.

**M7 Obsidian link behavior review (2026-10-16):** Official Obsidian help confirms symlinks/junctions may expose files outside a vault and warns of data loss/corruption risk. Combined with the Windows and reported macOS Vault API escape observations, a vault-relative path alone cannot satisfy the owner-selected physical-boundary gate. No supported per-subtree link-disable or no-follow API has been identified; this is a blocker, not proof that every future design is impossible. Details: P1 evidence plan.

**Snapshot date:** 2026-10-19
**Milestone transition (2026-10-15):** Owner approved M6 closeout after the final recorded checks. M6 is COMPLETE within its accepted scope at product `74bc3b14127962579ae03838bafea9336612a033`; the scoped review and its limits are recorded in `evidence/m6/m6-closeout-review-20261015.md`. M7 remains in preparation only. Windows P1 evidence includes plugin health IPC, Vault.process loop suppression, edit/create/rename/delete events, listener silence after plugin unload, manual-editor conflict preservation, path API cases, and a junction containment-risk observation. Owner-reported Mac evidence adds manual-editor conflict preservation and a symlink containment-risk observation, but came from ancestor source `87e9c4b0a49c5b670bdd7c6812d5a7c3fa768219`; the Obsidian version record conflicts (1.13.7 plist vs 1.14.4 window title). Both reports show that Vault APIs follow a link beyond the managed prefix; neither supplies a race-safe containment solution. The guarded Windows preflight also ran in the actual temporary Obsidian plugin and stopped before the Vault mutation API with zero mutation calls and no target file; it remains a static check with TOCTOU risk. A pending local named-pipe socket was closed on plugin unload in a Windows mock fixture; real-daemon in-flight cleanup remains unverified. Mac startup/unload parity, API-types/version baseline, safe path-boundary policy, and remaining repeatability/failure cases also remain open. No M7 product implementation has begun.
**M6 Windows/Mac follow-up (2026-10-14):** On the M6 working tree at HEAD `87e9c4b0a49c5b670bdd7c6812d5a7c3fa768219`, Windows fmt, projection 59/59, Desktop vault URI 2/2, daemon vault destination guard 1/1 and vault rebuild 2/2 tests, Desktop check, strict daemon/Desktop Clippy, and diff check PASS. This includes a cross-platform IPC path-comparison fix and generated-note target validation against symlinked generated directories. Owner-reported Mac fmt, projection 59/59, focused vault IPC 3/3, Desktop vault URI 3/3 (including Unix symlink test), strict Clippy, rebuilt app, and fresh unique synthetic-vault refresh/note-open/Graph/sentinel/clean-quit/lock-release PASS. Evidence: `evidence/m6/m6-windows-local-followup-20261014.md` and `evidence/m6/m6-macos-native-followup-20261014.md`. The subsequently defined entity-note/link model is implemented in candidate `74bc3b14127962579ae03838bafea9336612a033`. Windows focused verification: projection 64/64; reducer-to-vault mapping 1/1; guarded rebuild/stale-history hold 1/1; Desktop generated-note URI 1/1; fmt and strict Clippy for projection, daemon and Desktop PASS. Owner-reported Mac package and native synthetic projection-to-Obsidian Graph check PASS at the same candidate; the latter uses `rebuild_vault_with_knowledge` and does not exercise canonical history or the daemon retrieval-safety gate. Evidence: `evidence/m6/m6-entity-link-model-windows-verification-20261014.md` and `evidence/m6/m6-entity-relation-macos-native-synthetic-graph-20261014.md`. GitHub Actions run `38670407756` later passed macOS readiness, Windows readiness, and Windows IPC ACL at this exact SHA; the optional Windows package job was skipped. Evidence: `evidence/m6/m6-ci-verification-20261015.md`. This run was owner-authorized as a one-time exception. No commit, push, or merge. This entry predates the 2026-10-15 owner-approved M6 closeout recorded above.
**Current milestone:** M7 — Obsidian Controlled Write-back, preparation only. M6 is COMPLETE within accepted scope at `74bc3b14127962579ae03838bafea9336612a033`; final review evidence is `evidence/m6/m6-closeout-review-20261015.md`. The mandatory P1 Obsidian write-back spike is partially exercised on Windows and by an owner-reported Mac run. Manual editor conflict preservation was observed on both. Obsidian Vault APIs followed a junction/symlink outside the managed `Cryptex/` prefix on both reports; metadata-only lstat detection is not a race-safe guard. A static junction preflight was exercised both in a helper and in the actual temporary Obsidian plugin; neither result proves race-safe containment. The Mac report used ancestor source `87e9c4b0a49c5b670bdd7c6812d5a7c3fa768219` and has an unresolved Obsidian version discrepancy (plist 1.13.7, window title 1.14.4). Windows startup and pending local-socket closure on plugin unload have scoped evidence; the latter used a mock transport. P1 remains open for proof of the selected cross-platform containment policy, daemon peer/auth and real-daemon in-flight cleanup, macOS startup/unload parity, API-types/version inventory, and remaining repeatability/failure coverage. No M7 product implementation is authorized by these spike results. See `evidence/m7/m7-p1-obsidian-writeback-spike-plan-20261015.md`, `evidence/m7/m7-p1-windows-writeback-spike-partial-20261014.md`, and `evidence/m7/m7-p1-macos-writeback-spike-partial-20261015.md`. M5 is COMPLETE within its accepted scope following explicit owner approval on 2026-10-12; branch integration/merge has not been performed. Slice 3 native acknowledgement/archive/Quit PASS on macOS and Windows at product `a172d4bc45a2bd041a4e28ba6342a9ab9e62dd44`. Slice 4 derived catch-up/restart reconstruction is implemented at `7637ebc`; focused Windows and owner-reported macOS checks PASS. Slice 5 packaged Timeline/detail review is owner-reported PASS on Windows and macOS at product `ae6f5a64dbf2825080778462257068744197c501`; focused macOS/Windows CI PASS at `1f7420525824783417bdff37d5e79890e21a9961` (run `38275499486`). Scoped final DoD/security/UI review found no release blocker. Native first-to-second-page interaction passed with 21 synthetic notes on Windows and, by owner-supplied report, macOS in the existing packages. The native pagination gap is closed; the owner explicitly approved the final review and closeout in chat ("devam ke onayladim"). Bounded keyset paging has automated projection/IPC coverage. See `evidence/m5/m5-final-dod-security-ui-review-20261012.md`.
**Windows native pagination update (2026-10-12):** Existing packaged product `ae6f5a6` passed a fresh 21-note native pilot. Twenty numbered notes were saved through Inbox after daemon fixture seeding; Timeline showed 20 rows, Load older notes appended the final fixture without duplicates or lost rows, the exhausted control disappeared, and the appended note's detail/provenance matched. Clean app-menu Quit stopped Desktop/daemon and released the lock. The subsequent macOS report also passes this interaction; see the update below. No Actions or product changes. Evidence: `evidence/m5/m5-slice5-windows-native-pagination-pass-20261012.md`. This supersedes the Windows portion of the pagination limitation in the snapshot above.
**macOS native pagination update (2026-10-12):** Owner-supplied report at checkout `87e9c4b0a49c5b670bdd7c6812d5a7c3fa768219`, product `ae6f5a6`, macOS 27.0.1 arm64: 20 numbered Inbox notes plus fixture; 20-to-21 rows, newest-first order, no duplicates, exhausted control removal, matching fixture detail/provenance, and clean app-menu Quit PASS. Remote proof files were not independently inspected from Windows. Evidence: `evidence/m5/m5-slice5-macos-native-pagination-pass-20261012.md`. Both native pagination gates are now closed. No new Actions, rebuild, or test-suite rerun.
**M6 continuation:** P1 evidence supports copy-only derived attachment projection. Windows same-volume and C:→E: fallback passed; owner-supplied Mac report confirms same-volume copy/link and failure/interrupted-staging behavior; the separate-APFS cross-volume copy fallback now also passes. Hard-link mutation changed source bytes on both platforms, so M6 must not hard-link projected files. Decision: `evidence/m6/m6-p1-attachment-projection-decision-20261013.md`; reports: `evidence/m6/m6-p1-attachment-projection-windows-20261012.md`, `evidence/m6/m6-p1-attachment-projection-macos-20261013.md`, and `evidence/m6/m6-p1-attachment-projection-macos-cross-volume-20261014.md`. M6 has deterministic v1 note rendering, guarded staging/rebuild of the owned `Cryptex/` subtree, a copy-only synthetic attachment boundary, authenticated Desktop/daemon vault refresh with Open Vault/Graph integration, and deterministic representative-entity notes with directed explicit-relation WikiLinks. The daemon entrypoint verifies current retrieval safety and holds canonical locks through publication; regressions confirm stale history leaves the existing vault unchanged. Earlier Windows focused checks: projection 59/59, daemon knowledge-materialization 11/11. The latest link-model verification adds projection 64/64, reducer-to-vault mapping 1/1, guarded rebuild/stale-history 1/1, Desktop generated-note URI 1/1, fmt and strict Clippy for projection, daemon and Desktop. Evidence for earlier slices: `evidence/m6/m6-slice2-safe-vault-rebuild-and-copy-projection-20261013.md` and `evidence/m6/m6-slice3-desktop-vault-ipc-20261013.md`. Packaged Windows native pilot selected an isolated synthetic vault, refreshed one generated note, confirmed that exact UUID note opened in Obsidian, and opened Graph showing the generated note and test owner note. Pilot clean Quit was owner-approved; runtime confirms lock release and no remaining Cryptex processes. The link projection consumes M4 reducer output only through the existing shared `RetrievalSafetyState`; do not weaken or bypass ADR-0017, change the M4 model, or move M7/M9 responsibilities. A separate owner-reported Mac native synthetic check at `74bc3b14127962579ae03838bafea9336612a033` confirms two generated entity notes, one directed WikiLink, no reverse link, Obsidian note/Graph opening, and unchanged sentinel. It calls the projection library directly; it does not claim canonical-history or daemon safety-gate native coverage. GitHub Actions run `38670407756` passed the required macOS/Windows readiness and Windows IPC ACL jobs; optional Windows packaging was not selected. The owner subsequently approved M6 closeout on 2026-10-15; scoped review and limitations are recorded in `evidence/m6/m6-closeout-review-20261015.md`. No M7 write-back or M9 canonical blob backend was included in M6. The Actions run was a one-time owner-authorized exception, not standing authorization for future runs.
**Active branch:** `codex/m5-capture-timeline` (unchanged; no M7 branch created or switched to).
**Verified base:** `origin/m0/foundation` at M4 PR #10 normal merge
`a6c32bd30f883e546e2eb976787a59057fae211b`. The accepted M4 documentation and tested candidate are ancestors.
**M4 status:** COMPLETE within accepted scope and merged. Final owner-delegated
review: `evidence/m4/m4-final-review-20261010.md`; no independent human
line-by-line review is claimed. Corrected required CI
[38033047073](https://github.com/gungortarik/cryptex/actions/runs/38033047073)
PASS at `f9bd8945151a44d450bd8340a6794a5f33df3e72`, including explicit knowledge
and Desktop response-policy tests on Windows/macOS plus Windows IPC ACL.
**M5 Slice 1 contract:** Finalized in
`evidence/m5/m5-slice1-capture-reconciliation-contract-20261010.md`. Owner
authorized Slice 1 daemon capture service implementation only.
**Slice 1 verification:** Product fix `d206ea4dc213f56908807a7700c1f6542e14787a` has Windows focused PASS; owner-reported macOS PASS at exact HEAD `5d580a654d23f851e6853f0b297a8474ad87b747` (27.0.1 / 26A434 / arm64; capture 20, fake 6, writer 22, projection 17, fmt and strict Clippy). Evidence: `evidence/m5/m5-slice1-review-fix-windows-verification-20261010.md` and `evidence/m5/m5-slice1-macos-verification-20261010.md`. Slice 1 local platform gate is closed; final M5 CI/DoD acceptance is not claimed.
**Slice 2 verification:** Product fix `07005d93af58f7d2e9881bb62579ba69bb0e7e75` has Windows focused PASS; owner-reported Mac PASS at `44e686c829837bdfadc7f429a9ad331f01a8502f` (27.0.1 / 26A434 / arm64). Capture 20, restart 15 (1 helper + 14 scenarios), writer 22, projection 17; SecureStore Windows 17 / Mac 14. Format, non-test check and strict Clippy PASS. Evidence: `evidence/m5/m5-slice2-review-fix-windows-verification-20261010.md` and `evidence/m5/m5-slice2-macos-verification-20261010.md`. These are process-restart checks, not power-loss proof.
**Slice 3 verification:** Archive/Quit baseline e874c066ba011cf3c46516f3f4c9125d252abf80 remains ancestral. Explicit custom Quit at c3463bf2829be42d7bda41fdc3898ed4c759f1f plus single-owner menu routing fix b9b3e4cdfdc22077c5dd5ec78b0dbbf22969863f (tray Quit and app Quit each handled by exactly one global listener; Cancel prompts once). Windows focused PASS: exit lifecycle/menu 11; Quit guard 5; fmt; strict Clippy. Evidence ancestors: m5-slice3-app-menu-quit-windows-verification-20261011.md. No macOS native PASS claimed from Windows.
**Slice 3 Mac automated report:** Owner-reported PASS at exact reviewed HEAD `5a21c1ae8079a4d057f6facba85bc00421f128fa`, macOS 27.0.1 / 26A434 / arm64, clean before/after. IPC 30, capture 20, restart 15, writer 22, SecureStore 14, projection feed 17, projection crate 48, Quit guard 5, capture-policy 16, search-policy 2; fmt, strict Clippy, frozen installation, frontend build and diff check PASS. Evidence: `evidence/m5/m5-slice3-macos-automated-verification-20261010.md`. No native app launched; interactive behavior is not claimed.
**Native Mac review:** App-menu/Cmd+Q FAIL recorded at `3b0c6d4d0410ed963498b906c5da3e3967a6f82c` after ExitRequested-only fix (no confirm; daemon orphaned; muda `terminate:`). Tray Quit and close-to-tray/Dock reopen were previously PASS. Follow-up product `bc3463bf2829be42d7bda41fdc3898ed4c759f1f` replaces predefined Quit with guarded `cryptex-app-menu-quit` + `CmdOrCtrl+Q`; macOS native recheck not claimed from Windows.
**Corrected native Mac result:** Owner-reported scoped PASS at `3c17c7b50886139061f5ddc3651a19ea4d2f04a5`: app-menu and Cmd+Q Cancel/repeat/Confirm, close-to-tray/Dock reopen, physical tray Cancel/repeat/Confirm. Tray intermediate steps are owner observations, not AX-counted dialogs; Confirm cleanup is corroborated by process/status records. Focused Mac automated gates also PASS (lifecycle/menu 11, Quit guard 5, capture-policy 16, search-policy 2, fmt, strict Clippy, frontend/native build). Evidence: `evidence/m5/m5-slice3-macos-menu-owner-native-pass-20261011.md`.
**Windows native ack/archive/Quit:** Owner-reported scoped PASS at product `a172d4bc45a2bd041a4e28ba6342a9ab9e62dd44`, packaged from source `65b22417e64378dab0df10213523177888ea5225`. Fresh marker-only guest pilot verified ZIP SHA-256 `793430e6dae75b66cc72560ad246c5bae5f3e98171f4d11992ecd46398fb7336`; pending A `1283e391-33d4-49df-9f3b-9c2d73d2f116` was shown in Earlier unresolved captures with Resolve after deliberate-new B; owner reports Cancel preserved state, Quit No preserved app/session, and Quit Yes exited. Guest `Get-Process cryptex*` returned no processes. Supervisor/lifecycle files were not captured in this report; no independent lock-release claim. Evidence: `evidence/m5/m5-slice3-ack-dialog-windows-native-pass-20261011.md`. Optional close-to-tray/reopen with archive state remains unverified.
**Windows owner native update:** Product `eba337fcf2e81da8163254ff2092690f2a695ce3` launched in the guest. Owner reports X/tray reopen preserves draft, tray Quit No/retry prompts once and preserves draft, and tray Yes cleanly stops Desktop/daemon. Status output corroborates `clean_quit_from_tray` and released instance lock. Initial app-menu No preserved session; later clean app-menu shutdown is recorded, with final UI action ambiguous. Evidence: `evidence/m5/m5-slice3-windows-owner-native-quit-pass-20261011.md`.
**Mac portability update:** Owner-reported macOS PASS at HEAD `62342d88865d0ac0fe54d53b805c316daeea662c` (27.0.1 / 26A434 / arm64): fmt, `pilot_runtime` 8/8, and release strict Clippy. Product `eba337f` and menu-owner `b9b3e4c` are ancestors. No new Mac native Quit interaction was run. Evidence and archive gap: `evidence/m5/m5-slice3-macos-portability-and-archive-gap-20261011.md`.
**Mac archive UX attempt:** Owner-reported BLOCKED at source HEAD `62342d88865d0ac0fe54d53b805c316daeea662c`: synthetic Save A was rejected with pending state; Save B reached the unresolved notice, but the duplication acknowledgement did not appear/become actionable via System Events, Swift HID, or AXPress. No archive row was created and no archive Quit checks were attempted. Pilot PIDs 34358/34376 were reported alive. Evidence/screenshots location: `evidence/m5/m5-slice3-macos-archive-ux-native-blocked-20261011.md` (screenshots remain in the external pilot proof directory).
**Duplication acknowledgement fix:** Product `a172d4b` replaces the unresolved-save `window.confirm` with a Cryptex-owned accessible HTML dialog. Cancel preserves pending/archive state; confirmation rechecks the reducer state before sending the existing deliberate-new request. Focused verification: capture policy 16/16, Desktop build PASS, `git diff --check` PASS. macOS and Windows native acknowledgement/archive paths subsequently PASS; evidence: `evidence/m5/m5-slice3-duplication-ack-dialog-fix-20261011.md`, `evidence/m5/m5-slice3-ack-dialog-macos-native-pass-20261011.md`, and `evidence/m5/m5-slice3-ack-dialog-windows-native-pass-20261011.md`.
**Acknowledgement recheck:** Attempt at HEAD `be7d76d` was BLOCKED before exercising the new dialog: existing synthetic pilot Desktop 34358 / daemon 34376 owns the single-instance slot; fresh candidate launch restored that pilot and its new runtime never reached ready. Preserve its proof directory, close through supported Quit -> Confirm, then retry the candidate. Do not kill PIDs or remove runtime files. Evidence: `evidence/m5/m5-slice3-ack-dialog-native-blocked-single-instance-20261011.md`.
**Mac native acknowledgement/archive update:** Owner-reported PASS at source HEAD `c292d0902b3f387e0f3547fa7d165e18587fdcd0`, product `a172d4bc45a2bd041a4e28ba6342a9ab9e62dd44`, macOS 27.0.1 arm64. After cleanly closing the old pilot, fresh synthetic A→B showed the acknowledgement dialog; Cancel preserved A/B; Confirm archived A with Resolve; Quit Cancel kept both processes alive; Quit Confirm shut down cleanly. Evidence: `evidence/m5/m5-slice3-ack-dialog-macos-native-pass-20261011.md`.
**Slice 4 derived catch-up:** Implemented at product `7637ebc` after owner go-ahead. One bounded daemon worker schedules startup reconstruction, post-commit M3/M4 catch-up and retries when guarded search finds stale materialization. Windows-focused gates pass: IPC 34, projection 17, fmt and strict Clippy. Owner-reported focused macOS gates also pass at branch tip `80d534c` (product ancestor `7637ebc`): IPC 32, projection 17, fmt and strict Clippy. Evidence: `evidence/m5/m5-slice4-derived-catchup-windows-verification-20261011.md` and `evidence/m5/m5-slice4-derived-catchup-macos-verification-20261011.md`. Packaged/native parity remains OPEN; a search before asynchronous catch-up finishes can correctly return a retrieval hold.
**Slice 5 guarded timeline retrieval:** Owner authorized continuation with “yardir.” Windows implementation checkpoint adds a bounded newest-first keyset timeline for verified v1 note rows, safe 384-byte snippets, exact full-note detail by event ID, existing provenance details, and a minimal Desktop Timeline view. Daemon rechecks the existing Brain-wide retrieval gate for every page and detail request. Windows fmt, daemon `cargo check`, daemon/desktop strict Clippy, frontend build, projection 50/50, and IPC 39/39 PASS. macOS/native parity and Slice 5 closeout remain open. Evidence: `evidence/m5/m5-slice5-timeline-retrieval-windows-implementation-20261011.md`.
**Slice 5 Windows native follow-up:** Packaged release build and NSIS bundle succeeded at source `3e31da06e1cc8bb485352e13c6cca309eee8cf5e`; isolated marker-only pilot launched. Synthetic Save was rejected with pending ID `5818af28-1689-44b8-b7eb-f42811d5db13`; Timeline retrieval held because the fresh pilot had no verifiable accepted event history, so row/detail UX remains unverified. Quit No preserved the draft/pending state; Quit Yes cleanly stopped Desktop/daemon and released the lock. Evidence: `evidence/m5/m5-slice5-windows-packaged-native-review-20261011.md`.
**Slice 5 isolated fixture bootstrap:** Added `cryptexd pilot-fixture timeline-v1`. It requires explicit confirmation, an absolute fresh runtime containing only the exact pilot marker, and an available daemon instance lock; it initializes a synthetic Brain, enrolls in the OS SecureStore `dev` namespace, then creates one fixed public v1 note through the ordinary daemon capture/canonical-writer path. Success adds an exact `pilot-timeline-fixture.marker`, which lets Desktop accept the seeded but isolated runtime; an unmarked reused runtime remains rejected. The first packaged seed exposed Windows legacy MAX_PATH failure while persisting nested capture receipts; receipt filesystem I/O now uses extended-length paths, covered by a >260-character receipt round-trip test. Windows fixture test 3/3, capture 20/20, receipt 1/1, Desktop pilot-runtime 9/9, daemon and Desktop strict Clippy, fmt, and diff check PASS. Evidence: `evidence/m5/m5-slice5-isolated-timeline-fixture-20261011.md` and `evidence/m5/m5-slice5-windows-long-path-and-native-pass-20261012.md`.
**Slice 5 Windows packaged native review:** PASS at product `ae6f5a64dbf2825080778462257068744197c501`. Fresh marked fixture wrote a synthetic accepted v1 note through `cryptexd`; packaged Desktop Timeline displayed one row after derived catch-up, full-note detail and source provenance fields; File → Quit Confirm exited Desktop/daemon and released the instance lock. Exact tested executable and NSIS hashes, PIDs, and checks are in `evidence/m5/m5-slice5-windows-long-path-and-native-pass-20261012.md`. The >260-character receipt regression is covered. Timeline paging beyond one note is not established.
**Slice 5 macOS packaged native review:** Owner-reported PASS after the requested Timeline row, full-note/provenance, and clean-quit review. App was opened from `$CARGO_TARGET_DIR/release/bundle/macos/Cryptex.app` with the isolated `timeline-v1` fixture under `/var/folders/x6/4s8wgb5s1sv3btr0kdzhcnnr0000gn/T/cryptex-m5-s5.jhEpFO/runtime`; Brain/event IDs were only partially reported. No screenshot, process snapshot, or lifecycle files were supplied, so those details are not independently corroborated. Evidence note: `evidence/m5/m5-slice5-macos-native-pass-20261012.md`. Native paging beyond the first 20 results remains unexercised.
**Authorization / next action:** Slice 3 native acknowledgement/archive/Quit is PASS on both OSes at product `a172d4b`. Slice 4 focused Windows and owner-reported macOS checks are PASS at product `7637ebc`. Slice 5 Windows and macOS owner-reported packaged Timeline/detail checks PASS at product `ae6f5a64dbf2825080778462257068744197c501`. A fixture-test directory-order portability fix is at `1f7420525824783417bdff37d5e79890e21a9961`; Windows focused tests and required CI run `38275499486` PASS on Windows/macOS. Scoped final DoD/security/UI review is recorded at `evidence/m5/m5-final-dod-security-ui-review-20261012.md`. Before M5 can be declared complete, the owner must review and accept the closeout and its disclosed limit: multi-page Timeline interaction was not natively exercised. Do not start M6 until that completion gate is explicitly closed. No merge, production v2 activation, or M5-complete claim yet.
**Continuing invariants:** v1 memory.note unchanged; production v2 writing OFF;
canonical writer daemon-only; derived SQLite rebuildable; RetrievalSafetyState
holds apply to all content-returning paths. M4 visual/security limitations remain
explicit in its final review. Do not implement M6+ features opportunistically.

## Historical M5 preparation snapshot (superseded)

The preparation-only next-action and authorization wording below is superseded by
the Slice 1 authorization above and the finalized contract evidence.

**Next action (historical):** Windows Cursor should sync this exact branch only from a clean
checkout, read `evidence/m5/m5-kickoff-plan-20261010.md` and return the concrete
Slice 1 capture-service/failure-semantics proposal. English instruction:
`evidence/m5/m5-cursor-preparation-prompt-20261010.md`.
**Authorization (historical):** M5 preparation only. No M5 product code or new dependencies,
no Actions run for planning. Production implementation needs explicit approval.

## Historical M4 continuation snapshot (superseded)

The entries below retain their original capture-time statuses. Pending CI,
owner OPEN, M4 IN PROGRESS and do-not-start-M5 instructions in this historical
snapshot are superseded by the current M5 continuation above and the final review.

**M4 review packet:** Draft [PR #10](https://github.com/gungortarik/cryptex/pull/10) targets `m0/foundation`; the technical acceptance audit and diff are prepared. Git reports MERGEABLE (no merge-conflict finding), not approval. Corrected CI run `38033047073` and final human review remain the closeout gates. No merge, M4 COMPLETE claim, M5 work or production v2 activation is authorized by this status note.
**Corrected M4 CI run:** [38033047073](https://github.com/gungortarik/cryptex/actions/runs/38033047073) was dispatched once at exact candidate `f9bd8945151a44d450bd8340a6794a5f33df3e72`, containing CI coverage fix `9500e2f`. Both readiness jobs and Windows IPC ACL are running; result PENDING. This follow-up changes workflow coverage/documentation only; product-code baseline remains `b1c43e2`. Final human review is OPEN; M4 is not complete or merged.
**Final M4 DoD audit update (takes precedence over prior CI gate wording below):** Reviewed tip `9ab75af` and verified run `38021522882` succeeded at product-code SHA `b1c43e2`. Found a real CI coverage gap: readiness did not execute `cryptex-knowledge` unit tests (including interrupted SQLite migration) or Desktop `test:search-policy`. Added both existing test commands to the Windows/macOS readiness jobs; focused Windows verification PASS (knowledge 35/35, response policy 2/2). No production code or test assertions changed. Final M4 CI acceptance is PENDING corrected coverage. macOS retrieval-safety visual remains scoped PASS; status-item Quit remains NOT VERIFIED. Final human review remains OPEN. Audit and acceptance mapping: `evidence/m4/m4-final-dod-audit-20261010.md`. M4 remains IN PROGRESS; production v2 activation stays CLOSED.
**Current continuation update:** macOS native retrieval-safety visual review scoped PASS at documentation tip `ebdd8c975c103eb437c035fdda70956b9c9be4ef` (product-code SHA covered by final CI: `b1c43e2c36534e360cb06fb5cacef5bf6fd210ee`). Host macOS 27.0.1 (26A434) arm64; Tauri `tauri:package --bundles app` passed; isolated empty pilot consumed `show.request` with `window_visible=true`; synthetic `m4-visual-smoke-test` showed the expected RetrievalSafetyState hold and no results (`proof/02-hold.png` retained outside Git). Clean shutdown used documented `pilot-quit.request` (both PIDs exited, `clean_quit` recorded, `open -W` exited 0); status-item tray **Quit Cryptex** was NOT VERIFIED because Accessibility could not open the status-item menu. Evidence: `evidence/m4/m4-macos-native-visual-review-20261010.md`. Windows visual PASS remains at `79e03d6` (`evidence/m4/m4-windows-native-visual-review-20261009.md`). Final dual-OS CI PASS at `b1c43e2` in run [38021522882](https://github.com/gungortarik/cryptex/actions/runs/38021522882); evidence: `evidence/m4/m4-final-ci-gate-20261010.md`. Production v2 writing remains disabled. M4 remains IN PROGRESS; final DoD/owner review remains open.
**Current milestone:** M4 - Knowledge Engine (ADR-0017 documentation acceptance integrated at `ee7e173`; Slices 1â€“4 and the Slice 5 reader/rebuild/retrieval-safety library are locally verified on Windows/macOS at their recorded SHAs; guarded Desktop retrieval integration plus UI hardening is at `652b4de`; Windows and macOS Rust/Tauri/frontend checks are recorded at their respective SHAs; IPC peer-auth hardening at `946642a459531247091e858c301fc7ea17835ff0` binds sensitive retrieval/enrollment to the live parent Desktop PID and expected sibling executable path (Windows retained process handle; macOS current-parent check). Follow-up `37924852ad2207f0ab618c6737e5e08f0a6907c8` moves authenticated Desktop connections off the listener, bounds Desktop connections at eight and expensive full-history searches at two, preserves a separate four-worker non-Desktop health pool, and applies one 15-second deadline across lock waits, verification, reduction and SQLite search; timeout returns a Brain-wide hold without partial hits. A further startup fix at `b6d652da07c175d086c625bfb498cf370dfc5fe2` keeps non-Desktop launches on health-only IPC while sensitive retrieval stays unavailable unless parent Desktop authentication succeeds. Windows `cryptexd` package tests pass 198/198; owner-reported macOS checks on the same code SHA pass (193 daemon package tests, crate tests, strict Clippy and Desktop check). Path identity is not code-signature identity and assumes the installed sibling executable cannot be replaced by an attacker; on that report, visual review and final DoD/security review remained open. Reducer/IPC security review passes at `c2cf07b`; the follow-up Windows overlapped-I/O review at `f8e7636` found no P1/P2; Windows native visual review passes at `79e03d6`; macOS retrieval-safety visual review scoped PASS at `ebdd8c9` (tray status-item Quit NOT VERIFIED); final DoD review and owner acceptance remain open)
**M4 reducer security review (previous slice):** `c2cf07bf0662589a9932c201f5e9e749e6a1013a` is pushed on `m4/knowledge-engine`. Windows fmt, knowledge 34/34, IPC 14/14, full cryptexd 199/199, strict Clippy, Desktop check, and diff check PASS. Owner-reported macOS verification on the exact SHA also passes: knowledge 34/34, cryptexd 194/194, fmt, strict Clippy, Desktop check, and diff check. The read-only security re-review found no remaining P1/P2 issue in the changed reducer/IPC code. This SHA adds cancellation-aware deterministic union-find/alias/revocation reduction and bounded search execution. Record: `evidence/m4/m4-security-hardening-cancellable-reducer-20261008.md`.
**Historical Windows native visual review attempt (superseded):** At source HEAD `b289a943a3651c993ff2b17227fe0e43c20c6d6c`, Smart App Control blocked the normal Tauri build with OS error `4551`; no native window was launched. Evidence: `evidence/m4/m4-windows-native-visual-review-attempt-20261008.md`. The current-source Windows review later passed as recorded in `evidence/m4/m4-windows-native-visual-review-20261009.md`.
**Windows IPC closeout (historical):** code `6855c57766c018339a781fabc71a11a810905398` fixes the Desktop health/search failure seen while cryptexd was alive. Windows named-pipe `PIPE_NOWAIT` made a not-yet-ready read look like end-of-stream, so the renderer showed Attention and the generic guarded-search error even though `lifecycle.status` said `health=active`. Windows `cryptexd` library tests passed 185/185, strict Clippy passed, and a debug native window reached Active health and a fail-closed retrieval hold instead of the generic IPC error. Evidence: `evidence/m4/m4-windows-ipc-pipe-roundtrip-20261009.md`. The earlier final dual-OS CI in run `37987708059` at `7aaa08a` was superseded by run `38018083958` at `f8e7636`, which was itself superseded by the latest run `38021522882` at `b1c43e2`. Latest gate details are in `evidence/m4/m4-final-ci-gate-20261010.md`. macOS retrieval-safety visual review is scoped PASS at `ebdd8c9` (`evidence/m4/m4-macos-native-visual-review-20261010.md`); final DoD/owner review remain open. Production v2 writing stays disabled. Do not claim M4 complete.
**Active branch:** `m4/knowledge-engine`, based on M3 merge commit `35538c144434e1dbfb97b47f89d2536b7646046a` (PR #9)
**M2 status:** COMPLETE within M2 scope after owner-delegated review and required verification; normal merge PR #8 integrated `m2/immutable-event-store` into `m0/foundation` at `817b6a1`.
**M3 closeout:** Complete within accepted scope; normal merge PR #9 integrated `m3/local-projection-search` into `m0/foundation` at `35538c1`. Final review: `evidence/m3/m3-final-review-20261008.md`.
**M4 architecture:** ADR-0017 was accepted and integrated at `ee7e173`; ADR-0018 local-only Desktop retrieval identity was accepted by the owner on 2026-10-08. Follow `docs/milestones/M4_KNOWLEDGE_ENGINE.md` in order. Slices 1â€“4 and the Slice 5 reader/rebuild/retrieval-safety library are Windows/macOS locally verified at their recorded SHAs. Explicit daemon-owned local DeviceId/key enrollment, read-only local key resolution, and `identity enroll-local` setup are implemented. Guarded Desktop search IPC plus renderer integration and empty/stale-query hardening are at code `652b4de` / branch `9eea611`; Windows and macOS frontend checks pass, with the latest macOS report at exact HEAD `9eea611` (`test:search-policy` 2/2, frontend build, `git diff --check`). Mac Rust/Tauri checks passed at `de1616c`; later shared Rust behavior was owner-verified on macOS at `c2cf07b`. The security review findingsâ€”caller-supplied Desktop role not bound to the connecting process and serial IPC framing without a total deadlineâ€”are addressed at `16cb39b` (Darwin enum/raw-FD compile correction is included in the current candidate); non-Desktop health traffic has its own bounded four-worker pool; authenticated Desktop connections have eight bounded slots, with a separate two-worker search cap and one 15-second search deadline so slow searches do not block the listener or exhaust the health pool. Windows tests and strict Clippy pass; owner-reported macOS peer-credential/full parity passes at `946642a459531247091e858c301fc7ea17835ff0`. Desktop identity uses an expected sibling executable path, not signature/hash verification, and assumes install-directory protection. Reducer/IPC scoped security review passes at `c2cf07b`; the follow-up Windows overlapped-I/O review at `f8e7636` found no P1/P2, with rare OS/API cancellation failure potentially delaying safe drain. Windows native visual review passes at `79e03d6`; macOS retrieval-safety visual review scoped PASS at `ebdd8c9` (status-item tray Quit NOT VERIFIED; see `evidence/m4/m4-macos-native-visual-review-20261010.md`). The latest dual-OS Actions run [38021522882](https://github.com/gungortarik/cryptex/actions/runs/38021522882) passes at product-code SHA `b1c43e2`; tip `ebdd8c9` is documentation-only after that SHA. Evidence is in `evidence/m4/m4-final-ci-gate-20261010.md`. Next: final DoD and owner acceptance. Do not add M8 pairing/retirement, infer trust from the two machines' ownership, or enable production v2 writing. The future sync/import actor must join the canonical inventory-lock protocol. Phase-0 alternatives remain superseded audit records.
**M4 Slice 1 (historical status at initial landing):** code commit `2a382eb876d5646ff3e504dc3918423fa1594735` added the strict versioned reader and compatibility boundary. Windows checks and owner-reported macOS 27.0.1/arm64 checks passed at the recorded SHA. RetrievalSafetyState enforcement, which was open then, is implemented later through `ecbbaf7`; production v2 writing remains disabled.
**M4 Slice 2:** code commit `70dba01ff8fdb4018f6eb9be55d41943d300e90e` adds the strict typed `knowledge.assertion` v2 payload validator, separate generated v2 JSON Schema/TypeScript, and routes known v2 records to a typed supported classification. Windows and owner-reported macOS checks pass on this code SHA. Evidence: `evidence/m4/m4-slice2-typed-v2-payload-schema-20261008.md`. V1 artifacts and validator are unchanged; production v2 writing remains disabled.
**M3 Slice 1:** `evidence/m3/m3-slice1-workspace-scaffolding-20261008.md` - `cryptex-projection` path scaffolding + focused layout tests; no SQLite deps.
**M3 Slice 2:** `evidence/m3/m3-slice2-sqlite-fts5-pin-20261008.md` - `rusqlite =0.40.2` `default-features=false` + `bundled`; FTS5 fail-closed smoke; no schema/ingest.
**M3 Slice 3:** `evidence/m3/m3-slice3-projection-schema-v1-20261008.md` - empty schema v1 (`events` + text-only FTS5 + meta/checkpoint); open/create + schema-version validation; no ingest/search.
**M3 Slice 3 follow-up:** `evidence/m3/m3-slice3-fixture-schema-hardening-20261008.md` - exclusive temp fixtures + exact column/FTS shape validation; no ingest/search.
**M3 Slice 4:** `evidence/m3/m3-slice4-verified-feed-integration-20261008.md` - `for_each_verified_event` + transactional `project_verified_device_events`; no search/checkpoint advancement.
**M3 Slice 4 follow-up:** `evidence/m3/m3-slice4-idempotent-conflict-feed-errors-20261008.md` - identical replay idempotent; `EventConflict` on field mismatch; `ProjectionFeedError::Projection` preserved for insert failures.
**M3 Slice 5:** `evidence/m3/m3-slice5-rebuild-incremental-checkpoint-20261008.md` - rebuild + incremental catch-up; checkpoints in same txn; missing/corrupt/unsupported/checkpoint-invalid rebuild disposable only.
**M3 Slice 5 follow-up:** `evidence/m3/m3-slice5-replace-only-unusable-20261008.md` - `open_replacing_unusable` deletes only CorruptDb/UnsupportedSchema/SchemaInconsistent; Io/Sqlite/Fts5Unavailable preserve files.
**M3 Slice 5 blocking fix:** `evidence/m3/m3-slice5-device-canonical-lock-reader-20261008.md` - verified reader shares `writer.lock`; projection lock order deviceâ†’SQLite; no concurrent recover/truncate during append.
**M3 Slice 5 lock follow-up:** `evidence/m3/m3-slice5-device-lock-visibility-boundary-handshake-20261008.md` - lock helpers `pub(crate)` + device-bound; boundary check before FS mutate; thread-scoped reader-at-lock handshake.
**M3 Slice 6:** `evidence/m3/m3-slice6-search-api-20261008.md` - `search_events` FTS + conjunction metadata filters; UTC `created_at` ranges; `InvalidQuery` for bad FTS/bounds.
**M3 Slice 6 follow-up:** `evidence/m3/m3-slice6-fractional-instant-precision-20261008.md` - exact UTC instants (incl. fractional seconds) for search range + ordering via `rfc3339_utc_instant`; no new stored column.
**M3 Slice 7:** `evidence/m3/m3-slice7-cross-platform-ci-wire-20261008.md` - CI readiness wires `cryptex-projection` only; `cryptexd` `projection::` covered by existing full `cargo test -p cryptexd` (duplicate filtered step removed); sealed/open + multi-device + idempotent rebuild matrix coverage; Windows-local PASS.
**M3 Slice 7 macOS four-check:** `evidence/m3/m3-slice7-macos-four-check-20261008.md` - owner macOS 27.0.1 arm64 four-check PASS at exact HEAD `428b03f33eda8fbf645678220dbd3ce3d88d108e` (fmt; projection 35; cryptexd `projection::` 14; Clippy `-D warnings`). Both OS local checks PASS at that tip.
**M3 Slice 7 tip (both-OS local PASS):** `428b03f33eda8fbf645678220dbd3ce3d88d108e` (behavior through CI redundancy cleanup; Windows + macOS local four-checks recorded)
**M3 final dual-OS CI:** PASS in run [37720627242](https://github.com/gungortarik/cryptex/actions/runs/37720627242) at exact branch tip `51e6dc772757510e08c14ef565a87534a8a9478b` (documentation-only after tested behavior SHA `428b03f`). Windows and macOS readiness plus Windows IPC ACL PASS; unsigned package skipped by design. Record: `evidence/m3/m3-final-ci-gate-20261008.md`.
**Historical M3 Slice 8 initial tip (Windows-local at the time; superseded):** `10a11c82b0e07e09fd7ef97c2384e9a385991adc`
**Historical M3 Slice 8 follow-up tip (Windows-local at the time; superseded):** 04c794f907704455cea2463f13cbce83635940ff
**Historical M3 Slice 8 constraint-class tip (Windows-local at the time; superseded):** `7043c1f8633beebe5f43dfc313e7d72a8c682366`
**M3 Slice 8 constraint-class fix:** `is_constraint_rejection` is ConstraintViolation-only (no message substring).
**M3 Slice 8 verified candidate:** `275fb7acf0c2c7b4d7c050acb54670e81520377f`; Windows and owner macOS local checks PASS; final dual-OS workflow [37725223487](https://github.com/gungortarik/cryptex/actions/runs/37725223487) PASS. Evidence: `evidence/m3/m3-slice8-macos-four-check-20261008.md` and `evidence/m3/m3-final-ci-gate-20261008.md`.
**Historical M3 Slice 8 meta-CHECK tip (Windows-local at the time; superseded):** 52fe953ecdb29db1f1730148921f1af07be5f50c
**M3 Slice 8 meta-CHECK hardening:** structural `id=1` parse + multi-id constraint-only probes; non-constraint probe errors non-replaceable; savepoint cleanup errors preserved. See evidence follow-up.
**M3 Slice 8 follow-up:** FTS integrity-check maps only confirmed corruption to replaceable `CorruptDb`; semantic `projection_meta` id CHECK probe rejects permissive constraints. See evidence follow-up section.
**M3 Slice 8 (security/DoD findings):** `evidence/m3/m3-slice8-security-dod-findings-20261008.md` â€” FTS5 external-content `integrity-check` (rank=1) on open; empty-device checkpoint incremental catch-up; schema v1 type/PK/CHECK validation; bounded clear + limited search; error classifications use SQLite result codes only. Windows checks and owner macOS four-check PASS at exact HEAD `4a2ea56`; final dual-OS CI PASS in run `37780668640`. Scoped security/DoD and owner-delegated final review PASS are recorded in `evidence/m3/m3-final-review-20261008.md`; M3 completion gate is PASS within scope.
**Latest verified candidate SHA:** `9ade9f7db18ec0e383594c19e592809e3b194531` (`9ade9f7`), on `m2/immutable-event-store`.
**Final dual-OS CI:** PASS in run [37706283147](https://github.com/gungortarik/cryptex/actions/runs/37706283147) against the exact candidate SHA above. macOS and Windows readiness plus Windows IPC ACL succeeded; unsigned Windows package skipped by design. Record: `evidence/m2/m2-final-ci-gate-20261008.md`.
**Closeout audit tip (L1/L2/F4â€“F6 + staging ownership):** behavior `b5c9d69`, tip `1a26493` â€” `evidence/m2/m2-staging-cleanup-ownership-20261007.md`. Windows checks PASS; owner-reported macOS four-check PASS at `9ade9f7` is recorded in the final-gate evidence. Owner-delegated closeout review PASS is recorded at `evidence/m2/m2-owner-delegated-review-20261008.md`; PR #8 integration completed at `817b6a1`. `LongPathsEnabled=0` is not claimed.
**Base:** `origin/m0/foundation` @ `817b6a126cf6e9cd3e8a27b1348ec977bdc9dcdd` (M2 merge / PR #8)
**M1 merge:** `c2168f3edbd4deb00d5e16f5f35643948028b0ca` (PR #6) â€” integrated ancestor
**M1 implementation source:** `a39920564561f75ada08cba8a47244bec9074345`
**M1 documentation closeout:** `0c84c5c6a2872480ac4bbc82ebb1541773ddfb75`
**M3 Slice 8 corruption-class candidate:** code SHA `6b61882dce0814abd51fbf063c69d7775c483e37`; exact branch HEAD with docs stamps `4a2ea56aa2ddc4024d82599f8dc671472179cbe8`. Windows local checks PASS; owner macOS four-check PASS at exact HEAD; final dual-OS Actions PASS in [run 37780668640](https://github.com/gungortarik/cryptex/actions/runs/37780668640). Evidence: `evidence/m3/m3-slice8-security-dod-findings-20261008.md`, `evidence/m3/m3-slice8-macos-four-check-20261008.md`, and `evidence/m3/m3-final-ci-gate-20261008.md`.
**M3 closeout:** Completion gate PASS within accepted scope, recorded in `evidence/m3/m3-final-review-20261008.md`. Normal merge PR #9 integrated M3 into `m0/foundation` at `35538c1`.
**Historical M4 Phase 0 records:** `evidence/m4/m4-phase0-kickoff-plan-20261008.md` and `evidence/m4/m4-phase0-input-and-semantic-decision-review-20261008.md`. Their open alternatives are superseded by accepted ADR-0017; see their supersession notices.

> This file is an operational status snapshot, not an architecture authority. If it conflicts with `docs/00_MASTER_SPEC.md`, accepted ADRs, the milestone contract, or other authoritative project documents, stop and report the conflict instead of silently reconciling it.

## Required reading before continuing

1. `AGENTS.md`
2. `docs/00_MASTER_SPEC.md`
3. `docs/milestones/M5_CAPTURE_TIMELINE.md`
4. `docs/03_MEMORY_MODEL.md`
5. `docs/04_EVENT_SCHEMA_AND_INVARIANTS.md`
6. `docs/06_STORAGE_ARCHITECTURE.md`
7. `docs/17_TEST_AND_FAILURE_MATRIX.md`
8. `docs/21_DEFINITION_OF_DONE.md`
9. relevant accepted ADRs, including ADR-0013/0015/0016/0017/0018; UX design system `docs/14_DESKTOP_UX_AND_DESIGN_SYSTEM.md`
10. `evidence/m5/m5-kickoff-plan-20261010.md` and M4 final review/CI evidence under `evidence/m4/`

## Working agreement

### M2 closeout audit (L1/L2/F4â€“F6) â€” 2026-10-07

Source re-audit of previously deferred Low findings (see `evidence/m2/m2-closeout-l1-l2-f4-f6-20261007.md`):

- **L1 FIXED:** corrupt sealed `.seg` isolated from canonical tree only after durable quarantine; repeated load does not re-quarantine.
- **L2 NON-BLOCKER:** Desktop links `cryptexd` but only health IPC + sidecar process; Cargo features cannot guarantee isolation under workspace unification; Master Spec single-writer preserved by process/IPC/call sites.
- **F4 partial:** sealed integrity errors use `source_relative`; residual lifecycle absolute paths NON-BLOCKER.
- **F5 NON-BLOCKER:** quarantine containment by construction under `quarantine/canonical-events/`.
- **F6:** staging orphan cleanup FIXED; canonical `\\?\` remains addendum non-goal NON-BLOCKER.

Windows: `cryptex-storage` quarantine 24 ok; `cryptexd --lib` 126 ok; fmt check ok; clippy `-D warnings` on storage+cryptexd ok. **macOS four-check PASS** at source `9ade9f7`: quarantine 22, cryptexd lib 121, fmt and strict Clippy all passed on macOS 27.0.1 arm64. Final dual-OS CI run [37706283147](https://github.com/gungortarik/cryptex/actions/runs/37706283147) also PASS at exact SHA `9ade9f7`; see `evidence/m2/m2-final-ci-gate-20261008.md`. Owner-delegated closeout review PASS; M2 is complete and integrated via PR #8 at `817b6a1`. M3 Phase 0 plan is complete on `m3/local-projection-search` (`evidence/m3/m3-phase0-implementation-plan-20261008.md`); product implementation not started.

**Staging ownership fix (2026-10-07):** Arm `StagingCleanup` only after exclusive `create_new`; surface cleanup failures as `QuarantineCleanupFailed` without dropping the primary. Evidence: `evidence/m2/m2-staging-cleanup-ownership-20261007.md`. Windows quarantine **24** ok; cryptexd lib **126** ok; fmt/clippy ok. Mac four-check PASS and final dual-OS CI PASS are recorded in `evidence/m2/m2-final-ci-gate-20261008.md`.




**Historical M1 follow-up note (2026-10-07; superseded for owner-acceptance status):** Codex final review found fixed-temp races in
pairing and Recovery Kit public metadata persistence and corrected them with
serialized, synced atomic replacement. Sequence counter device-id validation now
also covers reads/initialization, not only allocation. Windows tests/format PASS; local Clippy
was blocked by Smart App Control (4551). Current-source cross-platform CI is
PASS at `a39920564561f75ada08cba8a47244bec9074345`, run 37569756071, including
strict workspace Clippy on both OSes. See
`evidence/m1/m1-public-metadata-final-review-20261007.md` and the newer
`evidence/m1/m1-final-review-20261007.md`. **Historical wording (superseded):** this paragraph originally said "Owner acknowledgment remains OPEN." That status is obsolete: the owner accepted the delegated Codex review on 2026-10-07; see the later M1 review update and `evidence/m1/m1-final-review-20261007.md`.

**Latest M1 review update (2026-10-07):** A final Codex review found that loss of
an initialized sequence counter could permit reuse after `initialize()` recreated
it at zero. Fixed at source `a39920564561f75ada08cba8a47244bec9074345` by committing
a durable per-device initialization marker before the initial counter; marker plus
missing counter now fails closed. Current-source final CI run
[37569756071](https://github.com/gungortarik/cryptex/actions/runs/37569756071) is
PASS (Windows/macOS readiness + Windows IPC ACL; unsigned package skipped by design).
Review record: `evidence/m1/m1-final-review-20261007.md`. The owner accepted the
delegated review on 2026-10-07; PR #6 was normally merged into `m0/foundation` as
`c2168f3`. M1 is complete within its defined scope. M2 has not started.

**Current status (2026-10-07):** M0 Foundation is COMPLETE (PR #5). M1 â€” Brain +
Device Identity is COMPLETE and integrated into `m0/foundation` at merge commit
`c2168f3edbd4deb00d5e16f5f35643948028b0ca` (PR #6). The behavior-bearing source
candidate `a39920564561f75ada08cba8a47244bec9074345` passed required Windows/macOS CI;
subsequent M1 commits were documentation-only review/acceptance records. Cursor was
the write-capable M1 implementer; the owner accepted Codex's delegated final technical
review on 2026-10-07.
**ADR-0015 is ACCEPTED**; **ADR-0014 is ACCEPTED** (2026-10-06) selecting **P1**
(direct platform APIs) with product pins `windows-sys = 0.61.2`,
`widestring = 1.2.1`, `security-framework = 3.7.0`. K2 and high-level `keyring`
are unselected. Gate C **Windows** and **macOS** product SecureStore are VERIFIED
(distinct host evidence under `evidence/m1/`). Required CI **PASS** on implementation
candidate `a399205`: run [37569756071](https://github.com/gungortarik/cryptex/actions/runs/37569756071)
(Windows readiness, macOS readiness, Windows cross-account IPC ACL; unsigned Windows
package job skipped by design). The owner accepted Codex's delegated technical review
on 2026-10-07; all M1 gates are satisfied and PR #6 is merged. The unsigned Windows
package was intentionally not part of the M1 gate.

**M1 status:** ADR-0015 ACCEPTED (2026-10-05). ADR-0014 ACCEPTED (2026-10-06, P1).
Identity/sequence, SecureStore (trait/fake + Windows/macOS P1 adapters), synthetic
Brain daemon wiring, pairing-record persistence, and Recovery Kit public metadata
persistence are in place under cryptexd. Gate C Windows and macOS product SecureStore
are both VERIFIED (2026-10-06; distinct host evidence files under `evidence/m1/`).
Gate A spike remains historical comparison only. Prior Windows tests, strict Clippy and format
for the install-identity staging/rollback follow-up pass locally (94 cryptexd tests);
The latest metadata and sequence corrections are verified by both-OS CI; Unix-specific
install rollback-sync tests and Windows/macOS readiness passed in CI run
37569756071. Codex final review and owner acceptance are recorded; PR #6 is integrated.
Production pairing/revocation remains M8; Recovery Kit container remains M11.
No custom cryptography.

**M1 audit (2026-10-06):** Sequence allocation and install identity are daemon-owned
(`evidence/m1/m1-scope-security-audit-20261006.md`). Brain public identity first-create is exclusive/no-replace; install-identity staging
cleanup and Unix rollback durability now match that hardened path (see audit). Final CI
PASS on candidate `a399205` is recorded (run 37569756071). The accepted M1 lifecycle and placement boundary is documented in
ADR-0015 Â§2.1.1 and the audit. The owner accepted the delegated Codex review; PR #6 is
merged. M1 is complete; M2 has not started.

**M2 closeout record (2026-10-08):** M2 productization was completed and integrated via PR #8 at `817b6a126cf6e9cd3e8a27b1348ec977bdc9dcdd`. The implementation is documented below as historical milestone context.
`m2/immutable-event-store`, based on `origin/m0/foundation` at `0c84c5c` (M1
documentation closeout). M1 merge `c2168f3` is an ancestor of the latest final-CI behavior tip `9ade9f7`. Earlier final-CI behavior tip `16f09a3` and its documentation closeout `7a84bdd` are historical; the latest gate is recorded in `evidence/m2/m2-final-ci-gate-20261008.md`. Owner-delegated closeout review PASS; M2 awaits normal integration. Cursor is
the sole write-capable M2 implementer. ADR-0016 is **ACCEPTED** (2026-10-07) at
`5cc89b709dcf236698c3253b8582c873de7c7f0f`. Slice 1 scaffolding and framing body_len enforcement are complete. Slice 2 event domain types +
schema/TS generation is recorded under `evidence/m2/m2-slice2-event-types-schema-20261007.md`; integrity primitives under `evidence/m2/m2-slice3-integrity-primitives-20261007.md` signing-key lifecycle under `evidence/m2/m2-slice4-signing-key-lifecycle-20261007.md` ; segment I/O under `evidence/m2/m2-slice5-segment-io-quarantine-20261007.md` ; durability hardening under `evidence/m2/m2-slice5b-storage-signing-hardening-20261007.md` ; canonical writer under `evidence/m2/m2-slice6-canonical-writer-20261007.md` ; writer transaction hardening under `evidence/m2/m2-slice6b-writer-transaction-hardening-20261007.md` ; schema validation/quarantine matrix under `evidence/m2/m2-slice7-schema-validation-quarantine-matrix-20261007.md` ; seq-gap/streaming-quarantine/framing reasons under `evidence/m2/m2-slice7b-seq-gap-streaming-quarantine-framing-20261007.md` ; cross-platform matrix + CI wiring under `evidence/m2/m2-slice8-cross-platform-matrix-ci-wiring-20261007.md` ; Clippy CI fix after failed run 37648931640 under `evidence/m2/m2-slice8b-clippy-ci-fix-20261007.md` ; macOS unused-`io` fix after partial-green run 37650566598 under `evidence/m2/m2-slice8c-macos-clippy-unused-io-20261007.md` ; Windows process-identity lifetime sync after near-final run 37653004440 under `evidence/m2/m2-slice8d-windows-process-identity-sync-20261007.md` ; pilot-runtime fixture uniqueness after macOS fail on run 37656380940 under `evidence/m2/m2-slice8e-pilot-runtime-fixture-uniqueness-20261007.md` ; ADR-0016 Â§8.1 quarantine directory format restored; Windows FS path-ops + per-device AcceptedEvent DeviceId binding under `evidence/m2/m2-slice8f-windows-path-ops-and-device-binding-20261007.md`; owner-approved Windows quarantine extended-path adapter (`\\?\` I/O only; no registry/`LongPathsEnabled` dependence; Â§8.1 names + DeviceId policy unchanged) under `docs/adr/ADR-0016-windows-quarantine-extended-path-addendum.md` (ACCEPTED) and `evidence/m2/m2-slice8g-windows-quarantine-extended-path-20261007.md`; regression tightened so >MAX_PATH existence/read/cleanup required checks use prefixed adapter paths only under `evidence/m2/m2-slice8h-extended-path-regression-tighten-20261007.md`; pre-Actions security/DoD review + in-scope fixes under `evidence/m2/m2-security-dod-review-20261007.md`; macOS CI `prepare_m1_runtime_foundation_*` fixture race fixed under `evidence/m2/m2-slice8i-install-runtime-temp-root-race-20261007.md`; full cryptexd `--lib` temp-root hardening under `evidence/m2/m2-slice8j-cryptexd-lib-temp-root-hardening-20261007.md` (shared `test_temp` exclusive allocator across identity/pairing/recovery/sequence/etc.). **Latest final dual-OS CI gate PASS** on tested SHA `9ade9f7db18ec0e383594c19e592809e3b194531`: run [37706283147](https://github.com/gungortarik/cryptex/actions/runs/37706283147) â€” macOS readiness **success**, Windows readiness **success**, Windows IPC ACL **success**, unsigned Windows package **skipped** by workflow design. Mac host four-check PASS at the same SHA is documented in `evidence/m2/m2-final-ci-gate-20261008.md`; run 37681630801 is historical. Owner-delegated closeout review PASS is recorded in `evidence/m2/m2-owner-delegated-review-20261008.md`; normal PR #8 integration into `m0/foundation` completed at `817b6a1`. Security/DoD dispositions remain documented in the existing audit records. `LongPathsEnabled=0` is not claimed. M2 is COMPLETE within scope and integrated at merge commit `817b6a1`; M3 Phase 0 plan is complete on `m3/local-projection-search` (`evidence/m3/m3-phase0-implementation-plan-20261008.md`); product implementation not started.
Do not freeze `segment_events=1000` as a production constant. Do not expand into
M3/M8/M10 product behavior. Do not dispatch further Actions unless the owner requests them.


**Historical M0 closeout (retained):** Final behavior-bearing source CI was
green at `c7fa011` (run `37387323751`); Mac embedded `.app` visual review
scoped PASS; scoped ADR-0012 and ADR-0004 accepted. Older continuation entries
below remain dated history. Prefer this current status and the ADR index for
present-day decisions.

- Work only on the current milestone (M4). Do not opportunistically implement M5 Quick Capture, M6/M7 Obsidian behavior, M8 replication, M10 purge, or M13 broad Desktop product UI.
- Follow the accepted M4 contract and authoritative memory/entity specifications. Knowledge materialization must remain derived/rebuildable; canonical history stays immutable and authoritative.
- If the user's intent about tool roles, workflow, architecture, branch strategy, milestone scope, or integration strategy is ambiguous, ask before changing direction.
- M3 final candidate `4a2ea56` has Windows-local checks PASS, owner-reported macOS four-check PASS, and final dual-OS Actions PASS in run `37780668640`; normal PR #9 merge is `35538c1`.
- M4 implementation is explicitly authorized after ADR-0017 documentation acceptance at `ee7e173` and follows `docs/milestones/M4_KNOWLEDGE_ENGINE.md`. Historical references here to the Slice 3 candidate `45c78fd` and the need to run macOS before Slice 4 are superseded by later recorded Slice 3/4/5 verification. Current candidate `b1c43e2c36534e360cb06fb5cacef5bf6fd210ee` has final dual-OS Actions PASS (run `38021522882`), with Windows IPC framed round-trip and interrupted SQLite migration tests; see `evidence/m4/m4-final-ci-gate-20261010.md`. macOS retrieval-safety visual review is scoped PASS at documentation tip `ebdd8c9` (`evidence/m4/m4-macos-native-visual-review-20261010.md`; tray status-item Quit NOT VERIFIED). Remaining gate is final DoD/owner review. Do not enable v2 production writing or implement M8/M10/M14 opportunistically.
- Do not merge, rebase, switch branches, amend, force-push, or rewrite history without explicit approval.
- Runtime data must remain outside the source repository.
- GitHub Actions minutes are constrained. Prefer relevant local checks; run only the required final M4 workflow after its implementation candidate is reviewed.
- Do not touch retained P0-1 corpora unless the current approved test requires it.
- Never label behavior VERIFIED unless it was actually physically/test verified.
- Do not freeze the P0-1 benchmark profile value `segment_events=1000` as a production constant.
- **Historical M1 working-agreement residue (superseded):** earlier text said "Work only on the current milestone (M1)" and "normal PR #6 merge remains before recording M1 integrated." M1 is complete and merged at `c2168f3`; documentation closeout is `0c84c5c`. Owner acceptance of the delegated M1 review is recorded at `evidence/m1/m1-final-review-20261007.md`.
- ADR-0013, ADR-0014 (P1), ADR-0015, ADR-0016, and ADR-0017 remain ACCEPTED (ADR-0016 at `5cc89b709dcf236698c3253b8582c873de7c7f0f`; ADR-0017 accepted 2026-10-08). M2 and M3 are complete. M4 implementation is underway only on the authorized branch/slice; the v2 production writer has a separate closed activation gate.

## P0-1 status

P0-1 physical event packaging is complete and ADR-0013 is accepted.

Decision:
- canonical physical packaging uses immutable sealed per-device segment/pack files
- one-file-per-event was rejected at scale
- hybrid packaging was not selected
- the benchmark profile's `segment_events=1000` is not a frozen production constant

Integration update:
- the owner explicitly approved normal merge of `origin/m0/p0-1-cursor`
- merge `cb9069e` integrates accepted P0-1 tip `6d4d10a`
- ADR index conflict preserves accepted IPC, proposed lifecycle and accepted
  ADR-0013 together; no retained corpus changed

## M0 closure continuation (2026-10-05)

Mac native renderer was initially **FAIL**, entirely blank in checkout `dee43e2` /
retained raw Cargo release `7f5ab2b`; that failure remains preserved. A fresh
embedded Tauri `.app` at checkout `e755601` has now produced a **SCOPED PASS**:
rendered operator captures and clean owned-session shutdown. See
`evidence/m0/macos-embedded-frontend-visual-evidence-20261005.md`.
Pinned Tauri frontend mode depends on `tauri/custom-protocol`, not Rust release
optimization. Raw release in the old plan omitted it and can use devUrl port 1420.
Source `65ce7b9` adds typed pre-Tauri wrong-mode release rejection and explicit
embedded-frontend CI build/test/lint (full release suite). Independent focused
security/verifier review PASS, including release feature-omission sentinel.
Windows 23 debug / 24 release / 4 CLI, strict lint/format/package build, wrong-mode exit 2
and supported hidden session/clean exit 0 PASS. No new visible Windows UI or Actions.
Evidence: `evidence/m0/windows-embedded-frontend-mode-evidence-20261005.md`.

Mac scoped plan completed: 16 debug / 17 release / 4 CLI, targeted rejection and
fresh-package visual review reported PASS. Raw Mac artifacts were not copied for
independent re-hashing. Final CI **PASS** at
`c7fa011672973101cb2a5519b5883b26c6909f90`, run `37387323751`; see
`evidence/m0/m0-final-ci-attempt-20261005.md`. Scoped ADR decisions, human
closeout review and final checklist reconciliation are complete. M0 is complete
for the synthetic foundation scope; no merge/M1 is inferred.

### Earlier candidate CI and review state

Corrected final CI **PASS** on `5c074115cd61ef7c1eb47e8ed2cf8c1105360679`,
run `37377520994`: Windows readiness **93 debug Rust + 1 release resolver,
160/160 + 8/8 Node**; Mac readiness **75 debug Rust + 1 release resolver,
162/162 + 8/8 Node**; actual Windows cross-account ACL PASS. All required jobs
green; package-only installer intentionally skipped. Exact jobs/log provenance
and first failed/cancelled attempt are retained in
`evidence/m0/m0-final-ci-attempt-20261005.md`.
At that earlier candidate, only missing plain benchmark build prerequisite was
added. Subsequent result-recording commits were documentation only. New `65ce7b9`
changes behavior/workflow; the fresh final CI at `c7fa011` verifies that fix.
Independent verifier and security audit completed at `3de9d00` after owner
explicitly authorized both agents with "hemen basla". Results:
`evidence/m0/m0-independent-review-results-20261005.md`. No new blocking M0
implementation/security defect established; candidate CI independently verified.
Stale pending-documentation finding is reconciled. Five production residuals
remain mandatory, including file-controlled developer launch and buffered REST.
Windows native foundation UI review now scoped PASS: actual PID19376/1116,
matching renderer health.pong, readable wrapping/reachable controls and retained
screenshots; real Quit clean exit0, processes/lock/IPC/gate clean. Owner coordinated
left-display placement. See `evidence/m0/m0-foundation-ui-review-20261005.md`.
Historical before corrected package revalidation: **Mac-only** narrow native foundation visual review under
`evidence/m0/m0-native-foundation-ui-review-plan-20261005.md`; do not repeat old
Windows/Mac login/tray/upgrade suites. Then explicit human diff/ADR acceptance
and final-source CI/checklist reconciliation. Windows proof is not Mac visual proof.
Mac attempt at `9752798` remains **NOT VERIFIED visually**: Computer Use
`Invalid app`, no screenshot/layout/scrolling proof. Supplied retained build
`7f5ab2b` has matching relevant application source; isolated Desktop/daemon
94503/94513 health and clean exit0/lock/gate/IPC cleanup reported. Record:
`evidence/m0/macos-native-foundation-ui-attempt-20261005.md` (Mac-local artifact
not directly inspected on Windows). Continue the plan's **operator-screenshot
fallback**, fresh owned pilot, two actual Mac window captures and image-file
review. No old suite/build/Actions repetition or security setting changes.
Existing PR #5 targets m0/foundation; do not change integration/base, merge,
accept ADRs, complete M0 or start M1 without the required concrete approval.
No redundant CI dispatch requested; avoid disturbing the owner's game.

### Historical pre-review continuation

Final CI attempt at `e337bd3`, run `37376952961`, found missing plain
`m0-storage-bench` before Golden Node tests on a fresh Mac runner (ENOENT).
Workflow now builds it alongside the lifecycle helper; Windows focused Golden/
Doctor tests **6/6** and exact prerequisite build PASS. No assertions changed.
Windows ACL job passed; Windows readiness cancellation was requested to avoid
the same missing prerequisite. See `evidence/m0/m0-final-ci-attempt-20261005.md`.
At that first attempt, final CI was NOT VERIFIED. The corrected-source run
subsequently passed as recorded above; the earlier failure remains historical evidence.
Independent verifier/security review and human diff/ADR acceptance remain open.
Review packet: `evidence/m0/m0-human-acceptance-review-packet-20261005.md`.

Latest Mac namespace parity reported **PASS** at
`7f5ab2bdaa72e213f5cb16312bca0292c436db18`: macOS 27.0.1 arm64,
Rust/Cargo 1.98.1, Node v24.19.0. Desktop **14/14 debug + 14/14 release**, CLI
**4/4**, both strict Clippy profiles/format/locked release/diff PASS; all exits 0,
no failures/skips/ignored, cleanup error-free. Compiled defaults use Cryptex-Dev
in both profiles; no app/daemon, runtime/startup/config or repo changes.
Evidence: `evidence/m0/p0-2/macos-release-probe-root-evidence-20261005.md`.
This superseded pending namespace parity below. Corrected-candidate implementer
review completed for the narrow M0 boundaries with production residuals retained.
Next: one final CI run at the clean pushed candidate, then verifier/human diff
review and explicit ADR acceptance. No new Mac physical plan is requested.
ADR-0012/0004 remain PROPOSED and M0 IN PROGRESS.

Latest correction: `ad44e82` moves ordinary M0 release probe roots to
Cryptex-Dev/runtime/p0-2 on both OSes; `fc79f25` fixes a discovered release-only
strict Clippy warning without changing startup-error behavior. Windows final
Desktop tests **21/21 debug + 21/21 release**, both strict all-target lint
profiles, format/release build and negative CLI **4/4** PASS. No old runtime,
installed binary, registration, global config or visible app changed.
Evidence: `evidence/m0/p0-2/windows-release-probe-root-evidence-20261005.md`.
New narrow Mac task: `evidence/m0/p0-2/macos-release-probe-root-test-plan-20261005.md`,
expect **14/14 debug + 14/14 release + 4/4 CLI**. No previous plan rerun.
Final CI now covers release resolver regression and release-only strict lint;
do not dispatch until this new Mac parity and corrected-candidate review pass.
Historical audit finding below is corrected in code, not yet cross-platform
verified. Final checklist/ADR/human acceptance remain open.

Final implementer gate audit at source `8437559`:
`evidence/m0/m0-final-gate-audit-20261005.md`, **NOT READY for final CI**.
Retained foundation/update/notice-documentation proofs now fill previously empty
checklist evidence; no ADR or final gate is accepted. Two retained Windows native
report hashes still match. Genuine remaining implementation finding: ordinary
M0 release Desktop defaults to product `Cryptex/runtime/p0-2` on both OSes,
while development/pilot fixtures use Cryptex-Dev. Correct the ordinary M0 release
namespace, add path regressions and collect narrow Mac parity before final CI.
Do not mutate/delete old runtimes, login settings or historical evidence.
Security/verifier review remains incomplete until this correction is reviewed.
No previously passed Mac runtime/Syncthing plan needs repetition.

Latest Mac sidecar regression is scoped **PASS**, reported by the operator/Codex
at `388e46a7e5f750d88e7fc786e0c108dcea5329e4`: macOS 27.0.1 arm64,
Rust/Cargo 1.98.1, Node v24.19.0, pnpm 11.28.0. Desktop **13/13**, pre-Tauri
CLI **4/4**, format/strict Clippy/frontend/locked release/diff all PASS; every
command exit 0, no failures/skips/ignored, fixture cleanup error-free.
Backend-selected path and developer override authority plus real POSIX symlink
refusal passed. No visible app/daemon, edits, commits or Actions; repo clean.
Evidence: `evidence/m0/p0-2/macos-sidecar-boundary-evidence-20261005.md`.
This closes the new Mac validator regression, not physical renderer invocation.
No passed Mac plan requires repetition.

Owner scope decision recorded: "kurallar dahilinde onayliyorum" authorizes
preparing the scoped ADR-0012/0004 revisions in
`evidence/m0/m0-acceptance-scope-decision-proposal-20261005.md`.
Both proposals now separate M0 direction acceptance from mandatory production
gates: M1 secure storage/identity, M2 event semantics, M8 replication, M14 installed
updater coordination and M17 signed release/final notices. No production gate is
reported verified; the Master Spec and accepted milestone contract are unchanged.
Next: review these concrete revisions and the retained security/notice residuals,
then run required final CI once on the resulting stable candidate. Human diff/ADR
acceptance and the evidence-backed full M0 checklist remain required.
ADR-0004/0012 remain PROPOSED; M0 remains IN PROGRESS and M1 is not authorized. *(Historical M0-era note â€” superseded: M0 complete, M1 active, ADR-0015 ACCEPTED; see top.)*

The owner explicitly authorized completing the entire M0 milestone. Closure
audit: `evidence/m0/m0-closure-audit-20261005.md`. P0-4 managed Syncthing and its
ADR-0004 addendum remain unaccepted; managed staged lifecycle now has both-OS
proof, including the exact 2.1.2 -> 2.1.5 version pair; acceptance review remains open.
P0-1 transport evidence alone does not close them.
Do not declare M0 complete or start M1 based only on the P0-2 pilot.

Native interruption `7d7a60e` adds schema-2 intent ownership, absent-child checks
and exclusive recovery claim. Live/ambiguous PID, missing evidence and previous
recovery claim refuse mutation/startup. A killed updater at the selected but
pre-successor-launch boundary released the native gate; actual baseline restart
and protocol-1 health PASS. Interrupted recovery claims remain inspection-only.
Golden/Doctor foundation `7daadaa` generates deterministic synthetic sealed packs
with the accepted P0-1 codec and checks actual IPC/PID/held-lock health, retaining
`not-implemented` for canonical store/Syncthing checks. Fixture membership/hashes
were unchanged across real recovery. No M2 semantics were added.

Windows integrated checks: Rust **89/89**, lifecycle/foundation Node **123/123**,
storage capture Node **5/5**, workspace format/strict Clippy/locked build PASS.
The first workspace run found two parallel Mode B tests competing for the one
user IPC endpoint; their lifetimes now serialize without removing assertions.
Historical failure is preserved. Actual recovered pair `30972/29584`, Doctor
healthy, clean exit 0, lock released and endpoint absent. Evidence:
`evidence/m0/p0-2/windows-native-interruption-evidence-20261005.md`.
Combined Mac plan:
`evidence/m0/p0-2/macos-native-interruption-test-plan-20261005.md`.
Mac native interruption/Golden/Doctor parity was reported PASS at exact source
`502abf409925b368ff1377380c719069207f0d61`: Rust **71/71**, Node **123/123**
plus storage **5/5**, format/strict Clippy/build/package/syntax/diff PASS.
Updater `82701` was killed before candidate launch; old pair `82711/82719`
had exited cleanly. Recovery pair `82733/82740` used the correct baseline paths
and parents, protocol-1 health and Doctor healthy. Final exit 0 released all
locks/gates/IPC/intent/claim; synthetic sealed fixtures remained unchanged.
User-reported artifact hash and scope:
`evidence/m0/p0-2/macos-native-interruption-evidence-20261005.md`.
No Actions or global Syncthing/startup changes.

P0-4 continuation `7ff485d` adds an external pinned **2.1.5** native controller
and actual two-profile Windows runner. Auth rejection 403/403, pairing, native
history, exact two-content conflict preservation, same-version restart/new
transfer and four clean monitor exits PASS. An initial 8-second stop bound was
shorter than upstream's native 10-second connection close; source-based 25-second
budget now passes immediate restart shutdown at ~10.1 seconds without removing
exit/PID/database checks. Historical forced-cleanup failures remain recorded.
Combined Node **130/130** plus storage **5/5**, changed syntax/diff PASS.
Evidence and explicit Mac task are under `evidence/m0/p0-4/`.
ADR-0004 addendum is **PROPOSED**, not accepted. Notice inventory is retained
under `third_party/syncthing/2.1.5/`; actual bundled distribution, changed-version
compatibility and production secret storage are still unproved. No P0-4 or M0
completion claim, unrelated/global process/config change or Actions run.

Staged Syncthing follow-up copies the exact native binary plus pinned upstream
LICENSE/AUTHORS to a fresh proof package, resolves only that role and rechecks
membership/digests after the real run. Linked roots, corrupt metadata, tampered
payload/notices and wrong roles fail closed. Windows actual packaged-role run
passes all prior assertions and four clean stops (including 10.107s early stop).
Combined Node **137/137**, managed **14/14**, syntax/diff PASS. Mac plan now expects
**15/15** managed tests including its POSIX mode check. Installed distribution,
publisher authentication and actual version-change compatibility remain open.

P0-2 changed-package follow-up `309bb66`: native runner accepts a separate,
fully inventoried candidate and refuses unchanged/resource-only native proof.
Windows actual baseline Desktop metadata 0.0.0 to trial 0.0.1 (temporary external
config) passes complete pair selection and actual protocol-rejected rollback.
Both Desktop and daemon payload hashes differ in this Windows retained pair.
Five owned sessions clean exit 0 with correct slot/parent paths, protocol-1 health,
released locks and absent IPC. Accepted-codec synthetic sealed packs/manifest
remain unchanged after each scenario. Combined Node **143/143**, storage **5/5**,
release Desktop **17/17**, workspace format/strict Clippy/syntax/diff PASS.
Evidence/task: `windows-changed-native-package-evidence-20261005.md` and
`macos-changed-native-package-test-plan-20261005.md` under `evidence/m0/p0-2/`.
Repo product version remains 0.0.0; no installed updater or semantic daemon
version migration/signature/canonical-production acceptance is claimed.
Mac changed-package parity was reported PASS at
`9a74b40414fad7b34d6e038fbe95eff334820262`: 6/6 new tests, builds/format/strict
Clippy/syntax/diff passed, five clean native sessions, committed candidate and
protocol-rejected rollback with unchanged sealed fixtures. Mac Desktop changed;
daemon bytes stayed identical. Report hash and exact scope are preserved in
`evidence/m0/p0-2/macos-changed-native-package-evidence-20261005.md`.
Native interruption/foundation subsequently reported PASS at `502abf4` as above.
Managed Syncthing subsequently reported scoped PASS at `501d11d` as below.
ADR-0012 PROPOSED and M0 IN PROGRESS. No Actions or startup/install changes.

Managed Syncthing Mac tests at `502abf4` stopped before live launch: **7 PASS,
8 FAIL**, `SYNCTHING_BUNDLE_SOURCE_CHANGED`. Notice pins described Windows CRLF
checkout bytes while Git/Mac files were LF. Fix `f17bcf7` pins the exact Git LF
blobs and scopes `.gitattributes` to LF notice checkouts, without runtime
normalization or alternate accepted hashes. Three regressions cover checkout,
copied bytes and tampering. Windows validation exposed two native observation
races: idle status now waits within the existing 45-second bound and synthetic
fixture ENOENT is pending during replacement; malformed status, pull errors and
other I/O errors still fail. Both failed runs remain recorded.
Corrected Windows live proof passes all managed assertions and four clean stops;
combined Node **150/150**, storage **5/5**, managed **21/21**, syntax/diff PASS.
Evidence: `evidence/m0/p0-4/notice-byte-portability-failure-and-fix-20261005.md`.
Mac rerun at exact `501d11d64cb7d77e8b1c51ad453fdf15f326fc8d` reported
**22/22**, five syntax/diff checks and live isolated runner exit 0. LF notice
checkout and staged binary/notice digests/POSIX modes passed. Incorrect API keys
returned 403/403; pairing, pause/resume, idle, history, restart/new transfer and
both divergent conflict contents passed. Global config observed digest unchanged.
Four monitor PIDs `83988/84004/84014/83984` exited 0 in
`36/10028/791/155` ms and were absent; no forced cleanup/database-stop failure.
Exact report hash and limitations:
`evidence/m0/p0-4/macos-managed-syncthing-evidence-20261005.md`.
Managed Mac parity is closed; no passed Mac plan needs repetition.
No Actions run; installed distribution/version-change/secret acceptance stays open.

Changed-version Syncthing continuation `133428b` now proves Windows **2.1.2 ->
2.1.5** against fresh synthetic profiles. Product pin remains 2.1.5; the old
version is an explicit historical test fixture with a separate ownership scope.
Both CLI and authenticated REST versions are pinned. Folders pause before cold
profile snapshot; live/PID-ambiguous receipts, ownership/corruption, overlap,
links and changed backup refuse copying/restoration. Compatible candidate
transfer/restart/new transfer PASS. Injected version-health rejection stops the
candidate, preserves its profile, restores the complete cold baseline and proves
2.1.2 identity/key/payload plus a new transfer. No newer database is opened by
the old binary. Eight monitor exits 0/PIDs absent; observed global config unchanged.
Combined Node **160/160**, storage **5/5**, four syntax/diff checks PASS; existing
managed 2.1.5 live regression also PASS with four clean stops. Evidence:
`evidence/m0/p0-4/windows-syncthing-upgrade-evidence-20261005.md`.
Mac parity reported PASS at exact `dec5fb19fb37b1199ac0f192c929a7113388cc0b`:
**33/33**, four syntax/diff checks, two live scenarios/runner exit 0. CLI and
authenticated REST confirmed 2.1.2 -> 2.1.5. Compatible upgrade/restart/new transfer
and controlled version-health rejection/cold baseline restore/new transfer passed.
Original identity/key/payload and staged notices/hash/modes retained; global config
observed digest unchanged. Eight monitors exited 0 and PIDs absent:
`85878/85889/85894/85874/85906/85912/85922/85902`, shutdown
`18/17/18/897/19/10020/883/1002` ms. No forced cleanup/database-stop error.
Official historical archive checksum matched; exact input/report hashes and
user-reported artifact provenance:
`evidence/m0/p0-4/macos-syncthing-upgrade-evidence-20261005.md`.
The corresponding Mac plan is complete; no passed plan needs repetition.
Cold maintenance-window fixture rollback is not live canonical rollback,
installed updater, crash atomicity or power-loss proof. ADR-0004 remains PROPOSED.
No Actions, global installation/startup change or unrelated process termination.

Final CI coverage preparation: `.github/workflows/ci.yml` readiness now includes
Desktop Rust tests, explicit debug helper/release Desktop builds and all lifecycle
Node tests plus storage capture/sidecar staging. Both OS jobs retain their
existing selectors/triggers; readiness timeout is bounded at 35 minutes for the
extra build. Local equivalent helper/release builds and Desktop **17/17**,
lifecycle Node **160/160**, storage/sidecar **8/8** PASS. Static workflow review
and diff check pass; a YAML parser is unavailable locally and cloud workflow
execution remains NOT YET VERIFIED. No Actions dispatched; final CI still needs
one stable-source run after outstanding acceptance/security/notice review.
P0-4 Mac upgrade plan is closed; no new Mac request at this continuation.

Security/acceptance review continuation: `82c4d32` fixes a normal-session
renderer executable-selection hole. `p0_2_start_daemon` now validates every
request against the backend-selected ordinary absolute sidecar before runtime
creation/launch; stores/launches the approved backend path. Displayed path is
read-only. Backend-controlled developer override remains available; renderer
cannot replace it. Windows Desktop **20/20**, format/strict Clippy/frontend and
release build, lifecycle Node **160/160**, storage/sidecar **8/8** PASS.
New Mac task: `evidence/m0/p0-2/macos-sidecar-boundary-test-plan-20261005.md`
(Desktop **13/13**, CLI **4/4**, format/strict Clippy/frontend/release build).
Passed Syncthing/login/tray/native recovery plans need no repetition.

Review: `evidence/m0/m0-security-notice-acceptance-review-20261005.md`.
Exact notice digests remain pinned, but delivered source-access notice,
dependency inventory/installer inclusion, secret/permission boundary and proposed
signed/canonical acceptance prerequisites are unresolved. IPC byte framing is
bounded; whole-read deadlines are not established. Checklist now reflects
both-OS Golden/Doctor and scoped Syncthing proofs; security/human/final gates
remain unchecked. No ADR/roadmap prerequisite was waived. No Actions dispatched:
new Mac boundary check and acceptance review precede the final paid CI run.

Read-only Windows signing readiness audit: SDK x64 SignTool is available, but
the current-user My store has **0 currently valid code-signing certificates with
private keys**. No certificate export, purchase, policy change or signing attempt.
This does not inspect external signing services or establish publisher trust;
signed-package acceptance cannot be claimed from the existing unsigned proofs.

## P0-2 evidence status â€” verified slices; acceptance open

### macOS

The combined Mac release lifecycle slices now PASS, including physical tray
Open/Quit at checkout `dfbaded` using the validated `c77479f` release build.
Desktop `41780` / daemon `41851` survived physical tray left-click restore and
right-click Open with the same daemon PID. Snapshots recorded
`window_restored_via_tray_left_click` and `window_restored_via_tray_menu`.
Physical tray Quit with the daemon active exited 0 and recorded
`clean_quit_from_tray`; both processes and the held lock were gone. Login start
remained disabled. The operator identified the actual black rounded-square
icon left of ChatGPT after its menu-bar position changed; earlier synthetic
input failures are historical, not proof of an application defect.

The initial Desktop-owned lifecycle run at `c77479f` was PARTIAL. Local daemon supervisor
tests (3), sidecar staging tests (3), sidecar resolution tests (2) and release
package-smoke passed. Release Desktop `31422` automatically started daemon
`31562`, rejected a second Desktop launch, and restarted the SIGKILLed daemon
as `32006` within a 500 ms polling observation; renderer IPC answered protocol 1
with the replacement PID. One tray icon and duplicate daemon rejection were
confirmed. Login start was enabled through `Cryptex.plist` for the release
Desktop with `--cryptex-background`. The operator completed real sign-out/sign-in:
Desktop `32654` started with the background argument, no visible window and one
tray icon; daemon `32891` answered renderer IPC with protocol 1 after Dock reopen.
Explicit Stop left no daemon for a measured 10 seconds. Login start was disabled
through the renderer and `Cryptex.plist` removal verified. Final `Quit clean`
removed the Desktop/daemon processes and recorded `state=clean_quit`. Actual
tray Open/Quit could not be exercised; tray exit 0 was not captured. Those tray
checks were NOT YET VERIFIED in that initial run; the test registration was cleaned up and the
runtime evidence is retained.
See `evidence/m0/p0-2/macos-tauri-owned-supervisor-restart-evidence-20261003.md`.
The later physical diagnosis closes those two Mac tray checks. Signed installer
delivery, actual package update/rollback and ADR-0012 acceptance remain open.

The focused tray-only follow-up at checkout `924cc13` reused the validated
release build without repeating login/restart/Stop suites. Desktop `38558`
started daemon `38569`; red close-control activation hid the window and
preserved that PID. Native right/left clicks at the accessible tray item's
observed center did not expose Open/Quit or restore the window. Physical
display captures are retained outside Git in `/tmp/cryptex-tray-only-*.png`.
Tray Open/Quit remained NOT YET VERIFIED in that synthetic follow-up. Fallback single-instance focus plus
renderer `Quit clean` exited 0; no test processes or held lock remain and
login start remained disabled. The UI limitation's root cause is unproven.

Existing evidence proves:
- single-instance rejection
- crash detection / restart under the macOS native supervision candidate
- Mode B DesktopSupervisor process-level lifecycle
- Mode B GUI CloseRequested/hide path evidence
- physical traffic-light close and operator Dock restore; daemon PID unchanged
- synthetic compatible update / failed update / rollback marker-safety evidence

Still not enough to accept ADR-0012.

### Windows Mode B â€” valid physical foundation evidence

Experiment:
- `windows-mode-b-r2-20261002T050408Z`

Verified:
- daemon ready
- duplicate daemon rejected
- process-level window-close event leaves daemon alive
- forced crash detected
- DesktopSupervisor restart succeeds
- restart latency observed: 32 ms
- stale-lock wait before respawn
- clean stop
- runtime control-file cleanup
- post-restart status observation

Not verified by that run:
- physical Tauri GUI window close
- full Desktop IPC reconnect

### Additional Windows physical-host proofs (2026-10-02)

- Tauri `CloseRequested` hide/restore and daemon-survival proof: VERIFIED for the
  automated control-file-triggered physical Tauri event path. See
  `evidence/m0/p0-2/windows-mode-b-gui-window-close-20261002.md`.
- synthetic compatible update, bad-candidate rejection and rollback marker
  safety: VERIFIED for the Windows comparison harness. See
  `evidence/m0/p0-2/windows-update-rollback-evidence-20261002.md`.
- A direct child-process parent-exit control passed: the daemon retained its lock
  after the launching PowerShell exited. This does not reproduce HKCU Run or
  explain the separate post-login daemon exit; see
  `evidence/m0/p0-2/windows-parent-exit-control-20261002.md`.
- Fixed a reproducible HKCU comparison-harness defect: the on-demand
  `--leave-registered` probe now clears its one-shot stop marker before a later
  login. The physical Windows regression run retained the Run value during
  inspection, then removed it; see
  `evidence/m0/p0-2/windows-hkcu-stop-marker-cleanup-20261002.md`. This does
  not explain the historical reboot event, whose cause remains OPEN.
- Additional Shell-Core review confirmed the 9708 `Explorer_ExecutingFromRunKey`
  event is launch-operation completion, not proof of process exit: another
  startup PID from that same sequence remains alive. The old runtime artifacts
  are insufficient to identify why PID 25856 later disappeared.
- The shared production Desktop Rust IPC client has now physically reconnected
  to real daemons after supervised restart on both Windows and macOS; see the
  platform-specific proofs below. The actual Tauri renderer-to-command
  invocation during external Mode B crash/restart is VERIFIED on both Windows
  and macOS. The Mac release Desktop-owned lifecycle now has partial physical
  evidence (see the current Mac run above); Windows release-mode physical
  verification and the complete cross-platform production gate remain OPEN.
- A separate P0-3 candidate prototype now exists at `tools/m0-ipc-spike/`.
  Its Windows physical run verified role/version/capability handling,
  malformed/oversized rejection and a client reconnecting across an actual
  server-process restart. The DACL is explicitly set to the current token SID,
  and access denial with an authenticated second account subsequently passed
  on a disposable Windows CI VM (`windows-cross-account-acl-20261002.md`). The macOS physical
  runtime proof now verifies socket permissions/path, protocol rejection and
  reconnect after server restart. This is not production-client integration or
  an ADR-0007 decision. Evidence:
  `evidence/m0/p0-3/windows-ipc-spike-20261002.md`.
- A coordinated Windows release-renderer attempt kept the Tauri reconnect
  command active while Mode B force-killed and restarted the daemon. Mode B and
  its shared Rust IPC reconnect passed (`25988 -> 35808`, 34 ms), but the
  renderer's status did not change during the 12-second observation. A separate
  login daemon using the same named-pipe endpoint was active; this may confound
  the observation, but its effect is not proven. Classify the renderer crash
  reconnect as **NOT YET VERIFIED** and preserve the login daemon. The optional
  `--pre-crash-hold-ms` and `--post-restart-hold-ms` harness windows support a
  coordinated repeat that leaves the replacement daemon available for the
  renderer.
  See `evidence/m0/p0-2/windows-tauri-renderer-ipc-reconnect-20261003.md`.
- A controlled Windows repeat used both bounded holds. The release renderer
  displayed `35224 -> 3432`, matching the Mode B measurement; restart latency
  was 32 ms, and both the harness and Desktop exited 0. The Windows external
  Mode B renderer crash/reconnect gate is **PASS / VERIFIED**. The earlier
  uncontrolled-window attempt remains historical NOT YET VERIFIED evidence.
  See `evidence/m0/p0-2/windows-tauri-renderer-ipc-reconnect-20261003.md`.
- The coordinated physical macOS renderer crash/reconnect test is recorded at
  `evidence/m0/p0-2/macos-renderer-crash-reconnect-test-plan-20261003.md`.
  The repeat at `dc0ae10` used both bounded holds (15000 ms before crash,
  10000 ms after restart). The visible renderer displayed
  `IPC reconnected after restart Â· pid 24966 â†’ 24987`; both PIDs match the
  retained harness measurement (30 ms restart, exit 0). The macOS renderer
  crash/reconnect gate is **PASS / VERIFIED**. Desktop remained open until
  the result appeared, then exited 0 through `Quit clean`. The prior attempt
  without a post-restart hold remains historical NOT YET VERIFIED evidence.
- `cryptexd lifecycle update-proof` now rejects a synthetic mismatched
  Desktop/daemon protocol pair before staging, keeps active daemon `v1`, accepts
  a matching pair, and then runs the existing failed-candidate/rollback proof.
  Windows local tests passed (44 tests); the physical-host measurement is
  retained under `%LOCALAPPDATA%\Cryptex-Dev\p0-2\compare\windows-update-compat-20261003`.
  This remains test-only protocol metadata; it does not update a Desktop
  package. The physical Mac repeat at `46169ee` is **PASS / VERIFIED** for
  the synthetic fixture: 33 local tests passed, all ten requested measurement
  fields matched, marker digest remained `d914619dfbb07c95`, and final instance
  count was 0. Tests, build and update-proof exited 0. Evidence is in
  `evidence/m0/p0-2/macos-update-compat-evidence-20261003.md`.
- The new instrumented Windows login at 16:41:48 EDT on 2026-10-02 confirmed a
  regular user-profile logon. After 11.4 minutes, neither the direct daemon Run
  entry nor the PowerShell observer had produced a runtime artifact; the
  observer never attached, so no daemon exit code was captured. Shell-Core had
  no matching Run execution event, and Code Integrity had no matching block.
  Both temporary Run values were then removed and verified absent. Classify this
  as **login start NOT OBSERVED / cause OPEN**, not daemon start-then-exit. See
  `evidence/m0/p0-2/windows-instrumented-login-result-20261002.md`.
- A second physical reboot/login in the `wl-1003h` test produced a fresh
  console daemon PID `26576`, attached observer PID `26356`, and another
  `alive_at_observation_deadline` result after about 601 seconds. The screenshot
  console was the expected output of this test's direct
  `cryptexd.exe lifecycle login-hold` HKCU Run command; it was not the Cryptex Desktop. Both
  test Run values were then removed by exact manifest comparison in the actual
  user's SID-scoped registry view. A cooperative stop was requested; fresh
  status reports `pid=none`, and no matching startup entries or process remain.
  The retained lock file is stale and is not held. Runtime evidence and a
  pre-cleanup registration backup remain under LocalAppData. See
  `evidence/m0/p0-2/windows-reboot-login-survival-repeat-20261003.md`.
- A current-source Windows release executable physically started the
  Desktop-owned daemon, rejected a second Desktop launch (exit 0), replaced an
  exactly identified child after controlled termination, and passed the
  automated Tauri close/show request path with the replacement daemon live.
  This run used the existing executable built from `c77479f`; literal tray
  Open/Quit and login startup were not exercised. A fresh
  `tauri:package-smoke` build is blocked on this host: Code Integrity events
  3033/3077 deny `rustc.exe` loading its generated unsigned
  `serde_repr` proc-macro DLL. No policy bypass was attempted. See
  `evidence/m0/p0-2/windows-release-desktop-owned-supervisor-20261003.md`.
- Desktop close/restore/snapshot proof artifacts now query a fresh daemon
  lifecycle observation for the live PID instead of treating `ready` as live;
  status errors are reported as unavailable. A physical stale-ready Windows
  regression proof is included in the GUI evidence.
- The test-only native login/reboot path is verified on both platforms (Windows
  HKCU Run; macOS LaunchAgent). This does not establish production registration
  or select a supervision candidate.
- The separate historical HKCU Run dispatch/exit findings remain without a
  root cause. Do not relabel the later successful reboot/login proof.

### Windows native Task Scheduler candidate â€” failed candidate

Experiment:
- `windows-mode-a-r1-20261002T052751Z`

Physical result:
- normal interactive user session
- `schtasks /Create` returned `ERROR: Access is denied.`

Classification:
- valid FAILED CANDIDATE / PERMISSION evidence
- not an invalid harness result
- no UAC/elevation bypass was used
- Task Scheduler via `schtasks` is not accepted as a no-elevation per-user registration path
- this failure does not automatically select Mode B

### Windows HKCU Run candidate

Test-only registration:
- value name: `CryptexDevP02HkcuRun`
- key: `HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run`
- runtime: `%LOCALAPPDATA%\\Cryptex-Dev\\p0-2\\compare\\mode-a-hkcu`
- no-elevation registration succeeded physically

The candidate is login-start only:
- it does not provide OS crash restart
- crash-restart parity remains OPEN
- ADR-0012 remains unresolved

## Historical physical event â€” earlier Windows reboot/login

Both Windows and macOS machines were shut down and restarted on 2026-10-02.

The Windows HKCU Run test value had been registered before shutdown.

After the real Windows reboot/login:

```text
registered=true
daemon_lock_held=false
daemon_pid=25856
ready_present=true
```

`Get-Process cryptexd` returned no running process.

The retained `lifecycle.status` contained:

```text
runtime_root=C:\Users\Tarik\AppData\Local\Cryptex-Dev\p0-2\compare\mode-a-hkcu
pid=25856
instance=held
health=active
supervision=unresolved
registration=not_configured
protocol_version=1
note=holding instance lock for harness
updated_unix=1790946028
```

Artifact inspection showed:
- `ready` exists, contains `25856`, last written 2026-10-02 13:00:28 UTC
- `daemon.lock` exists, contains `25856`, last written 2026-10-02 13:00:28 UTC
- `lifecycle.status` last written 2026-10-02 13:00:28 UTC
- `stop` is absent
- no live `cryptexd` process exists when checked
- the instance lock is no longer held

### Approved interpretation

**Windows HKCU Run login launch is PHYSICALLY VERIFIED.**

The matching timestamps and ready/lock/status artifacts prove the login registration launched `cryptexd` and it acquired the instance lock long enough to publish ready/status.

**Post-login daemon survival is FAILED / ROOT CAUSE OPEN.**

The process later disappeared without a stop file and without writing a clean-shutdown status. Do not claim HKCU Mode A passed.

This also exposes a truthful-health problem:
- stale `daemon_pid` can remain visible after the process has exited
- persisted `lifecycle.status` can still say `instance=held` / `health=active` even though a fresh lock observation says the lock is not held

Persisted status is therefore not sufficient evidence of current runtime health.

### Follow-up after the read-only audit

The Codex implementation pass keeps the supervision comparison open and changes
only lifecycle status reporting:

- `InstanceObservation` separates a PID associated with a currently held lock
  from the last PID recorded in lock/status metadata.
- `lifecycle status` refreshes the timestamped status snapshot from a fresh lock
  observation.
- the short-lived `lifecycle probe` writes a released/stale snapshot after it
  drops its lock, instead of leaving its temporary `active` report behind.
- platform status results expose `daemon_pid` separately from `recorded_pid`.
- tests cover crash, stale PID reporting, status refresh and the persisted
  snapshot.

This does not explain the post-login process exit or add a supervision
mechanism. A persisted snapshot may still age between checks; consumers must
query current status and must not treat the ready marker as liveness evidence.

### Benign Explorer Run dispatch probe â€” INCONCLUSIVE

The `wd-1003a` test-only registration had reported present before the user's
sign-out/sign-in. The post-login status output reported a registration mismatch
or absence and no marker; its old boolean does not distinguish the two cases.
Read-only inspection later found the exact registered
command present; rerunning Status returned `run_registered=true`. The
marker remains absent. Shell-Core shows that the Run key was enumerated but has
no launch event for the test marker. Its enumeration event does not expose the
value list, and the Security log was unavailable without elevation. Preserve
the session artifacts; use the currently matching registration for another
real sign-out/sign-in observation before cleanup. Do not infer a dispatch skip
or registration-removal cause. Evidence:
`evidence/m0/p0-2/windows-run-dispatch-followup-20261002.md`.

## Syncthing state after reboot

Before reboot, the running Windows Syncthing process was confirmed to be the old P0-1 development instance:

```text
syncthing.exe serve --home=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1 --no-browser --no-restart --no-upgrade
```

It was launched from a user PowerShell process and was not registered as a Windows service, scheduled task, HKCU Run value, or Startup-folder item.

The reboot therefore stopped it. P0-1 is already complete. Do not restart it merely to continue P0-2, and do not delete retained P0-1 forensic/corpus data.

## Current continuation

### Windows implementation slice â€” committed; Mac release lifecycle passed

The project owner approved Mode B with per-user login startup launching
Desktop. The proposal is recorded at
`docs/adr/ADR-0012-background-runtime-implementation-proposal.md`; it is
**PROPOSED, not accepted**. The committed Windows implementation slice adds
unexpected child-exit detection and bounded restart handling to the current Tauri P0-2
supervisor, plus lock-aware restart behavior to the shared process controller.
A target-matched daemon sidecar is now built into the Tauri package-smoke
artifact. Release builds use the product app-data runtime root, start or attach
the daemon, watch adopted daemon processes, enforce one Desktop instance,
expose reversible per-user login-start controls, and create a tray menu for
Open/Quit. Signed installer packaging and complete Windows physical validation
of the startup/tray/supervisor flow remain open; Mac release-mode results are
summarized below.

Verification on Windows:

- `pnpm --dir apps/desktop build`: PASS.
- `cargo check -p cryptex-desktop --locked`: PASS.
- `pnpm --dir apps/desktop test:sidecar-build-script`: PASS (3 target/path
  tests).
- `pnpm --dir apps/desktop tauri:package-smoke`: PASS; Tauri's package-flavored
  release build staged `cryptexd-x86_64-pc-windows-msvc.exe` and produced
  `target/release/cryptex-desktop.exe`. The sidecar is a synthetic development
  build, not yet a signed installer. The current run includes tray and
  autostart integration and completed without compiler warnings.
- `cargo check -p cryptex-desktop --tests --locked`: PASS; all Rust test targets
  compile, including the pinned Tauri single-instance integration.
- `cargo test -p cryptex-desktop --bin cryptex-desktop --locked`: PASS (8 tests)
  on the current implementation, including release runtime paths and sidecar
  resolution.
- `cargo clippy -p cryptex-desktop --all-targets --locked -- -D warnings`:
  blocked because Smart App Control rejected `cargo-clippy` with OS error 4551.
- Rust formatting and `git diff --check`: PASS.
- `cargo test -p cryptexd --test mode_b_supervisor --locked`: test binary built,
  but two process-spawning cases were blocked by Windows Smart App Control
  (OS error 4551). Do not bypass that policy. `cargo check -p cryptexd --tests
  --locked` passed. The Mac run passed all three targeted process tests; Windows
  release-mode physical lifecycle verification remains open.
- `cargo build -p cryptex-desktop --release --locked`: PASS.
- Latest `pnpm --filter @cryptex/desktop tauri:package-smoke`: frontend build
  and sidecar preparation passed, but Tauri Rust compilation failed with
  `E0463` because Windows Code Integrity blocked the generated
  `serde_repr` proc-macro DLL (events 3033/3077). The previously built release
  executable was byte-matched to the latest Desktop source commit and used for
  partial physical lifecycle checks. Do not bypass the host policy. A fresh
  Windows release package must be built on an approved host/configuration.
- No GitHub Actions workflow ran.

A later single-job manual Windows package build was dispatched at `0867b9f`:
`https://github.com/gungortarik/cryptex/actions/runs/37168036553`.
Readiness/Mac and IPC jobs were skipped. It builds an unsigned development
NSIS installer and retains Desktop/daemon hashes; it is not signing or physical
host acceptance. See `evidence/m0/p0-2/windows-ci-package-build-20261003.md`
for execution status before requesting another run. That run completed:
NSIS build, 3 staging tests and 8 release helper tests passed. Artifacts were
downloaded outside Git and their hashes verified. The loose CI Desktop/daemon
ran on the Windows host; installer execution and signing remain unverified.

Windows physical tray Open and active-daemon Quit now PASS in
`evidence/m0/p0-2/windows-physical-release-tray-20261003.md`: Desktop `8140`,
daemon `22828`, native title-bar close hid the window, physical Open preserved
the daemon PID, and physical Quit exited 0 with both processes gone and lock
released. Actual Desktop login startup remains open. A login toggle attempt
from the CI artifact showed a direct-HKCU versus Windows startup-list mismatch;
it was disabled again and the CI processes cleaned up. Launch from the
operator's ordinary session before arming the real login test. Do not dispatch
another workflow for these local checks.

The ordinary-session follow-up is now armed: Explorer launched Desktop `12084`
and daemon `8052`; the renderer enabled startup and Windows' startup list
verified the exact CI-artifact Desktop command with `--cryptex-background`.
Preparation is outside Git at
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\windows-desktop-login-1003`.
The real sign-out/sign-in has now completed. Explorer launched Desktop `29872`
with `--cryptex-background` and daemon `26588` in the new session. The main
window was hidden, physical tray Open restored it, and renderer status/IPC
confirmed the held lock, active daemon and protocol 1. The renderer disabled
the product startup value; Windows startup-list removal was verified. Renderer
Quit clean exited 0 and both processes disappeared. These login slices PASS;
see `evidence/m0/p0-2/windows-desktop-login-result-20261003.md` for exact scope
and the stale agent filesystem records excluded from this proof.

The operator reported login-time consoles. A daemon-owned console host was
observed; all Desktop status/preflight/hold commands now use a platform adapter
with Windows `CREATE_NO_WINDOW`. Local check, all-target Clippy, 11 Desktop
tests and release package-smoke PASS. No host policy was changed and no new
workflow ran. The rebuilt release launched locally, then was cleaned up; its
physical no-terminal/flash observation remains inconclusive, and UI geometry
errors prevented the new-release status/IPC check. Do not count that visual
gate as passed. Product login is now disabled; test processes are gone.
The later observed foreground launch at checkout `73263ca` closes that visual
check: Desktop `14768` / daemon `8968`, renderer status held/active and IPC
protocol 1 passed, and the operator watching the screen confirmed no terminal
window/flash during launch and queries. Renderer Quit clean exited 0; processes
and held lock were gone. No login/reboot, rebuild or workflow was repeated.
Evidence: `evidence/m0/p0-2/windows-console-visual-result-20261003.md`.
Mac compile/lint request:
`evidence/m0/p0-2/macos-background-command-check-plan-20261003.md`.
Executed on macOS 27.0.1 arm64 at `83a770a`, Rust 1.98.1: format PASS
(exit 0), Desktop check PASS (exit 0, dead-code warning), strict all-target
Clippy FAIL (exit 101). `main.rs:64:4` reports unused
`mac_product_runtime_root` in the debug binary; its production caller is
release-only and its other use is test-only. The failure reproduced in an
individual command run. Exact evidence:
`evidence/m0/p0-2/macos-background-command-check-evidence-20261003.md`.
No application code, lint allowance, release package or Actions was changed.
That initial run did not close the strict Mac lint gate.

Source follow-up: `mac_product_runtime_root` is now compiled only for macOS
release or test targets, matching its actual callers and the Windows helper
pattern. No dead-code allowance was added. The Mac runtime tests also cover
missing HOME. Windows format, Desktop strict Clippy and the 11-test Desktop
suite were rerun locally. Mac revalidation has now completed at
`808bb1c039ab3735e3a6a777fafa6eae49790b10` on macOS 27.0.1 arm64,
Rust 1.98.1: format, Desktop check, strict all-target Clippy and targeted
`mac_runtime_path_tests` all exit 0 (2 tests passed, 2 filtered out). Check
and Clippy emit no warnings. The Mac compile/lint gate is now PASS/closed;
the evidence above retains the initial failure and appends exact new results.
No application code, physical lifecycle tests, release package rebuild,
runtime configuration or GitHub Actions was changed/run in this revalidation.

The Mac execution request at
`evidence/m0/p0-2/macos-tauri-owned-supervisor-restart-test-plan-20261003.md`
has been executed. The evidence at
`evidence/m0/p0-2/macos-tauri-owned-supervisor-restart-evidence-20261003.md`
records release Desktop startup and crash/restart, duplicate rejection, real
login startup, Dock reopen plus renderer IPC, Stop without restart, login-start
removal and process cleanup. The subsequent physical-input diagnosis verified
actual tray **Open Cryptex** and **Quit Cryptex**, including active-daemon
shutdown and foreground exit code 0. The test registration and processes are
cleaned up. Do not repeat the passed steps merely to fix the document state.

The focused diagnostic procedure at
`evidence/m0/p0-2/macos-tray-input-diagnostic-plan-20261003.md` is complete;
its physical result closes the Mac tray gate. Windows release login-start and
physical tray/menu slices are now verified, and the earlier release run
verified daemon replacement. The rebuilt Windows release's observed console-free
launch/query behavior and renderer status/IPC are now verified. The old CI
package predates the fix. The Mac debug-target lint failure is resolved and
revalidated at `808bb1c`; do not repeat its passed compile/lint or physical
lifecycle suites for this request. Next, complete the real compatible
Desktop+daemon package update/rollback and canonical-data checks on both
platforms. Keep ADR-0012 unaccepted until every required P0-2 proof passes.

Windows native-package candidate preparation now passed locally: retained
unsigned NSIS `0.0.0` and test-labelled `0.0.1` installers at source `5015ff1`,
with payload hashes outside Git under
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\native-package-proof-1003`.
No version/dependency change was made in the repository and no workflow ran.
Execution plan: `evidence/m0/p0-2/windows-native-package-replacement-plan-20261003.md`.
Baseline installation from the ordinary Windows session now passed its scoped
identity/status/IPC/clean-exit checks: Desktop `26124`, daemon `9412`, protocol 1,
exit 0 and no remaining processes. Evidence:
`evidence/m0/p0-2/windows-native-package-baseline-20261003.md`.
The installed Desktop differs from the retained loose binary only in Tauri's
three-byte `UNK` to `NSS` bundle marker; the exact comparison reproduces its hash.
Candidate `0.0.1` replacement now passed installed version/payload/status/IPC
checks: Desktop `9416`, daemon `28480`, protocol 1, clean exit 0 and no remaining
processes. Evidence: `evidence/m0/p0-2/windows-native-package-update-20261003.md`.
Operator rollback to `0.0.0` also passed: installed Desktop/daemon hashes match
the first baseline, Desktop `25436`, daemon `15340`, renderer protocol 1,
clean exit 0 and no remaining processes. Login startup remains disabled.
Evidence: `evidence/m0/p0-2/windows-native-package-rollback-20261003.md`.
The final installation is stopped at `0.0.0`. The real package sequence passed
its scoped install/replace/operator-rollback checks. A separate serialized
filesystem transaction candidate now passes 28 Windows tests, including five
actual updater-subprocess kill boundaries, plus 8 retained-binary-copy scenarios.
It is not integrated with installed Desktop and its health is injected fixture
behavior. Proposal: `docs/adr/ADR-0012-update-orchestration-candidate.md`.
Windows evidence: `evidence/m0/p0-2/windows-package-transaction-candidate-20261003.md`.
Mac executed `evidence/m0/p0-2/macos-package-transaction-candidate-test-plan-20261003.md`
at `780c0473bdb85b52abc994661f10671cc85e4992`, macOS 27.0.1 arm64,
Node 24.19.0. The default `/var` temporary path failed closed with
`UNSAFE_PATH` (1 passed, 27 failed): `/var` is a symlink and initialization
rejects symlink ancestors. With process-local TMPDIR set to the verified
`/private/var/.../T` real path, all 28 tests passed, none skipped, including
five actual owned-worker SIGKILL boundaries, pre-commit baseline recovery,
post-commit candidate selection, idempotence, corrupt-baseline refusal and
unchanged synthetic fixture inventories. POSIX directory fsync succeeded;
this is not power-loss proof. All four syntax checks and diff checks passed.
Evidence: `evidence/m0/p0-2/macos-package-transaction-candidate-evidence-20261003.md`.
No code repair or security-check weakening was made. Temporary fixtures were
cleaned, including 27 verified leftovers from the failed setup run. Optional
retained-binary-copy proof was not run; no two retained pairs were established.
That initial Mac serialized candidate run was scoped PASS with the explicit
non-symlink temporary-path prerequisite; its unprefixed command failed.
After reviewing `12f76ac`, the test fixture allocator now resolves only the OS
temporary directory before creating owned roots and registers cleanup before
experiment initialization. The package engine's link/junction rejection is
unchanged. A new alias regression passes on Windows: 29/29 tests, no skips.
Evidence: `evidence/m0/p0-2/windows-package-fixture-path-fix-20261003.md`.
Mac follow-up now PASS at `fc31489fc619b9b4b12f93cd350b215efa7f3c7c`:
the ordinary test command with the host's original `/var/.../T/` TMPDIR,
without override, passed all 29 tests (zero failed/skipped, exit 0). The alias
regression and all five actual SIGKILL boundaries passed; POSIX directory
fsync and fixture inventory checks completed. All six syntax checks and diff
check passed. Owned temporary roots were absent before and after execution.
The evidence above appends exact output and both temporary-path forms while
preserving the prior default-path failure and overridden 28-test PASS.
The Mac default-path fixture repair is now verified; no external override is
required. No optional binary staging, source repair or physical test was repeated.
Next: review native helper/startup integration and failure/data-preservation
gates before production architecture selection. Publisher trust, actual
runtime compatibility, concurrency, machine power loss and canonical-data
acceptance remain open; ADR-0012 remains PROPOSED. No repeated physical Mac
lifecycle suite, installed-package change or GitHub workflow ran.

Windows real-health candidate follow-up now PASS: the transaction health callback
executes the copied daemon and the actual shared Rust Desktop IPC client, matches
PID/protocol/held lock, then verifies clean exit 0 and lock release. Compatible
replacement, non-executable daemon rollback and actual protocol mismatch rollback
passed (3 scenarios); restored baselines also answered IPC. Active endpoint
preflight refusal was measured. Rust tool tests 10/10, Node adapter tests 4/4,
format, strict Clippy, local build and syntax checks passed. Evidence:
`evidence/m0/p0-2/windows-package-real-health-20261003.md`.
Mac real-health plan `evidence/m0/p0-2/macos-package-real-health-test-plan-20261003.md`
now PASS at source `1e07ca4` on macOS 27.0.1 arm64 with Node 24.19.0. Format,
Rust tests 10/10, strict Clippy, local helper build, Node adapter tests 4/4,
both syntax checks and diff check passed. The retained arm64 release Desktop /
daemon pair from `c77479f` was supplied as identical baseline and candidate
inputs (Desktop SHA-256 `0bfd52594a4a30bfcd6e37678835bb7d833cfe6be867f44996e45e18de36f1a5`,
daemon SHA-256 `bfbfa26c6df1934312ba0d483f5351ac759c8f3f96458f8753d5b4a70157feb4`).
Three real health scenarios passed: compatible pair, non-executable candidate
rollback and actual protocol 2/2 declaration against protocol 1 daemon rejection
and baseline restoration. Four healthy IPC observations confirmed Desktop role,
matching daemon PID, held lock and protocol 1; candidate/recovery exits were 0
with locks released. The invalid daemon error was `DAEMON_START_FAILED`, child
exit `-13`. Synthetic fixture bytes were unchanged. Full report remains outside
Git at `~/Library/Application Support/Cryptex-Dev/p0-2/mac-package-real-health-20261003-b15a85f/real-health-report.json`.
Detailed evidence: `evidence/m0/p0-2/macos-package-real-health-evidence-20261003.md`.
Desktop bytes were staged only, not run. This is not a version-changing Mac
upgrade or production canonical-data proof; installed updater handover, signing,
concurrency, power loss and canonical-data gates remain open. No GUI/lifecycle
repeat, Desktop rebuild or Actions ran; ADR-0012 remains PROPOSED.
Canonical-data preservation and an operator-context post-exit lock query were
not established by this baseline. The stock installer force-shutdown and sequential
Desktop/daemon copy behavior does not prove safe atomic updates. Automatic
rollback and incompatible-pair preflight remain unimplemented; keep these
distinct from the planned operator-controlled replacement/rollback experiment.

### Prior continuation record

### Latest Windows reboot/login result â€” VERIFIED

The user prepared `wl-1003h` from ordinary Windows PowerShell at
`2026-10-03T02:02:24Z`, source `0a7eb0e`, clean tree, then physically restarted
Windows. Boot was at 22:06:04 EDT; console session 1 logged in at 22:06.
Daemon PID 31512 started at 22:07:32 EDT and observer PID 31192 attached.
Shell-Core launch records match both PIDs. The complete 600-second observation
ended at 22:17:34 EDT with `alive_at_observation_deadline`, no stop marker and
no observed daemon exit. The direct process query afterward confirmed the same
live daemon and executable. A duplicate probe was rejected with exit 1.
See `evidence/m0/p0-2/windows-reboot-login-survival-20261002.md`.

The legacy comparison registration was safely backed up and replaced in the
user's actual context. Its old stop file remains retained. A second physical
reboot/login repeat and cleanup are recorded at
`evidence/m0/p0-2/windows-reboot-login-survival-repeat-20261003.md`. The
`wl-1003h` test registrations are now absent and no matching daemon process is
running, so registry-mutating Rust lifecycle tests no longer risk disturbing an
armed experiment. No GitHub workflow ran. The historical 13:00 UTC disappearance
still has no established cause. The Windows Mode B physical crash/restart proof
is already retained above. The next Windows lifecycle task is the release-build
Desktop physical startup/close/quit flow, not another Mode A login repeat.

Windows release foundation build is now verified: the full Rust workspace and
the Tauri Desktop with embedded frontend built successfully. The release IPC
process-level check passed. CI now includes the IPC spike on both platforms;
the first dispatched run passed on macOS and exposed a Windows-only redundant
borrow lint. The fix passed full local Windows workspace Clippy; combined-source
CI revalidation passed on Windows and macOS for source commit `a0f1c6f`.
See `evidence/m0/windows-release-build-20261002.md`. The user currently has no
accessible second Windows account. The cross-account IPC ACL proof instead
passed on a disposable GitHub-hosted Windows VM at source `7668d11`, run
`37085465368`. The test authenticates a temporary non-admin account and checks
the effective client-thread SID, exact access denial, owner connections before
and after, identity restoration and account cleanup. It refuses local workstation
execution. See `evidence/m0/p0-3/windows-cross-account-acl-20261002.md`.
The consolidated Windows result index is recorded at
`evidence/m0/windows-test-results-20261003.md`.

Retained diagnostic history and continuation constraints:

1. the second real login changed session 3 to 4, but the user's external
   collector found `wd-1003a` registration absent while Codex-context reads
   found it present. Do not equate these views or infer deletion/recreation.
   The user prepared fresh `wd-1003b` from ordinary Windows PowerShell at
   `2026-10-03T01:38:11Z`; external collection confirmed
   `run_matches_both_views=true`. The next collection at `01:39:33Z` still
   shows session 4 / Explorer created at `01:33:05Z`, before preparation.
   The subsequent real login changed session 4 to 5. Marker PID 15656 wrote
   at `01:42:14.7693778Z`; Shell-Core 9708 recorded that PID at `01:42:15Z`.
   The external-prepared benign HKCU Run command is now physically verified
   to dispatch after login. This does not prove daemon survival.
   Preserve `wd-1003a` and avoid preparing/cleaning startup values from Codex.
   The collector now records package identity and executable ancestry as well.
   `collect-windows-run-dispatch.ps1` retains observations in each session
   root. This command captures registry views, identity, policy, marker integrity
   and relevant Shell-Core events into a unique retained JSON observation.
   Five synthetic controls passed; collection on the physical Windows host
   recorded matching registry views, unchanged script hash, no Run-disabling
   policy values, Run enumeration and no experiment launch/marker.
   Registry visibility depends on observer context; package virtualization is
   a hypothesis, not yet a verified root cause. Preserve session evidence. This
   probe does not establish daemon survival or explain a termination cause.
2. The `wl-1003g` attempt was prepared through `Shell.Application` from Codex.
   Its process chain reaches Explorer, but its status wrapper still inherited
   Codex's registry view; this is not proof of the user's unvirtualized Run
   values. After the latest user sign-out/sign-in, no daemon or observer runtime
   files were present under `wl-1003g`. It is NOT a verified armed login test.
   Prepare a fresh instrumented root in the user's ordinary Windows PowerShell,
   confirm the manifest there, then sign out/in and return. Codex can read the
   retained results. Earlier `wl-1003c/d/e/f` preparations are incomplete.
   `wl-1003f` identified missing `Get-FileHash`; framework SHA-256 fixed that
   failure but not registry-view separation. No CI was dispatched. Preserve
   process-start/shell-launch
   evidence. Do not infer a termination cause if no observer attaches. See
   `evidence/m0/p0-2/windows-instrumented-login-result-20261002.md`.

   Latest external registry output identifies the occupied daemon value as the
   original `target/debug/cryptexd.exe lifecycle hold` command for
   `%LOCALAPPDATA%/Cryptex-Dev/p0-2/compare/mode-a-hkcu`; the external observer
   value is absent. This explains why cleanup against `wl-1003g` refused.
   The legacy root retains a stop file and records a clean shutdown at that
   login. The user ran `replace-legacy-windows-login.ps1 -Apply` from ordinary
   PowerShell and prepared `wl-1003h`; reboot/login and ten-minute survival
   subsequently passed as recorded above. The helper requires the
   exact confirmed old command and an empty observer slot, saves the old value,
   refuses a live legacy ready PID and restores the saved command on preparation
   failure if both slots are vacant. It preserves legacy runtime files.
3. P0-3 candidate comparison is recorded in
   `evidence/m0/p0-3/adr-0007-candidate-comparison-20261003.md`. The user
   selected the evidence-backed `interprocess` 2.4.4 candidate; the scoped
   implementation addendum is accepted at
   `docs/adr/ADR-0007-local-ipc-implementation-addendum.md`. The production
   daemon health endpoint and shared Tauri Rust client are implemented. A
   physical Windows Mode B comparison at source `c5e059d` verified that the
   client connected before forced crash and reconnected to the restarted daemon
   (`2880 -> 24780`, 32 ms restart); see
   `evidence/m0/p0-2/windows-desktop-ipc-reconnect-20261002.md`. Separate
   physical Tauri renderer proofs are now recorded for Windows and macOS in
   `evidence/m0/p0-2/windows-tauri-renderer-ipc-reconnect-20261003.md` and
   `evidence/m0/p0-2/macos-tauri-renderer-ipc-reconnect-20261003.md`. Both drove
   health/reconnect commands through the real renderer and observed a new daemon
   PID after clean stop/replacement. The macOS and Windows repeats also prove
   renderer reconnect while external Mode B force-restarts the daemon. The
   production supervisor/packaging decision remains open; do not accept ADR-0012
   yet.
4. use the existing physically verified macOS LaunchAgent login/reboot proof
   in `evidence/m0/p0-2/macos-native-supervision-evidence.md`. The separate
   release Desktop login-start flow now has partial physical evidence on Mac;
   Windows release-mode proof and signed package integration remain open.
5. use `evidence/m0/p0-2/supervision-candidate-comparison-20261003.md` as the
   current test-evidence comparison; production supervision/update proofs and
   the ADR-0012 decision remain open.

## OPEN

### Next continuation â€” approved native complete-package handover pilot

Native recovery cleanup and Mac formatting parity are recorded below. The next
concrete plan is `evidence/m0/p0-2/native-package-handover-plan-20261005.md`.
It recommends extending the complete-package slot candidate to actual owned
Desktop/daemon handover in fresh isolated roots, with native resource inventory,
whole-handover exclusion, clean shutdown, real IPC readiness and baseline restart.
The project owner explicitly approved this isolated pilot in the Windows
conversation ("onaylanmistir"). No further method approval is needed within that
scope. No production updater or
ADR acceptance follows from pilot approval. Canonical fixture acceptance also
requires the accepted P0-1 layout reference/integration; do not reconstruct the
missing ADR-0013 decision or touch retained corpora. Mac gets an executable parity
plan after the first implementation slice. Documentation review needs no Actions.

First slice at `e5423f2`: separate native-tree inventory and staging preserve
resources, empty directories and POSIX modes; target/schema/path validation
refuses mismatches, links/junctions, case collisions and overlapping/reused roots.
Copied trees and source bytes are reverified; partial staging retains a typed
diagnostic. Windows Node v24.19.0: **79/79 PASS** (51 transaction + 4 health +
6 native recovery + 18 native tree), no failures/skips, exit 0; three syntax
checks and diff check passed. The retained Windows installed tree (three files)
was copied into a fresh owned proof root and reverified without executing it.
Evidence: `evidence/m0/p0-2/windows-native-package-tree-evidence-20261005.md`.
Mac plan: `evidence/m0/p0-2/macos-native-package-tree-test-plan-20261005.md`.
Mac reported **79/79 PASS**, no failures/skips, exit 0 at `8296e66`, macOS 27.0.1
arm64, Node v24.19.0 and Rust 1.98.1. Locked helper build, three syntax and diff
checks passed, including POSIX modes and symlink refusal. No fixture/child/flush
error was reported; Mac changed no files. Result:
`evidence/m0/p0-2/macos-native-package-tree-evidence-20261005.md`.
Native OS gate primitive at `80b727f` now holds a persistent fs2 lock across
cooperating pilot callbacks; startup/handover contenders refuse it, callback
exceptions release it, and killed helpers invalidate the lease. Existing Desktop
startup/transaction paths are not integrated and arbitrary callbacks are not
automatically canceled. Each later phase must honor abort and check `assertHeld`.
Windows: Rust **14/14**, Node **86/86 PASS** twice on final source, strict Clippy,
locked build, format, two syntax and diff checks PASS. Earlier helper spawn
`UNKNOWN` failure is retained in evidence, with no established cause or bypass.
Evidence: `evidence/m0/p0-2/windows-native-handover-gate-evidence-20261005.md`.
Mac plan: `evidence/m0/p0-2/macos-native-handover-gate-test-plan-20261005.md`.
Mac gate parity: Rust **14/14**, Node **86/86 PASS**, no failures/skips, exit 0 at
`6a7d294`, macOS 27.0.1 arm64, Rust/Cargo 1.98.1, Node v24.19.0. Format, strict
Clippy, locked build, two syntax and diff checks passed; fixture/child/flush
cleanup was error-free. Mac changed no files. Evidence:
`evidence/m0/p0-2/macos-native-handover-gate-evidence-20261005.md`.
Isolated owned session slice at `dd5021b` adds pre-Tauri pilot runtime validation,
separate instance identity, initially hidden windows, bundled sidecar binding,
blocked pilot autostart changes and clean quit control. Windows Node **97/97**,
format, strict Desktop Clippy, frontend/locked release build and five syntax
checks PASS. Local Desktop debug unit execution is BLOCKED by Application Control
4551; it is not counted as a PASS. One targeted Windows package/unit workflow
37308202823 completed SUCCESS at `dd5021b`: release Desktop **17/17**, sidecar
staging **3/3**, unsigned NSIS build and artifact upload PASS. Other workflow jobs
were skipped. Artifact ID 11344842389 has source/file-hash provenance. Independent
CI closes unit verification; the local debug policy block remains historical.
Real local loose release sessions PASS: Desktop/daemon `33552/3364`, then
`460/10924`, matching OS parent/executable identities, protocol-1 IPC, clean exit
0, gone daemon PID, released lock and absent endpoint. Completed launch snapshots
are hidden; this is not physical no-flash confirmation. Early intermediate
snapshot failures were preserved and the readiness event was corrected with a
regression. Same package reopened; no selection activation or rollback yet.
Evidence: `evidence/m0/p0-2/windows-owned-desktop-session-evidence-20261005.md`.
Mac plan: `evidence/m0/p0-2/macos-owned-desktop-session-test-plan-20261005.md`.
Initial Mac owned-session attempt at `8152267` was **NOT YET VERIFIED**: Rust test build
exited 101 before execution because main and the config test both generated a
Tauri context, embedding duplicate `_EMBED_INFO_PLIST` symbols. Format/diff
passed; Clippy, packaging, Node and live sessions were not attempted.
Fix `49e4e5d` tests the config transform directly with two visible windows and
preserves identity/window assertions; main is the sole context generator.
Windows local Desktop **17/17**, format and strict all-target Clippy PASS on this
fix without Actions or policy changes. This does not supersede the historical
4551 result or independently verify Mac. The Mac operator subsequently reported
the full updated plan **PASS** at `6712269b6fffe5d39df81d273bf3e46585a4e0dd`:
macOS 27.0.1 arm64, Node v24.19.0, Rust/Cargo 1.98.1, pnpm 11.28.0;
Desktop **10/10**, helper **14/14**, Node **97/97**, no failures/skips,
format/strict Clippy/helper build/current arm64 .app packaging/five syntax/diff
checks PASS, all exits 0. Owned Desktop/daemon pairs `72303/72313` and
`72322/72330` had correct executable/parent identities (Desktop parent runner
`72300`), protocol-1 health and held locks. Both clean exits were 0 with gone
daemon PIDs, released locks and absent IPC endpoint; native gate cleanup passed.
Snapshots were hidden; physical no-flash/focus observation remains unverified.
Mac changed no files or commits and ran no Actions. This closes only the
same-package owned-session Mac gate, not update/rollback. Report path/hash and
the initial failure are preserved in:
`evidence/m0/p0-2/macos-owned-desktop-session-evidence-20261005.md`.
Native selection slice `13b435d` now stages complete baseline/candidate slots,
writes a separate flushed pending intent and replaces selection under the held
native gate after proven baseline shutdown. Real candidate health failure with
proven cleanup restores selection and restarts baseline before clearing intent.
Uncertain cleanup/failed rollback retains diagnostics and reports failure;
pending intent blocks startup/handover. No interrupted recovery is implemented.
Windows **110/110 Node**, Desktop **17/17** and helper **14/14**, format/strict
Clippy/four syntax/diff checks PASS. Actual compatible candidate commit and
protocol-2-declared vs protocol-1-real rejection/baseline restart PASS, with
clean exits 0, correct OS identities, lock release and endpoint absence.
Both slots use the same prior pilot-enabled release bytes; this is selection
and rollback evidence, not a version upgrade or installed updater.
Evidence: `evidence/m0/p0-2/windows-native-selection-evidence-20261005.md`.
Mac request: `evidence/m0/p0-2/macos-native-selection-test-plan-20261005.md`.
Mac operator reported parity PASS at
`b18c28b39428bd87b4117e4bc1b461dc0798ccee`: macOS 27.0.1 arm64,
Node v24.19.0, Rust/Cargo 1.98.1, pnpm 11.28.0; Desktop **10/10**, helper
**14/14**, Node **110/110**, no failures/skips. Format, strict Clippy, helper
build, current arm64 .app packaging, four syntax and diff checks passed;
all plan/runner exits 0. Compatible baseline/candidate pairs `73913/73921`
and `73937/73947` committed candidate. Protocol-reject pairs `73959/73967`,
`73977/73984`, then `73993/74000` rejected real protocol mismatch (helper exit
2), cleaned candidate and restarted healthy baseline. Parent/executable
identities matched runner 73910 and owning Desktops. All five clean exits 0,
PIDs gone, locks/gates released, IPC absent and both intents removed.
Hidden snapshots passed; physical flash/focus remains unverified. Mac changed
no files or commits. Evidence:
`evidence/m0/p0-2/macos-native-selection-evidence-20261005.md`.
No Actions or installed startup changes on either host.
Next implementation is interrupted native handover recovery with owned-child
absence/ambiguous-state refusal. Signing, canonical acceptance, native
interruption and version-changing update remain open; ADR-0012 stays PROPOSED.

### Latest native interrupted-claim cleanup â€” Windows and Mac scoped PASS

A test-only Rust command `clear-interrupted-recovery-claim` serializes claim
cleanup with the already pinned fs2 0.4.3 OS lock. It validates an owned,
outside-repository root without links/junctions, requires a valid claim and OS
ESRCH for its owner, rechecks identical claim bytes and removes only that claim.
The OS lock file is persistent and never unlinked. Node adapter
`recoverAfterInterruption` calls the helper then existing guarded recovery;
another caller winning the new claim causes safe refusal. This does not lock
all updater operations or integrate an installed updater.
Windows: Rust **13/13**, Node **51 transaction + 4 health adapter + 6 native
recovery = 61/61 PASS**, no failures/skips. Strict Clippy, locked build, two new
Node syntax checks and diff check passed. `cargo fmt --all` was blocked by
Application Control (os error 4551); the policy was not bypassed.
Mac initially failed format at `486d86c`, applied the authorized formatter-only
repair to `package_recovery_claim.rs` in `7011fc7`, then passed Rust **13/13**
and Node **61/61**, zero failures/skips and test exit codes 0. Format, strict
Clippy, locked build, both syntax checks and diff check passed on macOS 27.0.1
arm64 with Rust/Cargo 1.98.1 and Node v24.19.0. No POSIX flush or child cleanup
error was reported; no fixture remained. Windows reviewed the formatting commit.
No dependency versions changed; Cargo.lock adds only the direct
fs2 edge for the lifecycle tool. No Actions, GUI or installed runtime was used.
Evidence: `evidence/m0/p0-2/windows-package-native-recovery-evidence-20261004.md`.
Mac plan: `evidence/m0/p0-2/macos-package-native-recovery-test-plan-20261004.md`.
Mac result: `evidence/m0/p0-2/macos-package-native-recovery-evidence-20261004.md`.
Native cleaner SIGKILL releases its OS lock; dead recovery claim cleanup and
baseline retry are verified. Broader stress, signing, native Desktop handover,
canonical-data and power-loss durability remain open; ADR-0012 is PROPOSED.

### Latest concurrent recovery slice â€” Windows and Mac scoped PASS

The test-only candidate exclusively creates `recovery-claim.json` before
inspecting pending intent. A second recovery and updater activation while the
claim exists fail with `RECOVERY_BUSY`. Normal release verifies claim contents.
Three separate-process regressions pass: competing recovery/updater refusal;
a prevalidated updater arriving after old-intent removal cannot activate and
its new journal survives the older recovery; and killed recovery retains claim
and journal, refusing automatic reclamation. Recovery no longer automatically
continues after interruption inside this claim; retained claim needs inspection.
Windows: **51 transaction + 4 health adapter = 55/55 PASS**, no failures/skips.
Four syntax checks and diff check passed. Prior faults, owner checks, process
races and five updater SIGKILL boundaries passed.
Evidence: `evidence/m0/p0-2/windows-package-recovery-claim-evidence-20261004.md`.
Mac plan: `evidence/m0/p0-2/macos-package-recovery-claim-test-plan-20261004.md`.
Mac operator/Codex reported PASS at
`0ce9c93f621dceedd2edb78d736eb526f760a817`, macOS 27.0.1 arm64,
Node v24.19.0: 51 transaction + 4 health adapter = 55/55, no failures/skips,
exit 0. All three claim tests, previous faults/owner/process races and five
updater SIGKILL boundaries passed. Four syntax checks and diff check passed;
no fixture remained and owned-child cleanup was error-free. Mac changed no
files and its branch was clean. Evidence:
`evidence/m0/p0-2/macos-package-recovery-claim-evidence-20261004.md`.
This is fail-closed candidate exclusion, not an OS lock or production selection.
Native automatic recovery after recovery-process crash, broad stress, publisher
trust, handover, canonical-data and durability gates remain open.

### Latest live-updater recovery guard â€” Windows and Mac scoped PASS

The filesystem candidate now writes schema-2 pending journals with process PID
and transaction UUID. External recovery permits rollback only after the OS
reports ESRCH for the recorded owner. Live/ambiguous owners are refused without
changing selection or intent. Legacy ownerless intents are retained for diagnosis.
Internal automatic rollback requires exact matching of the current invocation's
journal; changed owner content is rejected. An external retry after failed
automatic rollback waits for owner process exit, even if its update call ended.
PID reuse conservatively blocks recovery; no OS process lock is claimed.
Windows: **48 transaction + 4 health adapter = 52/52 PASS**, zero failures/skips.
Seven new cases cover recovery refusal at intent/selected/healthy, successful
baseline restoration after owned updater termination, continued live-owner
commit after refusal, legacy/malformed owner refusal and changed-token refusal.
Four syntax checks and diff check passed.
Evidence: `evidence/m0/p0-2/windows-package-live-owner-evidence-20261004.md`.
Mac plan: `evidence/m0/p0-2/macos-package-live-owner-test-plan-20261004.md`.
Mac operator/Codex reported PASS at
`b05be5e07b8289c6eedfd2fc6303a213511e4839`, macOS 27.0.1 arm64,
Node v24.19.0: 48 transaction + 4 health adapter = 52/52, no failures/skips,
exit 0. All new owner-guard cases, prior races/I/O faults and five SIGKILL
boundaries passed. Four syntax checks and diff check passed; no fixture remained
and owned-child cleanup was error-free. Mac changed no files and its branch
was clean. Evidence:
`evidence/m0/p0-2/macos-package-live-owner-evidence-20261004.md`.
Simultaneous recovery callers, broad stress, native handover/signing, durability
and canonical-data acceptance remain open; ADR-0012 remains PROPOSED.

### Latest separate-process updater slice â€” Windows and Mac scoped PASS

Four new tests run two actual owned Node updater subprocesses against one fresh
outside-repository package fixture. IPC barriers control staging and selection
validation; both processes exit before cleanup. Competing commits preserve the
winner and reject the stale caller, an existing pending intent refuses a second
updater, and simultaneously released validated contenders produce exactly one
successful commit plus one typed refusal. Post-exit recovery is idempotent and
synthetic fixture inventory assertions pass.
Windows: **41 transaction + 4 health adapter = 45/45 PASS**, no failures/skips.
Syntax checks and diff checks passed. No engine behavior changed for this slice.
Evidence: `evidence/m0/p0-2/windows-package-process-race-evidence-20261004.md`.
Mac plan: `evidence/m0/p0-2/macos-package-process-race-test-plan-20261004.md`.
Mac operator/Codex reported PASS at
`6b8776c479efcb3091985a7c90f7987ba74d2e2e`, macOS 27.0.1 arm64,
Node v24.19.0: 41 transaction + 4 health adapter = 45/45, no failures/skips,
exit 0. All four process races, previous recovery/concurrency tests and five
SIGKILL boundaries passed; three syntax checks and diff check passed. No fixture
remained, owned-child cleanup was error-free, and Mac changed no files.
Evidence: `evidence/m0/p0-2/macos-package-process-race-evidence-20261004.md`.
These are four controlled process races, not broad stress or an OS process lock.
Recovery competing with an active updater remains unverified. Native handover,
signing, power-loss durability, canonical-data acceptance and ADR-0012 remain
open. No GUI, installed runtime or Actions was used.

### Latest recovery-I/O slice â€” Windows and Mac scoped PASS

The test-only transaction candidate now wraps recovery mutation failures in
`RECOVERY_IO_FAILED`, retaining the underlying cause and structured phase,
OS error code, and observed intent-presence diagnostics. Six new regressions
inject write/flush/rename/unlink failures and automatic rollback failure after
rejected/throwing health. The four direct faults preserve intent bytes and
package slot inventories, then recover the baseline idempotently on retry.
Automatic rollback faults do not report that restoration succeeded.
Windows Node v24.19.0: **37/37 transaction tests + 4/4 health adapter tests PASS**,
zero failures/skips; engine/test syntax and diff checks passed.
Evidence: `evidence/m0/p0-2/windows-package-recovery-io-evidence-20261003.md`.
Mac request: `evidence/m0/p0-2/macos-package-recovery-io-test-plan-20261003.md`.
The Mac operator/Codex reported PASS at source
`9d9159c30af01b45af1c5c9f9994417f3beec611` on macOS 27.0.1 arm64,
Node v24.19.0: 37 transaction + 4 health adapter tests = 41/41, no failures or
skips, exit 0. All six fault cases, controlled concurrency and five SIGKILL
boundaries passed, as did syntax/diff checks. No temporary fixture remained;
Mac changed no files and its branch was clean.
Evidence: `evidence/m0/p0-2/macos-package-recovery-io-evidence-20261004.md`.
These are injected operation errors, not physical disk-full or ACL tests.
Final directory-flush failure after unlink cannot promise journal retention;
power-loss/native durability and cross-process serialization remain open.
No installed app, GUI, startup registration, canonical data, or Actions changed.

### Latest package-candidate concurrency slice â€” Windows and Mac scoped PASS

Review of the serialized filesystem candidate found a lost-selection race when
one updater paused after staging and another committed. The candidate now
revalidates the selected package before creating its intent, claims the intent
exclusively, then verifies the selection again before activation. It removes
only its content-identical owned intent when it detects that another package won;
unexpected intent content is retained and reported as a typed error.
Windows Node 24.19.0 ran the focused transaction suite: **31/31 passed**, zero
failures/skips, including two deterministic races (commit during staging and
commit between validation and intent creation), all five actual worker-kill
boundaries, and synthetic fixture preservation. `node --check` for engine and
tests plus `git diff --check` passed. Evidence:
`evidence/m0/p0-2/windows-package-concurrent-transition-evidence-20261003.md`.
Exact Mac parity request:
`evidence/m0/p0-2/macos-package-concurrent-transition-test-plan-20261003.md`.
The Mac operator/Codex reported parity PASS at
`ad379892ebd26f74544d37aa56e2552cc71daeee`, macOS 27.0.1 arm64,
Node v24.19.0: 31/31 tests, zero failures/skips, both controlled races and all
five SIGKILL boundaries passed. Syntax/diff checks passed, temporary roots were
cleaned, and the Mac branch was clean/synchronized. Mac changed no files.
Evidence: `evidence/m0/p0-2/macos-package-concurrent-transition-evidence-20261003.md`.

This remains a test-only serialized selection candidate. It has no OS-level
process lock, publisher signature validation, Desktop executable handover, or
installed updater integration. Recovery-I/O failures, stress
concurrency, native executable locks, canonical-data acceptance, and the
explicit ADR-0012 orchestration decision remain open. Do not accept ADR-0012;
do not run Actions for this slice.

- root cause of Windows HKCU Run post-login daemon exit
- Windows crash-restart parity for a no-elevation/native candidate
- Tauri renderer health/reconnect invocation is physically verified on both
  platforms for clean daemon stop/replacement and external Mode B crash/restart.
  Literal macOS traffic-light close and operator Dock restore are VERIFIED at
  evidence commit `52e9dc4` (test build source `aed973d`): Desktop PID `27505`, daemon PID `27600` unchanged after restore,
  `source=tauri_close_requested`, clean exit 0 and released daemon lock.
  See `evidence/m0/p0-2/macos-physical-traffic-light-close-evidence-20261003.md`.
  That historical run did not test the production tray menu or packaged
  supervision. The later Mac release Desktop-owned run is recorded at the top
  of this handoff; its physical tray Open/Quit checks now pass. Signed package
  verification remains open.
- Windows cross-account ACL denial verified on a CI VM; production Desktop/CLI/MCP
  integration remains absent
- real compatible Desktop + daemon package coordination and rollback/canonical-data proof
- production-backed Mode A vs Mode B vs justified hybrid comparison
- ADR-0012 acceptance

## Historical M0 BLOCKED note (superseded)

> **Historical only.** The text below reflected M0 closeout constraints before
> PR #5 merge and M1 authorization. It is retained for audit trail and must not
> be read as current status.
>
> **Current status (see top of this handoff):** M0 Foundation is COMPLETE at
> `8de3c8f`; M1 is active on `m1/brain-device-identity`; ADR-0015 is ACCEPTED;
> ADR-0014 remains PROPOSED.

~~P0-2 cannot be considered complete and ADR-0012 must not be accepted until the
current revised M0 criteria, scoped review, required CI and human approval pass.
Historical production prerequisites in earlier slices remain separate mandatory
product gates under the owner-authorized revision; no production proof is waived.

M1 is not authorized until all M0 P0 gates are complete.~~
