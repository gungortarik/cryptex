# M0 closure audit and remaining work

Status: **IN PROGRESS**. Scope: complete M0, authorized by project owner.
Authority: Master Spec, accepted ADRs, M0 milestone and foundation gate checklist.
No M1 work is authorized by this audit. Checkboxes must follow verified evidence.

## Current gate inventory

| Gate | Verified foundation | Remaining closure work |
| --- | --- | --- |
| P0-1 | Accepted ADR-0013 and physical cross-platform benchmark evidence integrated via approved normal merge `cb9069e` from `6d4d10a` | Confirm integrated branch's required checks; no corpus regeneration |
| P0-2 | Both OS lifecycle/IPC/tray/login and changed-byte native package selection/rollback with sealed-fixture preservation (Mac daemon bytes unchanged) | Mac native interruption/foundation parity; remaining native interruption/controlled-error review; signing/installed updater and canonical fixture acceptance; ADR-0012 decision |
| P0-3 | Accepted ADR-0007 implementation addendum; physical transport, negotiation, roles, malformed frames, ACL and reconnect evidence | Final integrated security/acceptance review; do not expand production APIs |
| P0-4 | Windows pinned 2.1.5 managed lifecycle, two isolated native profiles, auth/pairing/pause/status/history/conflict, retained restart and clean stop; staged native role/notice preservation and proposed ADR-0004 addendum | Mac parity, installed distribution/notice review, actual version-change compatibility, secret boundary and addendum acceptance |
| Golden Brain | Deterministic synthetic sealed-pack foundation and Windows preservation proof | Mac parity and review of fixture scope; production event schema is outside M0 |
| Doctor | Actual Windows daemon PID/IPC/lock health and absence/attention diagnostics | Mac parity; explicitly retain unimplemented canonical/Syncthing checks |
| Quality/final gate | Windows integrated workspace 89 Rust, 123 lifecycle/foundation Node + 5 storage Node, strict Clippy/build/format PASS | Mac integrated checks, final required CI at stable source, human review and final evidence-backed checklist |

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
