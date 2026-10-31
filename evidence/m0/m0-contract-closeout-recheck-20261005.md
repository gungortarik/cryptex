# M0 contractual closeout recheck — 2026-10-05

Reviewed checkout: `9b33d4994095f8374836cc08bb93525981a061fc`.
Owner requested a full M0 closure review against the main project rules.

## Verdict

M0's full defined milestone acceptance criteria are satisfied by the retained
evidence and accepted implementation decisions. This is completion of the M0
contract, not a reduced subset introduced by this review. Synthetic data is an
explicit requirement in M0 section 4: real applications, processes, IPC,
transport and failure paths are exercised using generated test memory.

Milestone acceptance and Git integration are distinct: PR #5 remains OPEN and
UNMERGED. GitHub reports it MERGEABLE with CLEAN merge state, targeting
`m0/foundation` at `331f1ac2e0c874b1e4a027e6bc9547d4a001058d`. Integration requires
the explicit merge authorization required by the handoff. No merge or M1 work
was performed by this review.

## Authority and acceptance mapping

The Master Spec, M0 contract, required P0 spikes, Definition of Done, roadmap,
ADR index and accepted M0 implementation records were compared. The existing
gate checklist remains the detailed item-by-item evidence index.

| Contract requirement | Evidence and outcome |
| --- | --- |
| Executable Windows/macOS Desktop, Rust workspace, daemon and IPC | Both readiness CI jobs; retained native sessions and UI review. PASS within the foundation shell scope. |
| P0-1 realistic physical storage comparison | Accepted ADR-0013 and both-platform/both-direction benchmark matrix. The rejected large one-file layout's timeout remains a failure, not a passing measurement. |
| P0-2 close/login/crash/single instance/reconnect | Both-platform physical and process evidence indexed in checklist section C; accepted Mode B decision. Close hides; explicit Quit stops the owned daemon and exits. |
| P0-2 compatible update, rejection/rollback, data preservation | Both-platform changed native package and interruption reports, complete slot selection and sealed-fixture inventory preservation. Installed product updating is assigned to M14. |
| P0-3 transport/security/protocol/roles/frame handling | Accepted ADR-0007 implementation addendum, physical transport evidence and actual Windows different-account ACL CI. |
| P0-4 managed pinned Syncthing | Both-platform native 2.1.5 lifecycle, authentication, isolated profiles, pairing, pause/resume, conflict/version history and real 2.1.2 to 2.1.5 upgrade/cold restore evidence. |
| P0-4 license/notices handling | Exact staged LICENSE/AUTHORS membership and byte checks, upstream provenance and documented distribution obligations; final distribution notices belong to M17. |
| Golden and Doctor foundation | Deterministic synthetic sealed packs and actual PID/IPC/lock health; unimplemented product checks remain explicitly `not-implemented`. |
| Security, UI, human review and ADR decisions | Retained independent review, focused frontend correction review, Windows native visual review, operator-reported Mac packaged visual review and accepted owner decisions. |
| Required final CI | Run 37387323751 independently queried again: Windows readiness, macOS readiness and Windows cross-account ACL SUCCESS. Manual unsigned installer job SKIPPED; not counted as required M0 evidence. |

## Checks performed in this review

- Queried GitHub run 37387323751: completed SUCCESS at
  `c7fa011672973101cb2a5519b5883b26c6909f90`.
- Compared that tested source to the reviewed checkout: all changed files are
  Markdown documentation. No application, service, tool, dependency or workflow
  change follows the tested source.
- Checked 37 distinct literal repository file references extracted from the
  M0 checklist and ADR index: zero missing files. Wildcards and abbreviated
  evidence names were not counted as literal file checks.
- Rehashed retained Windows `native-recovery-report.json` at
  `%LOCALAPPDATA%\Cryptex-Dev\p0-2\native-recovery-proof-20261005-8d33666a06ff49d3bf5045237547137c`:
  `712b9f2f6ac97b44e3360ff343457e6e6098d1377214a066ae58565b68d204b2`,
  matching the recorded evidence.
- Reviewed foundation source boundaries: native close prevents exit and hides
  the window; backend selects renderer-requested sidecar paths; release frontend
  mode is guarded; Golden/Doctor code declares its synthetic/unimplemented scope;
  Syncthing bundle verification checks exact inventory, notice bytes and modes.
- Verified the clean local checkout matched the remote head before this report.
  Queried PR #5 state and mergeability without performing integration.

No runtime suite, visible application, startup change or new Actions run was
needed for this documentation/review pass. Existing passing execution evidence
is reused only for the unchanged code it actually tested.

## Evidence limitations and product follow-through

Mac raw images/reports were not independently rehashed on this Windows host;
their provenance remains the operator's retained reports. This limitation is
preserved, not converted to direct Windows inspection or a new physical PASS.
The review is a source/evidence audit, not an exhaustive new execution of every
retained runtime scenario.

| Remaining product obligation | Governing roadmap milestone |
| --- | --- |
| Production Brain/device identity and OS secure storage | M1 |
| Signed canonical event schemas, integrity and quarantine | M2 |
| Production Syncthing controller, trust, private-file permissions and bounded authenticated REST | M8, with M1 secure storage prerequisite |
| Installed update coordination, migration/downgrade safety and console-free packaged startup | M14 |
| Whole-read IPC deadlines, developer file-launch gating and broad failure/power-loss/stress validation | Hardening work before release; M16 final verification, and corresponding feature milestones before production reuse |
| Publisher verification, signed/notarized artifacts, installer/source-access/dependency notices | M17 |

These obligations stay mandatory. They do not become M0 deliverables merely
because M0 has selected the underlying architecture. No acceptance criterion,
test assertion or Master Spec invariant was weakened in this recheck.
