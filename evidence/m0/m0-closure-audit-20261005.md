# M0 closure audit and remaining work

Status: **IN PROGRESS**. Scope: complete M0, authorized by project owner.
Authority: Master Spec, accepted ADRs, M0 milestone and foundation gate checklist.
No M1 work is authorized by this audit. Checkboxes must follow verified evidence.

## Current gate inventory

Latest actionable audit: `m0-final-gate-audit-20261005.md` at source `8437559`.
The retained inventory below is historical where it mentions production blockers;
owner-authorized ADR revisions retain those as separate mandatory product gates.
Current implementation blocker before final CI: ordinary release M0 Desktop
uses the product runtime namespace on both OSes. Correct to Cryptex-Dev and
collect focused path parity; then finish review, final CI and human ADR gates.
Checklist evidence was consolidated without accepting ADRs or completing M0.

| Gate | Verified foundation | Remaining closure work |
| --- | --- | --- |
| P0-1 | Accepted ADR-0013 and physical cross-platform benchmark evidence integrated via approved normal merge `cb9069e` from `6d4d10a` | Confirm integrated branch's required checks; no corpus regeneration |
| P0-2 | Both OS lifecycle/IPC/tray/login, changed-byte native package selection/rollback and pre-successor updater-kill recovery with sealed-fixture preservation (Mac daemon bytes unchanged) | Remaining native interruption/controlled-error review; signing/installed updater and canonical fixture acceptance; ADR-0012 decision |
| P0-3 | Accepted ADR-0007 implementation addendum; physical transport, negotiation, roles, malformed frames, ACL and reconnect evidence | Final integrated security/acceptance review; do not expand production APIs |
| P0-4 | Both OS pinned 2.1.5 managed lifecycle/staged notices and real 2.1.2 -> 2.1.5 transition/cold rollback PASS; Mac upgrade 33/33, original notice failure retained | Installed distribution/notice review, secret/upgrade boundary review and ADR-0004 addendum acceptance |
| Golden Brain | Deterministic synthetic sealed-pack foundation and preservation proof on both OSes | Review of fixture scope; production event schema is outside M0 |
| Doctor | Actual daemon PID/IPC/lock health on both OSes, including recovered Mac baseline healthy | Review diagnostics; explicitly retain unimplemented canonical/Syncthing checks |
| Quality/final gate | Windows workspace 89 Rust, current lifecycle Node 160 and storage/sidecar Node 8; Mac foundation 71 Rust, 123 Node + 5 storage Node and managed/upgrade 33/33; CI coverage prepared | Final required CI at stable source, human review and final evidence-backed checklist |

P0-1 transport measurements do not satisfy managed P0-4 integration. An unchecked
item is not filled merely because similarly named code or an old config exists.

## Execution order

1. Keep accepted P0-1 decision integrated; run one combined Mac foundation/native
   interruption plan: `p0-2/macos-native-interruption-test-plan-20261005.md`.
2. Complete remaining Windows native handover cases and changed-package proof
   using fresh synthetic fixtures/slots; batch their Mac parity requirements.
3. Implement the bounded managed-Syncthing P0-4 spike against pinned 2.1.5 in
   fresh app-data homes, without altering global or retained P0-1 instances.
   Select/document packaging/supervision only through its explicit ADR process.
4. Consolidate evidence and supervision/update comparison, then present concrete
   ADR-0012/ADR-0004 decisions for human acceptance. Do not silently accept a
   proposal or waive its recorded proof requirements.
5. At stable source, run final required CI once if the physical-host evidence
   cannot supply the required check/artifact; verify required jobs, not just an
   unrelated targeted old build. Complete human review and gate checklist.

Platform polish/shipped updater productization belongs to M14. M0 must prove its
explicit spike requirements and resolve the architecture; this audit does not
authorize product feature expansion or silently remove recorded signing/update
acceptance prerequisites. Those scope distinctions must be explicit in decision
review, not resolved by marking a proposal ACCEPTED without evidence.

## Windows continuation

The current recovery slice proves one actual updater kill before successor
launch, healthy baseline restart and unchanged synthetic sealed fixtures.
Missing PID evidence/live child/recovery claim safely refuses startup. It does
not prove automatic reclamation after recovery itself dies, or a version upgrade.
See `p0-2/windows-native-interruption-evidence-20261005.md`.

Mac binary inventory for pinned Syncthing 2.1.5 is requested read-only alongside
the parity plan. Windows has 2.1.5; the historical Mac P0-1 binary was 2.1.2.
No Syncthing process or global config was changed for this audit.
No Actions was run. Final M0 completion remains unclaimed.

Windows managed spike is now recorded under `p0-4/`; global profile digest stayed
unchanged during successful runs. Source-based stop-budget correction preserves
historical forced cleanup as failure. Its Mac task is independent of the still
pending native interruption/Golden/Doctor parity plan; both are needed.

Changed native package Windows proof now PASS at `309bb66`, with actual different
Desktop/daemon hashes, compatible candidate commit and real protocol-rejected
rollback. Its Mac task is `p0-2/macos-changed-native-package-test-plan-20261005.md`.
Temporary candidate Desktop version metadata does not change the repository
release version. Installed/signed upgrade and production canonical acceptance
remain unclaimed. Current Node checks are **143/143** plus storage **5/5**.

Mac changed-package task reported PASS at exact source `9a74b40`; evidence is
`p0-2/macos-changed-native-package-evidence-20261005.md`. It closes only that
scoped parity item; native interruption/Golden/Doctor and managed Syncthing Mac
tasks still need their own results. No redundant Windows build/test rerun is
needed for recording this report.

Windows signing readiness (read-only): SDK x64 SignTool exists; CurrentUser My
contains zero currently valid code-signing certificates with private keys.
No external signing service was inspected and no key/export/security change was
made. Production signing remains an explicit unmet acceptance prerequisite.

## Native recovery parity and notice portability update

Mac native interruption/Golden/Doctor reported PASS at `502abf4`: Rust **71/71**,
Node **123/123** plus storage **5/5**, actual pre-launch updater kill, restored
baseline health and complete cleanup with unchanged synthetic sealed fixtures.
See `p0-2/macos-native-interruption-evidence-20261005.md`. This supersedes the
earlier pending parity statements above, without expanding their acceptance scope.

Mac managed Syncthing stopped before live launch with **7 PASS / 8 FAIL** because
notice hashes pinned CRLF checkout bytes instead of Git LF bytes. Fix `f17bcf7`
preserves exact-byte verification and explicit LF checkout. Corrected Windows
managed proof passes four clean stops; combined Node **150/150**, storage **5/5**.
See `p0-4/notice-byte-portability-failure-and-fix-20261005.md`, including two
intermediate failed Windows observations and their bounded polling corrections.
The subsequent Mac rerun at `501d11d` reported **22/22** plus isolated native
proof PASS, including four clean exits, unchanged global config digest and
exact staged binary/notices/POSIX modes. See
`p0-4/macos-managed-syncthing-evidence-20261005.md`. No passed Mac task needs rerun.
No Actions run. ADR-0004/0012 remain PROPOSED and M0 remains IN PROGRESS.

Windows changed-version continuation `133428b` proves an actual 2.1.2 -> 2.1.5
transition, upgraded restart/new transfer and controlled health rejection with
complete cold baseline restore/new transfer. Eight clean exits/PIDs absent,
global config observed digest unchanged; current Node **160/160** plus **5/5**.
See `p0-4/windows-syncthing-upgrade-evidence-20261005.md`. Mac parity reported
PASS at `dec5fb1`: **33/33**, four syntax/diff checks and both live scenarios,
eight clean absent monitors, preserved identity/key/payload and unchanged global
config digest. Exact source/input/report hashes and supplied artifact provenance:
`p0-4/macos-syncthing-upgrade-evidence-20261005.md`. No passed Mac task needs rerun.
This does not establish live canonical rollback, native migration failure or
crash/power-loss atomicity. Product pin remains 2.1.5.

Then finish notice and supervisor/secret/upgrade-boundary review. Same-version
restart is already proven and must not be repeated or relabeled upgrade.
P0-2 signed/installed acceptance prerequisites and human ADR decisions remain
separate gates. Final CI must cover the introduced Node foundation/failure paths;
readiness workflow now adds Desktop Rust tests, native helper/current-source release
Desktop builds, every lifecycle `*.test.mjs`, storage capture and sidecar staging
tests on both OS jobs. The readiness timeout is bounded at 35 minutes to include
the extra release build; triggers and isolated ACL/package-only selectors stay
unchanged. Local equivalent commands PASS (Desktop 17/17, Node 160/160 + 8/8).
Workflow execution is still NOT YET VERIFIED; YAML parser unavailable locally,
static review/diff and actual PowerShell test block pass. No Actions was run.
Do not substitute an older readiness run or these local checks for final CI.

## Security and notice review continuation

Latest owner decision: "kurallar dahilinde onayliyorum" approved preparing the
concrete M0/production-boundary ADR revisions. ADR-0012/0004 now state scoped M0
criteria and retain mandatory product secure storage, canonical safety, installed
updater, signed release and distribution notices separately. The roadmap assigns
packaging/updater coordination to M14 and signed/notarized release/final notices
to M17. Master Spec, accepted ADRs and M0 contract were not modified.
The earlier unanswered-question notes below are historical. Both proposals
remain PROPOSED; scoped review, required final CI and human diff/ADR approval
must pass before M0 closure. No code, tests, startup or installed state changed
for this documentation revision, and no Actions was dispatched for it.

Mac sidecar validator regression subsequently reported scoped PASS at
`388e46a7e5f750d88e7fc786e0c108dcea5329e4`: Desktop **13/13**, CLI **4/4**,
all format/Clippy/frontend/locked release/diff checks and exits 0, no skipped or
ignored tests, real POSIX symlink refusal and error-free fixture cleanup.
Exact environment/provenance/scope:
`p0-2/macos-sidecar-boundary-evidence-20261005.md`.
The pending Mac check below is historical and is now closed; physical renderer
invocation was not tested. The owner scope question remains unanswered, so no
ADR prerequisites or milestone authority change. Final CI awaits the remaining
review decision; no Actions were dispatched.

`m0-security-notice-acceptance-review-20261005.md` records the implementer review
and remaining exact prerequisites. Finding: normal Desktop sessions previously
allowed a renderer-selected executable for daemon startup. Fix `82c4d32`
constrains all requests to the backend-selected ordinary absolute sidecar before
runtime creation/launch and makes the field read-only. Windows Desktop **20/20**,
format/strict Clippy/frontend/release builds and Node **160/160 + 8/8** PASS.
New narrow Mac task: `p0-2/macos-sidecar-boundary-test-plan-20261005.md`.
This supersedes the earlier statement that no new Mac task was needed.

Golden/Doctor and nine scoped managed Syncthing checklist requirements now carry
both-platform evidence. Notice completeness/distribution, proposed secret/signed
acceptance boundaries, whole-read IPC availability and human ADR review remain
open; the security review is not marked complete. M1 secure storage/M14 signing
scope must be explicitly reconciled with the proposal's prerequisites by the
owner; no scope/authority change is inferred. No Actions run while this completion
candidate has a new pending Mac check.
