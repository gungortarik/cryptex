# M8 completion ledger

Updated 2026-11-09. Status: **OPEN**. Authority: Master Spec, accepted ADRs,
M8 contract and Definition of Done. This ledger records evidence; it does not
relax protocol acceptance or authorize production activation.

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
  The previously authorized synthetic XPC feasibility probe is still NOT RUN in
  `evidence/m8/m8-macos-xpc-peer-identity-feasibility-20261025.md`; complete that
  bounded probe/review first. It does not authorize replacing ADR-0007's production
  Unix-socket IPC. Any production architecture amendment needs separate acceptance.
  Latest source59411433 personal-Mac prerequisite receipt passed API compile/
  presence and enumerated one valid file-Keychain signing certificate; the
  earlier borrowed-Mac receipt had zero. Select the existing identity locally;
  actual signing/private-key use and different-signer fixture availability remain
  untested. Do not repeat API preflight or completed Windows pairing checks.
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
