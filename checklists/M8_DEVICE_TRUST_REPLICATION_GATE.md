# M8 completion ledger

Updated 2026-11-12. Status: **OPEN**. Authority: Master Spec, accepted ADRs,
M8 contract and Definition of Done. This ledger records evidence; it does not
relax protocol acceptance or authorize production activation.

Latest disabled integration: the [Mac transport helper evidence](../evidence/m8/m8-macos-transport-helper-20261112.md)
and [independent review](../evidence/m8/m8-macos-transport-independent-review-20261112.md)
record fresh owned engine identity, native pairing/reopen, helper/daemon loss,
eleven live drift refusals and profile-lease-fenced fixture credential cleanup.
Review through `0d98a16` found no actionable blocker; exact-source
[CI 43334196206](https://github.com/gungortarik/cryptex/actions/runs/43334196206)
passed both selected platform jobs and the exported transport launcher. One
matching owner-Mac run remains pending. See the
[structured receipt](../evidence/m8/macos-transport-native-0d98a16-ci.json). The prior passing artifact is
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
