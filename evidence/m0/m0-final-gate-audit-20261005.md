# M0 final gate audit — historical snapshot (superseded by closeout)

> **Historical status note (2026-10-05):** Status statements below reflect the state when this dated plan/result was recorded, before M0 closeout. Current M0 status is COMPLETE for the synthetic foundation scope; scoped ADR-0012/0004 are ACCEPTED. This note does not expand this artifact's evidence, satisfy production residuals, or authorize M1. See `evidence/m0/m0-closure-audit-20261005.md`.

**Result at this audit's snapshot: candidate CI PASS; independent/human acceptance OPEN; M0 NOT COMPLETE.** Original audit below
was NOT READY at `8437559`; its namespace finding is corrected and now verified
on both OSes. Mac PASS at `7f5ab2bdaa72e213f5cb16312bca0292c436db18` records
14/14 debug + 14/14 release + 4/4 CLI, all lint/format/build/diff checks exit 0.
See `p0-2/macos-release-probe-root-evidence-20261005.md`.

Corrected-candidate implementer review rechecked the path diff, compiled-default
tests, backend-selected sidecar validation, unchanged validated pilot authority,
release-only error-recording condition and final CI job selectors. No new
implementation blocker identified within the revised narrow M0 scope. Security
residuals below remain mandatory product gates, not newly tested guarantees.
At the time of this audit, final CI and independent verifier/human diff/ADR acceptance were still open.
The mapping below preserves original audit observations and provenance.

Final CI continuation: corrected run `37377520994` at exact source `5c07411`
is **SUCCESS** (both readiness jobs plus actual Windows ACL). Windows 93 debug
Rust + 1 release resolver, Node 160/160 + 8/8; Mac 75 debug Rust + 1 release
resolver, Node 162/162 + 8/8. Initial run's missing plain benchmark build was
corrected without weakening tests. See `m0-final-ci-attempt-20261005.md`.
Candidate CI is verified; independent verifier/security review and explicit
human diff/ADR acceptance remain open. Newer documentation-only result commits
are not claimed to have status checks at that newer SHA. M0 remains IN PROGRESS.
Reviewed source: `8437559`; Windows implementer audit on 2026-10-05.
Authority: Master Spec, accepted ADR-0007/0013, M0 contract, Definition of Done.
Owner authorized preparing scoped ADR-0012/0004 revisions; both remain PROPOSED.
This is implementer review, not independent verifier or human diff acceptance.

Continuation: namespace fix `ad44e82` and release-only Clippy fix `fc79f25`
now PASS on Windows: Desktop **21/21 debug + 21/21 release**, both strict lint
profiles, format/release build and CLI **4/4**. Code now uses Cryptex-Dev for
ordinary release probes; historical path finding below is retained. Mac parity
is still pending in `p0-2/macos-release-probe-root-test-plan-20261005.md`.
No final CI, human/ADR acceptance or milestone completion follows from this
Windows-only continuation. Exact evidence and initial Clippy failure:
`p0-2/windows-release-probe-root-evidence-20261005.md`.

## Gate-by-gate mapping

| Checklist area | Audit result | Evidence / remaining action |
| --- | --- | --- |
| A: executable Desktop and daemon | Scoped foundation PASS on both OSes | Windows physical-release tray and Mac owned-supervisor evidence; both-OS native interruption confirms real Desktop/daemon paths, parents, protocol, lock and clean exit |
| A: workspace, Golden, Doctor | Recorded PASS | Windows native-interruption workspace checks; Mac corresponding checks; synthetic accepted-codec packs preserved, Doctor actual health and explicit unimplemented fields |
| A/G: clean repo and final CI | OPEN | Clean source before this audit; final cross-platform readiness and Windows cross-account ACL jobs not run on final candidate |
| B: physical event layout | PASS / ADR-0013 accepted | `p0-1/README.md` and accepted decision; no retained corpus regenerated |
| C: lifecycle | Scoped both-OS PASS | Physical close/tray, real login/removal, crash/duplicate/Stop and reconnect slices; later foreground no-console observation does not prove final console-free login |
| C: native update/error/data safety | Scoped spike PASS | Both-OS changed-package and native interruption evidence; complete pair selection, protocol rejection, baseline restore, accepted-codec synthetic inventory preserved. Mac daemon bytes unchanged; no production canonical/schema claim |
| C/H: ADR-0012 | OPEN | Concrete revised proposal needs human acceptance and required CI; no automatic acceptance |
| D: IPC | Narrow accepted spike PASS | Physical both-OS protocol/roles/framing/reconnect; historical actual Windows cross-account denial; accepted ADR-0007. Current final ACL CI still required |
| E: managed Syncthing | Scoped spike PASS | Both-OS staged 2.1.5 and actual 2.1.2 -> 2.1.5/cold restore evidence; local synthetic identities, not shipped replication |
| E: notice handling documented | M0 documentation PASS; distribution OPEN | Exact retained LICENSE/AUTHORS, upstream source/release links, staged byte/membership checks; `third_party/syncthing/2.1.5/README.md` and security review explicitly require dependency/source-access/installer notice completion before shipment |
| E/H: ADR-0004 | Proposal recorded; acceptance OPEN | Reviewed supervisor/secret/upgrade boundary in revised addendum; no shipped Node controller or OS secure-store claim |
| F: writer/master/data scope | Scoped PASS | Health-only IPC; Golden explicit synthetic benchmark codec; isolated transport fixtures; no production identity, writable memory API or master device |
| F: paths | Outside-repo PASS; development namespace OPEN | Debug/pilot/fresh transaction/Syncthing roots use Cryptex-Dev, but ordinary release Desktop still defaults to Cryptex product runtime on both OSes |
| F/G: security | Implementer boundary review performed; NOT COMPLETE | Arbitrary renderer executable finding fixed and both-OS regressions passed. Namespace finding below unresolved; no independent/human review claim |
| F/G: failure/quality | Retained relevant checks PASS; integrated final CI OPEN | Windows latest Desktop 20/20, Node 160/160 + 8/8; Mac latest guard 13/13 + 4/4; earlier workspace/native/process/I/O/kill suites retained at their exact sources |
| H/final: closure | OPEN | Resolve path finding, final CI, verifier/human review, ADR acceptance and final checklist/index/handoff consistency |

## Concrete remaining implementation finding: release probe namespace

`apps/desktop/src-tauri/src/main.rs` chooses `default_runtime_root_path()` when
no validated pilot root is present. Debug uses Cryptex-Dev; ordinary release uses
`mac_product_runtime_root` / `windows_product_runtime_root`, selecting
`Cryptex/runtime/p0-2`. Historical physical release evidence confirms this path.

These were empty synthetic probe runtimes outside Git, not personal canonical
data. Nevertheless, an M0 release build is still a development probe; successful
tests do not satisfy the M0 development-root checkbox for this default.
Do not reclassify those paths as Cryptex-Dev or erase historical evidence.

Required correction before final CI: use Cryptex-Dev for ordinary M0 release
probes on both OSes; retain validated pilot-root authority and startup controls;
add explicit debug/release path regressions and run relevant Windows/Mac checks.
Do not move/delete old runtime data, change an existing login registration, or
open a visible application as part of the audit. Production path selection can
be introduced under its authorized product milestone; the Master Spec product
path itself must not be changed.

## Security and recovery residual ledger

- Sidecar request validation now occurs before runtime creation or spawning and
  launches the backend-selected path. Windows ordinary/hard-link and Mac real
  symlink validator tests passed. This is not physical malicious-renderer proof
  or publisher authentication; same-user on-disk executable replacement remains
  outside this validator's claim.
- No Tauri shell/filesystem plugin is installed. The repository has no explicit
  capabilities directory; custom probe commands are registered in Rust. This
  absence is not proof of a reviewed production capability manifest. Source
  review of the narrow command set and CSP/text rendering is the M0 evidence.
- Accepted IPC ACL/mode and framing/correlation proofs are retained. Whole-read
  timeout/stalled same-user peer resilience remains unimplemented and must be
  resolved before hardened production APIs; successful health calls do not
  establish a latency guarantee.
- Fixture Syncthing keys remain native private test files, not production device
  identities or OS secure-store acceptance. Windows inherited profile ACLs were
  not independently verified. Production secret/permission gates remain required.
- Corrupt/ambiguous owner, runtime, claim or slot evidence refuses recovery.
  Interrupted recovery claims remain inspection-only. One native updater kill
  boundary and injected I/O failures are not machine power-loss proof.
- Only synthetic sealed-pack/manifest data was preserved; production event
  validation, signatures and live canonical rollback remain later product gates.

## Retained evidence spot checks

The Windows retained `native-recovery-report.json` and changed-package
`native-selection-report.json` both still exist outside Git. Read-only SHA-256
checks match their evidence digests respectively:

- `712b9f2f6ac97b44e3360ff343457e6e6098d1377214a066ae58565b68d204b2`
- `2d6e19d8e6e2519a7568347824cc28f20813b11df6688d18365d88152d62cdb5`

Mac records are owner-supplied reports with explicit source/environment/artifact
provenance; Mac artifacts were not opened from Windows. Existing PASS reports
are not rerun or expanded in scope. No full retained-corpus rehash was performed.

## Next steps

1. Correct and verify the M0 release namespace finding, including narrow Mac parity.
2. Finish scoped verifier/security review on the corrected candidate; then run
   final required readiness (Windows/macOS) and Windows ACL CI once.
3. Present concrete diff and ADR decisions for human acceptance. Owner approval
   to prepare proposals is not approval to mark them ACCEPTED.
4. Update remaining checklist/ADR index/handoff and milestone completion only
   when every applicable gate is actually verified. M1 stays unauthorized.

This audit changed documentation only. No tests, GUI, startup/config changes,
runtime mutation, process termination, Actions or production installation.
