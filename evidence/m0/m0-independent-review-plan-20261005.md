# M0 independent verifier and security review plan

Use clean branch `m0/p0-2-cursor` containing `5c07411`; report full SHA.
This is a read-only review task for an executor independent of the implementer.
It may be performed on either OS; no repeated physical Mac test is requested.

Read AGENTS, `docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md`, Master Spec, M0
contract, Definition of Done, accepted ADR-0007/0013 and revised proposed
ADR-0012/0004. Apply `.cursor/agents/verifier.md` and
`.cursor/agents/security-auditor.md` as separate review responsibilities.

1. Verify the actual M0 gate matrix against repository source/tests/retained
   evidence; distinguish local Windows artifacts from owner-supplied Mac reports.
   Validate narrowed fixture claims, not just PASS headings or checked boxes.
2. Review canonical writer/master/offline boundaries, private runtime roots,
   renderer executable authority/CSP/text rendering, IPC ACL/mode/protocol/frame
   limits, native ownership/gates/recovery refusal and Syncthing isolation/auth.
3. Inspect recent sidecar, namespace and release-only lint changes and their tests.
   Confirm no test weakening, old runtime migration or production product feature.
4. Review notice/provenance and upgrade/secret boundaries. Flag required M0
   omissions separately from mandatory product residuals. Owner authorized
   preparing scope revisions, not violating an accepted invariant or accepting ADRs.
5. Read final CI results for the exact candidate if available. First run
   `37376952961` had a missing benchmark binary and was cancelled; corrected run
   `37377520994` must independently pass. Do not infer success before completion.
6. Report any authority conflict or finding with severity, file/line, evidence
   and necessary remediation; do not silently fix code or mark ADRs ACCEPTED.

Retained review inputs:

- `evidence/m0/m0-final-gate-audit-20261005.md`
- `evidence/m0/m0-security-notice-acceptance-review-20261005.md`
- `evidence/m0/m0-human-acceptance-review-packet-20261005.md`
- `evidence/m0/m0-final-ci-attempt-20261005.md`
- Latest both-OS sidecar/root, changed-package/interruption and Syncthing
  managed/upgrade evidence referenced in the gate checklist.

No file edits/commit/push, workflow dispatch, installation, startup/global
configuration mutation, fixture/corpus regeneration or app/daemon launch.
No unrequested full test rerun; if evidence cannot establish a claim, report
NOT VERIFIED with the exact missing proof. Return scoped VERIFIED / FAILED /
NOT VERIFIED / OUT OF SCOPE matrix and prioritized security findings.
Do not describe historical failure as a current verified result or claim broad
production security/distribution acceptance. Final human diff/ADR approval and
M0 completion remain outside this review task.
