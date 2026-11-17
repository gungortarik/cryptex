# M8 completion ledger

## November 17 current product checkpoint

Test-only source `ed44709` passed CI44172483591: Windows104/Mac30 checks, zero
failures, both disposable standard-user account/profile cleanup markers passed.
The actual normal-Owner exchange/offline/replay test now also passes live
Compromised cutoff reconciliation, revoked reader refusal, LAN listener removal,
old-settings refusal, retained accepted search, restricted restart and local
self-retirement/restart with no transport authority. Active security packs and
immutable sealed bytes remain unchanged across reopen. Shared renderer status
checks10/10 pass locally. [Exact scope and fixture corrections](../evidence/m8/m8-live-owner-restriction-20261117.json).
No new product package is required. This is not physical cross-device restriction
or terminal self-retirement dissemination; the latter remains explicitly outside
the current sealed-export checkpoint. See the finite current closure map in
`docs/M8_REMAINING_ACCEPTANCE.md`; final candidate review and M8 remain OPEN.

Follow-up native offline backlog test exposed a Windows OS33 lock-contention
classification bug. Fix `2107297` retries only fs2's native contention code
within the existing deadline. Exact test source `476c47f` differs only in workflow
YAML and passed CI44166454534: Windows99 policy/writer/publication +5 native,
Mac30 canonical writer tests. The actual normal-Owner pause/offline/resume/restart
scenario passed in131.17s with equal canonical sets and unchanged earlier bytes.
[Failure, reproduction, fix and verification](../evidence/m8/m8-normal-offline-backlog-20261117.json).
This is two processes on one CI host; physical Windows-Mac interruption acceptance
is not implied. Owner confirms applying this additional VM contention fix and
saving Pause sync=false; this is owner observation, not a fresh binary attestation.

Normal Windows incoming discovery/publication is integrated through `19c1da9`;
source `06655f5` passed 73 scoped native Windows checks, including two actual
normal Owners exchanging offline notes in both directions and replaying after
restart with unchanged canonical bytes. Mac regression and unsigned VM package
build also passed. [Exact automated evidence](../evidence/m8/m8-windows-normal-incoming-20261117.json).

Owner now confirms the existing Mac-origin note is visible in the updated VM,
following earlier Windows-origin visibility on Mac. Thus normal-product physical
bidirectional delivery/search is owner-observed; it is not merely local readiness.
[Observation and limits](../evidence/m8/m8-owner-bidirectional-delivery-20261117.json).
Owner additionally confirms the received note remains searchable after normal
VM quit/reopen with sync paused, matching Mac's earlier paused-reopen observation.
Independently evidenced cross-device convergence/interruption, integrated restriction/status acceptance and final
security/human review remain separate gates. M8 stays OPEN. Do not repeat pairing,
forward delivery or unchanged CI, or require another diagnostic ZIP for success.

The dated entries below are historical source-specific checkpoints. Claims there
that normal incoming integration or peer configuration is absent do not describe
the current source; use the current handoff and latest evidence for continuation.

Supervised install/start e03218c: backend Windows worker and authenticated Mac
helper now retain fresh canonical policy/admission through installed readback,
owned REST readiness and final correlated acknowledgement. Epoch/hold/namespace
invalidation denies stale readiness; runtime exclusion survives draining workers.
Local17 exchange/9 admission/11 native profile/8 worker tests, real Windows
first-install/restart/hold/close fixture, format/clippy and Python12+7 passed.
Independent source review has no remaining blocker. Native CI43796009611 PASSED
both platforms on exactbd4a691; Mac's actual signed helper reports positive
install/restart/revocation/close. Windows image tests passed under the real standard
user with4 enumerated transport suites and account cleanup. Initial43795035199
Windows green is rejected: detailed logs exposed a masked test failure under the
elevated runner; fixed CI checks/account selection without weakening ownership.
[Evidence](../evidence/m8/m8-supervised-exchange-start-20261115.json). Product settings ownership,
active roster reconciliation, sealed export and real Windows-Mac convergence remain
OPEN. No new owner package; M8 is not closed.

Latest persistence boundary1e1a54b / exact CI source2238073: a module-private
first-install/recovery transaction, native bounded replacement and original Mac
ancestor-edge validation. Existing bootstrap refuses every exchange marker/stage.
No worker/helper install or exchange-mode launch is exposed. CI43790365795 PASSED
Windows and Mac; Windows8 persistence tests plus native profile11/1 ignored,
Mac feature-enabled private/persistence suites via the signed bounded runner,
existing signed startup/import/restart and credential/account cleanup. Local
format/clippy/Python7 pass; local Rust execution blocked by Application Control4551.
Independent source review's committed-retry flush finding was fixed and tested.
Fault injection/reopen is not a power-loss or abrupt-process-kill acceptance.
Admitted install/start/ack, live reconciliation, helper-owned exchange roundtrip,
sealed export and actual Windows-Mac durable convergence remain OPEN. No owner
packet requested. [Evidence](../evidence/m8/m8-exchange-persistence-20261115.json).

Latest compiler faacc8c/native probes1c43a94: exact trust-derived peer/folder XML and
schema1 derived binding readback implemented. Local7 eligibility/12 XML tests,
format/clippy and actual pinned Windows two-start config roundtrip pass. Independent
compiler source review has no actionable finding. Corrected CI43786002606 PASSED
Mac and Windows; logs confirm5 compiler tests each, Mac role refusal, existing
signed native pairing/import/restart, and credential/account cleanup. Windows native
two-start fixture ran locally, not in CI;6 broader ignored Mac tests remain excluded.
Mac probe proves unauthenticated-helper launch refusal only. Positive Mac config
roundtrip remains OPEN and must use the authenticated helper, not a libtest bypass.
Configuration installation/recovery, helper acknowledgement, live reconciliation
and bounded sealed export remain OPEN; no network activation or owner packet.
[Detailed scope](../evidence/m8/m8-strict-exchange-configuration-20261115.json).

Latest implementation3ef998d/73ce74c: locked canonical transport eligibility
view and separate retained-history incoming binding. Local7 policy,5 incoming
controller and1 beyond-cutoff publication tests pass; format and daemon clippy
pass. Independent source review confirms the historical replay separation.
Native CI43781475685 PASSED Mac and Windows. Completed logs confirm native
signed pairing/import/restart and isolated credential cleanup, plus Windows
standard-user7+5+1 policy/controller/cutoff tests and account cleanup. Broader
Mac suites retain6 explicitly ignored tests; they are not claimed passed.
Native XML peer/folder configuration, derived
restart binding and admitted sealed export remain OPEN; no network activation
or new owner package. See current handoff for the exact continuation boundary.

Latest owner c2026149 PASSED the bounded independent helper shutdown gate:
natural exit0, helper topology observed, independent absence before collector
cleanup, quiet afterward, all12 command diagnostics successful and all five
pairing/reopen/refusal/cleanup flags true. One frozen pre-spawn opaque instance
is excluded; its executable identity is unknown and it is not claimed stopped.
CRC/source/launcher and acceptance fields verified. Do not repeat this unchanged
successful packet. Product peer/folder policy, sealed export and real Windows-Mac
durable network acceptance remain OPEN; M8 is not complete.
[Owner acceptance](../evidence/m8/m8-owner-helper-shutdown-accepted-20261115.json).

Earlier350ed69b diagnostic identified live-process path/ENOENT(2). Corrected
observer f7e5c00 freezes only pre-spawn opaque PID/births; new/tracked/reused
opaque births and other errors still fail. Native CI43777096942 PASSED32
window/inventory tests, including real libproc failure injection and orphan
negatives, plus2 product-key and3 CI credential tests. Independent source review
has no actionable fixture finding. Native application/export43777097110 PASSED
Mac and Windows, including the actual exported transport launcher. New owner
packet Cryptex-Mac-Kapanis-f7e5c00.zip is ready, expiry2026-11-16T23:54:15Z;
owner independent shutdown subsequently passed above. Real Windows-Mac durable
sync remains OPEN.
[Correction scope and receipt](../evidence/m8/m8-owner-helper-opaque-baseline-20261115.json).

Earlier owner helper result f6e60eb8 FAILED at inventory_unavailable before pairing
driver spawn. Earlier native CI stays passed; real-owner independent shutdown is
not accepted by that failed receipt (superseded by c2026149 above). Three
identity/refusal stages passed, but the full result did not.
Read-only diagnostic ef28d58 passes native CI43767799632 and is ready for the
owner. It adds fixed stage/errno metadata without relaxing process visibility or
launching Cryptex. See [failure and handoff](../evidence/m8/m8-owner-helper-inventory-failure-20261115.json).

Latest helper observation slice: [receipt](../evidence/m8/m8-independent-helper-shutdown-20261115.json)
binds f7fe55f to passed Darwin collector CI43763552289 and scoped Mac/Windows
application CI43763653794. The exported signed local pairing/import/reopen driver
requires user-wide image/instance absence before collector group cleanup and
later quiet samples; actual separate-session orphan negatives pass. Independent
source review has no actionable security finding. Owner packet is ready, receipt
pending. This is local native evidence, not actual Windows-Mac durable delivery;
trust-derived product peer/folder policy and daemon export remain prerequisites.

November15 continuation: [owner normal result b77b619e](../evidence/m8/m8-mac-normal-owner-acceptance-20261115.json)
PASSED on product d3927b4/collector 1c0e3c3: two no-argument natural exit0
launches at the actual default root, ten completed Keychain reads, four positive
visual confirmations, unchanged canonical/device state and exact key/runtime
cleanup. The initially absent default root held synthetic test history; personal
profile migration is not covered. Product-exit shutdown is checked, independent
out-of-group helper enumeration is not. Do not repeat the unchanged owner package.
Pinned native43761107215 passed actual nonempty signed product startup/restart,
modified-inventory refusal, unchanged canonical/public identity, exact synthetic
credential permissions and full Keychain preference restoration/deletion. Product
images21f6bf7; collector59f9ba5. This uses owner-approved test access provisioning,
not automatic owner permission. The CI Keychain is a new isolated child under the
hosted runner's Library/Keychains; zero/missing/extra partition ACL is refused.
Final scoped Mac+Windows startup CI43761233562 PASSED on59f9ba5; actual
signed startup/restart, tampered inventory refusal, both Keychain cleanup markers
and Windows standard-user/account cleanup are verified in completed job logs.
Six explicitly ignored Mac tests are not claimed passed. See
[current handoff](../docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md) and
[evidence](../evidence/m8/m8-mac-keychain-partition-20261115.json). M8 remains OPEN.

Updated 2026-11-15. Status: **OPEN**. Authority: Master Spec, accepted ADRs,
M8 contract and Definition of Done. This ledger records evidence; it does not
relax protocol acceptance or authorize production activation.

Latest bounded window acceptance: [owner return4692ce47](../evidence/m8/m8-mac-window-owner-acceptance-20261115.json)
passes the explicit-existing-profile shared product UI on d3927b4 with real HOME
retained:10 positive owner observations,3 natural exit0 launches,10 completed
Keychain reads, unchanged canonical bytes and exact test-key/runtime cleanup.
Reopen/Cancel preserve the sampled daemon PID set and owner-reported draft.
This closes that window exercise only; do not repeat the unchanged package.
Normal default-profile acceptance, headless CI Keychain startup, OS denial,
independent helper enumeration, fresh-session replay and cross-device sync are
not established by this receipt. Full M8 status remains OPEN. Follow the current
handoff for the continuation point; earlier source-specific notes remain historical.

Latest bounded integration: [Darwin filesystem/native existing-profile review](../evidence/m8/m8-macos-filesystem-native-integration-review-20261114.md)
and [receipt](../evidence/m8/m8-macos-filesystem-native-integration.json) bind
`d9561d8` to passed full43671790009 and native43671787980. Mac589/Windows682daemon
tests passed, zero failures; separate read-only agent review has no remaining
finding in this slice. Corrected16x8 creation/disappearing-leaf guards, original
namespace binding, actual-send admission/reconciliation and live signed import/
restart checks passed in CI. The returned owner packet failed at native pairing;
identity checks passed. The diagnostic return identifies a clean incoming
`Rejected`, exit2 without timeout/output/pipe failure; its inner cause is still
identified by the next validation receipt as open_fd EMFILE24. MacCI43681401473
reproduced the same rejection/EMFILE path with a256-descriptor budget. Correction
`7fc6d44` shares original ancestor pins without opening duplicate descriptors;
nativeCI43681948389 passed both platforms, including the exported Mac transport
launcher under256. The [correction receipt](../evidence/m8/m8-macos-descriptor-correction-20261115.json)
records passed fullCI43682159822 on the same source (Mac591/Windows682 daemon
tests, zero failures) and the corrected owner package. The corrected
[owner return5ca78781](../evidence/m8/m8-macos-descriptor-owner-acceptance-20261115.json)
passes all13 commands,12 expected markers and five completion flags under256;
the bounded local owner acceptance gate is closed. Earlier failed-source receipts
remain historical evidence. Ordinary activation, peer/folder/cross-device durable
replication, installed UX and full protocol/M8 acceptance remain open. This
supersedes earlier adapter-pending notes only within the default-off candidate.

Earlier bounded correction: [R4 incoming conflict retention/restart](../evidence/m8/m8-incoming-policy-hold-review-20261114.md)
and its [structured receipt](../evidence/m8/m8-incoming-policy-hold-native.json)
bind `1a042f9` to successful Mac/Windows readiness and Windows cross-account ACL
in [CI43657350874](https://github.com/gungortarik/cryptex/actions/runs/43657350874).
Daemon Mac553passed/7ignored, Windows681passed/5ignored, zero failures. All14 new
Windows tests and7 shared Mac tests passed; no actionable independent source
finding remains in this bounded correction. Full inventory intents precede
classification, held evidence survives restart without SQLite, and final holds
have no automatic M8 override. This does not admit a competing-fork witness or
activate Mac publication/product routes. Continue with the current handoff's
remaining adapters, policy/configuration and product acceptance; do not repeat
already covered owner packages or mark full M8 done from this result.

Earlier disabled integration: the [Mac transport helper evidence](../evidence/m8/m8-macos-transport-helper-20261112.md)
and [independent review](../evidence/m8/m8-macos-transport-independent-review-20261112.md)
record fresh owned engine identity, native pairing/reopen, helper/daemon loss,
eleven live drift refusals and profile-lease-fenced fixture credential cleanup.
Review through `0d98a16` found no actionable blocker; exact-source
[CI 43334196206](https://github.com/gungortarik/cryptex/actions/runs/43334196206)
passed both selected platform jobs and the exported transport launcher. The
matching [owner-Mac receipt](../evidence/m8/m8-macos-transport-owner-receipt-0616b9b2.md)
passed all five aggregate checks, closing this candidate's owner-device run.
Do not repeat that package. See the
[structured CI receipt](../evidence/m8/macos-transport-native-0d98a16-ci.json). The prior passing artifact is
superseded and withheld. Combined supervisor loss, other endpoint/exec races,
product publisher/support floor, normal activation and cross-device acceptance
remain open. Read the [current handoff](../docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md)
before the historical source checkpoints below.

Owner-approved [disabled Mac adapter foundation](../evidence/m8/m8-disabled-macos-owner-adapter-20261110.md):
sourceca5665d CI42960387437 and source review passed. Default-off outbound/Unix
inbound gates cover all M8 families with no fallback. Shared request/reply types
and admitted exchange replies are implemented; new contract/reply tests9/9 and
Mac gate2 passed. The remaining [typed approval domain/session boundary](../evidence/m8/m8-approval-domain-sessions-20261111.md)
is implemented on `449dc3c` (final tests `e0da25e`), with11 new regressions and no
actionable source-review finding. Exact CI42963863735 passed both platforms:
complete IPC Windows80/Mac71pass, four preexisting Mac ignores, zero failures.
All11 new tests passed on both; native lint/format/frontend passed. Native proof/backend,
stable product-image policy and native product acceptance remain unimplemented.
ADR-0020 authorizes continued disabled development; activation and M8 stay gated.

Existing product connection/session corrections: [source9dd0a40 CI42956476071](../evidence/m8/m8-connection-session-lifetime-review-20261110.md)
passed Windows/Mac startup9/10, Desktop46/39, approval registry17/17 and client
stream/response3/3 with zero failures/ignored; native lint/format/frontend passed.
Windows standard-user checks/cleanup and exact selected/excluded inventories were
verified. This does not enable Mac mutations or prove physical UI/signed product
custody. ADR-0020 now records the separate disabled-integration owner decision.

Authorized public message-bootstrap: [personal-Mac exact source8e8fabb receipt](../evidence/m8/m8-macos-signed-bootstrap-owner-receipt-20261110.md)
passed all19 signed native rows, certified two-binary/self-bundle eligibility,
fault controls and independent normal/forced namespace cleanup on Mac27.2 arm64.
Exact CI42952782022 and source review passed separately. The isolated experiment
is complete; no unchanged package rerun is needed. Product custody/session,
bundle replacement, Unix frame attribution and M8 acceptance remain open.

Latest owner receipt888d68a: public SDK/runtime and certified bundle eligibility
pass, but only five of eleven cases pass. All six mapped-child handoff cases fail.
The direct CLOFORK + ordinary spawn-duplication carrier is stopped; no unchanged
owner rerun is requested. Published kernel ordering supports the diagnosis, but
actual spawn errno was not recorded. Neither custody finding is closed; see
[receipt and next decision](../evidence/m8/m8-macos-launch-owner-receipt-2-20261110.md).
Older pending-run entries below are historical, superseded by this result.

Personal-Mac b234322 launch receipt: incomplete, not a positive case pass. The
TeamID query omitted the API's required Signing flag; this is a harness bug, not
proof of a missing certificate. Corrected888d68a passed CI42914376385, with direct
per-declaration public probes and the proper metadata request. One updated native
run remains pending. See [receipt and correction](../evidence/m8/m8-macos-launch-owner-receipt-20261110.md).

Earlier reviewed candidate b234322 addresses the two launch-draft findings with
atomic public Mac27 descriptor creation and a verified-live-self organization/
sealed-counterpart policy. CI42911736801 passed only compile/refusal/scoped
regressions on Mac26; eleven positive owner-native observations remain pending.
See [candidate and exact owner action](../evidence/m8/m8-macos-atomic-launch-candidate-20261110.md).
No production gate, minimum-OS decision, bidirectional pin or bundle-race acceptance
is implied by packaging this candidate.

| M8 requirement | Current evidence | Remaining closure evidence |
| --- | --- | --- |
| Pairing and explicit local owner actions | Windows signed exchange, durable anchors, cancel/refusal/restart receipts; local journal/IPC tests | Native macOS identity/connection binding and full reviewed protocol acceptance; final product UI review |
| Managed transport and incoming validation | Actual Windows host-to-VM worker transfer/publication/restart/replay passed on c669a9a | Supported production policy/configuration and startup feeder remain disabled; activate only after protocol/platform/security gates |
| Offline convergence | New daemon integration: both peers write offline, reversed incoming hint order converges to identical canonical hashes/search, replay adds nothing | Actual disconnected/reconnected cross-device sequence through supported product integration; macOS parity |
| Retire/compromise cutoff | New daemon integration: real owner journal confirmation/appended restriction at verified seq 3 rejects offline seq 4 for retired and compromised; authorized history remains searchable | Restriction propagation and reader/issuer conflict handling across real installations; native product integration |
| Truthful protection status | Typed transport/trust status and fail-closed local tests | Product flow must accurately report local durable action vs peer propagation/confirmation, including offline/restricted peers |
| Windows platform | Actual native pilot on 25H2/NTFS; long-path publication and recovery tests | Native Desktop authorization beyond the test IPC seam; final scoped security/platform review |
| macOS platform | Prior selected native coordination checks passed on exact earlier candidate | Production publication ancestor/volume/no-replace/IPC authorization parity, accepted implementation scope, exact-candidate native verification |
| Security review | Prior technical independent agent source reviews and corrected findings | Final candidate review including recent platform/native integration changes; required human diff review |
| CI | [Run42564404285](https://github.com/gungortarik/cryptex/actions/runs/42564404285) passed Windows/macOS readiness and different-user IPC ACL on source6de8dbe; Windows library596/0failed/5ignored, Mac464/0failed/4ignored | Revalidate changed behavior on later implementation candidates; explicit native/activation/human gates remain separate |

Do not repeat completed Windows native pairing/transfer packages. Existing result
receipt is in `evidence/m8/m8-native-controller-result-20261108.md`.

Immediate sequence: local offline/restriction integration and exact-code CI passed;
prepare remaining Mac/platform work against the accepted
protocol scope, then consolidate final native integration/security/human review.
Any additional owner/device action should be packaged once against a concrete
candidate. M9 starts only after this milestone gate is green.

Latest launch correction: source4229585 requires real supervised-child readiness
on Desktop startup/restart, with local Windows evidence; fixture/CI source867c893
passed scoped native CI42906355843: startup Windows9/Mac10 and Desktop debug
Windows46/Mac39, all zero failures; native clippy/format passed. The endpoint fixture requires a disposable
account, with real standard-user token/owner checks on Windows. The Mac custody draft was withdrawn for non-atomic pipe
inheritance and missing trusted release-image anchor; socket-owner sampling is
not per-frame sender proof. Owner-operation gates stay closed. See
[launch review/continuation](../evidence/m8/m8-desktop-startup-review-20261110.md).

## Next consolidated Mac implementation/verification batch

Source audit at18774b2 confirms these are implementation gaps, not just missing
screenshots or an unrun test package:

- `managed_profile/mod.rs` private profile handle and `native.rs` prepare/reopen
  refuse non-Windows; `worker.rs` depends on WindowsCredentialStore. Add verified
  Mac platform adapters under the existing lifecycle/credential contracts.
- `lib.rs` excludes incoming_publication on Mac; non-Windows
  `import_staging::inspect_directory_chain` returns no pinned ancestor handles.
  Establish equivalent containment/owner/no-follow/volume/no-replace/directory
  durability guarantees before connecting the publication worker.
- `ipc.rs` owner-security preparation and bootstrap completion deliberately refuse
  `platform_peer_identity_unavailable` on Mac. Bind the exact live Desktop
  connection/process instance before enabling those operations; a PID match alone
  cannot replace that proof. Preserve stale/cross-connection confirmation refusal.
  The previously authorized synthetic XPC feasibility probe has partial native
  owner observations in `evidence/m8/m8-macos-signed-xpc-observations-20261109.md`:
  signing/positive controls and wrong-ID/ad-hoc message rejection worked, but a
  same-requirement second process used the forwarded endpoint. Parent-launch
  exact-instance custody remains unproven; continue the bounded probe/review.
  This does not authorize replacing ADR-0007's production
  Unix-socket IPC. Any production architecture amendment needs separate acceptance.
  Latest source59411433 personal-Mac prerequisite receipt passed API compile/
  presence and enumerated one valid file-Keychain signing certificate; the
  earlier borrowed-Mac receipt had zero. Existing-key synthetic signing succeeded
  in the2026-11-10 owner run. Different certified signer, actual stale-connection
  reuse, connection-negative attribution and product minimum OS remain unverified.
  Do not repeat signing/API preflight or completed Windows pairing checks.
  Separate raw Mach sender experiment: eight CI and eight personal-Mac cases passed on85ef4b1 using
  public kernel audit-trailer PID/generation getters and retained task liveness;
  personal-Mac MAC-SUREC receipt verified2026-11-10. This is not an XPC fix or actual
  Desktop-parent custody proof. See message-instance evidence20261110; accepted
  production Unix-socket IPC and disabled owner-operation gate stay unchanged.
  Next evaluate actual Desktop-parent custody on the accepted adapter, including
  the public audit-generation getter now confirmed in the SDK. Do not infer that
  this receipt selected or accepted a different production transport.
- Keep normal startup feeder and peer/folder configuration disabled until the
  complete protocol/security acceptance gate permits activation. Passing Mac CI
  with these paths excluded/refused is useful regression evidence, not parity.
- After implementation and source review, produce one pinned Mac native batch for
  authorization, transport/publication interruption/reopen/replay, offline writes
  and restriction propagation. Reuse the completed Windows receipt; request a new
  Windows/native round only if changed behavior needs it. Do not repeat pairing.

Physical power-cut and full soak testing are not claimed by these results.
Determine the applicable M8 durability evidence from accepted storage/DoD rules;
do not silently expand this ledger into M11 recovery or M16 full stress scope.
