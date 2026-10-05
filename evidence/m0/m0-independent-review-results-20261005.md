# M0 independent verifier and security audit

Reviewed source: `3de9d000f131cecf5d816f8dd16937eb101fcee8`.
Owner authorized both independent read-only reviewers with "hemen basla" after
the explicit two-reviewer proposal. Separate verifier and security agents applied
their repository role instructions. Neither edited files, ran tests/workflows,
launched processes or changed runtime/startup/global configuration.

## Verifier outcome

No new M0 implementation defect or accepted-authority conflict identified in
reviewed slices. Ready for scoped human review after documentation consistency
and relevant UI review; **M0 is not COMPLETE**.

| Area | Result and limit |
| --- | --- |
| Clean source / candidate correspondence | VERIFIED: seven Markdown differences from CI source, no behavior/workflow delta |
| Candidate CI | VERIFIED independently via GitHub: run `37377520994`, exact `5c074115cd61ef7c1eb47e8ed2cf8c1105360679`, both readiness and actual Windows cross-account ACL SUCCESS; installer intentionally skipped |
| Build/test inventory | VERIFIED: relevant workflow suites, release resolver/lint, frontend/native builds; original failed attempt preserved |
| Backend sidecar / development roots | VERIFIED: pre-spawn ordinary-path equality and negative tests; actual compile-selected Cryptex-Dev resolver tests |
| IPC | VERIFIED within accepted health-only scope: frame/protocol/role checks, private Unix parent and protected Windows DACL |
| P0-1 | VERIFIED from retained accepted ADR-0013/measurement matrix; controlled large-file timeout not relabeled success |
| Lifecycle / native recovery | VERIFIED within retained both-host slices: conservative owner/claim refusal, complete slots, pre-successor interruption only |
| Golden / Doctor | VERIFIED synthetic codec/health foundation; recomputed retained Windows recovery report SHA matches; unimplemented checks stay explicit |
| Managed Syncthing / notices | VERIFIED retained source/evidence for authenticated isolated pinned spike and exact inventory; not delivered product acceptance |
| Mac-local raw report recomputation | NOT VERIFIED directly on Windows; exact owner-reported provenance remains distinct from direct artifact inspection |
| Explicit foundation UI-quality record | NOT VERIFIED at reviewed source; retained physical behavior does not alone record a visual quality verdict |
| Completion-commit CI / human ADR approval | NOT VERIFIED; candidate success is not a newer completion SHA check or human acceptance |
| Production identity, signed events, installed updater, power loss, signing/distribution | OUT OF SCOPE, mandatory later product gates retained |

P2 documentation finding: checklist introduction/CI bullets, human packet
introduction and older handoff continuations contain stale present-tense pending
instructions. Reconcile candidate PASS vs historical failures vs still-open
completion/human gates without repeating workflows.

P2 UI finding: DoD sections 9/11 and completion workflow require an explicit
scoped foundation UI review. Record visual evidence/applicability; do not claim
new native/theme/DPI proof from code review alone or expand into broad product UI.

## Independent security outcome

**VERIFIED for the narrow synthetic M0 boundaries reviewed.** No new blocking
M0 security defect or accepted-invariant conflict established. This is not
production certification, ADR acceptance or milestone completion.

Reviewer inspected canonical writer/master/offline scope, renderer text/CSP/
capabilities and backend-selected launch, IPC ACL/mode/protocol/frame checks,
native locks/owner/claim fail-closed recovery, authenticated isolated Syncthing
and notices. Exact notice fingerprints and final CI were independently checked.
Mac raw artifacts were not directly inspected from Windows.

| Priority | Affected source | Finding and required follow-up |
| --- | --- | --- |
| P2 | `services/cryptexd/src/ipc.rs:48`, `:108`, `:259` | Serial health worker and synchronous read lack whole-read deadline; permitted stalled peer can occupy worker/client. Add read/write deadlines and connection/resource bounds with tests before hardened production APIs. Connect budget and byte cap do not bound total latency. |
| P2 | `tools/m0-lifecycle-compare/scripts/syncthing-pilot.mjs:81`, `:90` | Fixture API key in native config; POSIX home 0700, Windows inheritance not independently established. M1/M8 must establish OS secure storage/private-file permissions; never adopt fixture identity. |
| P2 | `apps/desktop/src-tauri/src/main.rs:781`, `:790`, `:798` | Non-pilot developer `start.request` may specify executable without renderer validator. Existing filesystem-authorized test control, not demonstrated renderer escape. Remove/gate or constrain it before shipped product; renderer fix is not removal of every file-controlled launch. |
| P2 | `tools/m0-lifecycle-compare/scripts/syncthing-bundle.mjs:27`, `:51` | Binary/LICENSE/AUTHORS inventory does not prove publisher authenticity, delivered source/dependency notices or installer inclusion. Complete distribution/signing gates before shipment. |
| P3 | `tools/m0-lifecycle-compare/scripts/syncthing-pilot.mjs:118` | REST text buffered before size check. Stream bounded bytes if reused in product supervision. |

These are explicitly retained production residuals, not presently blocking the
synthetic M0 security verdict. Do not reuse developer controls or fixture secrets
as product authorization boundaries. Human diff/ADR review, relevant UI review
and final completion-source reconciliation remain open.

## Subsequent documentation and Windows UI review

Both independent reviewers checked their recorded results/proposed residual
additions and found no broadened security or milestone acceptance claim.
Verifier requested clearer marker/request semantics in the Mac UI plan and
historical CI wording; these documentation-only corrections were applied.

After the owner authorized native Windows review/placement, verifier independently
opened its three retained screenshots, recomputed all four artifact hashes,
read health/identity/cleanup report and checked absent recorded processes.
**Windows native foundation visual review scoped PASS**; equivalent Mac review
remains NOT VERIFIED. Details/provenance: `m0-foundation-ui-review-20261005.md`.
The report has no embedded source-SHA or native-gate-release field; source/build
and gate-release claims retain their separate run transcript provenance.

## Embedded frontend build correction follow-up

At `65ce7b91fe500cfe751aa3d13b39010f6dcc935e`, focused verifier review PASS;
security review of the same behavior delta also PASS. The typed release-mode
guard runs after pilot-root validation and before Tauri context/runtime creation.
Debug development mode remains supported; no CSP, capability or secret boundary
was relaxed. A release-only assertion verifies embedded frontend mode, closing
the risk of a release test suite silently passing without `tauri/custom-protocol`.
CI now explicitly builds, fully tests and lints that supported release mode.

These read-only reviews did not run tests, apps or workflows. Windows focused
execution is recorded separately in
`windows-embedded-frontend-mode-evidence-20261005.md`. Mac operator review
subsequently established an actual blank-window FAIL; the older NOT VERIFIED
attempt above is historical. Fresh Mac package/visual proof and new-source final
CI are required; the proposed Mac cause/remediation is not yet verified there.
