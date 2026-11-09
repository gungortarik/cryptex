# M8 completion ledger

Updated 2026-11-08. Status: **OPEN**. Authority: Master Spec, accepted ADRs,
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
| CI | CI workflow defines Windows/macOS readiness and Windows different-user IPC ACL | Green exact-candidate runs; local selected tests and old platform receipts are not substitutes |

Do not repeat completed Windows native pairing/transfer packages. Existing result
receipt is in `evidence/m8/m8-native-controller-result-20261108.md`.

Immediate sequence: finish local offline/restriction integration checks, run
exact-candidate CI, prepare remaining Mac/platform work against the accepted
protocol scope, then consolidate final native integration/security/human review.
Any additional owner/device action should be packaged once against a concrete
candidate. M9 starts only after this milestone gate is green.

Physical power-cut and full soak testing are not claimed by these results.
Determine the applicable M8 durability evidence from accepted storage/DoD rules;
do not silently expand this ledger into M11 recovery or M16 full stress scope.
