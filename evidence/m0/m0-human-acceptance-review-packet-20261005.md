# M0 human acceptance review packet

**Status: PREPARED — NOT APPROVED.** Candidate implementation/docs source
`e337bd38678875daed088ce32c3e83734bc966c7`. M0 remains IN PROGRESS.
This packet itself changes no acceptance status or milestone authority.

CI continuation: first run failed because the workflow omitted building the
plain storage benchmark needed by Golden tests. Workflow-only correction builds
both required native helpers; local focused tests 6/6 PASS. See
`m0-final-ci-attempt-20261005.md`; corrected-source cloud result remains required.
The candidate above identifies the reviewed implementation; the newer workflow
correction does not change application/runtime behavior or weaken any test.

## Reviewable decisions

1. `docs/adr/ADR-0012-background-runtime-implementation-proposal.md`: Mode B
   Desktop ownership with per-user Desktop login startup, hide-on-close,
   native Open/Quit, bounded daemon restart and backend-selected sidecar.
   Native complete-pair selection, actual protocol rejection/rollback and one
   pre-successor updater-kill boundary preserve accepted-codec synthetic packs.
   Accepting this direction would not accept an installed/signed production
   updater or production canonical schema safety.
2. `docs/adr/ADR-0004-managed-syncthing-implementation-addendum.md`: pinned
   unmodified target-matched 2.1.5 native package and proposed backend-owned
   authenticated loopback REST supervision. Test-only staged lifecycle and
   actual 2.1.2 -> 2.1.5/cold restore are verified; the Node controller is not
   shipped and does not implement production replication semantics.

Existing accepted ADR-0007 and ADR-0013 retain their scopes. Missing original
source records are not reconstructed. Owner approval to prepare revised
proposals is recorded separately; it is not final ADR/human diff acceptance.

## Focused recent code and evidence

- `82c4d32`: renderer may request only the backend-selected ordinary absolute
  daemon path; backend-selected path is actually launched. Display field is
  read-only. Both-OS validator and CLI evidence, not physical malicious renderer
  invocation or publisher verification.
- `ad44e82`: ordinary M0 release probe defaults to hidden Cryptex-Dev app data,
  with unchanged debug comparison and validated pilot authority.
- `fc79f25`: same release startup-error recording condition/body expressed as
  a let-chain to pass strict Clippy; no lint suppression or weakened test.
- Both-OS namespace evidence: Windows 21/21 debug + 21/21 release + 4/4 CLI;
  Mac 14/14 debug + 14/14 release + 4/4 CLI. All required focused checks exit 0.
- `evidence/m0/m0-final-gate-audit-20261005.md` maps every gate, preserves
  historical failures and records matching retained Windows report digests.
- `evidence/m0/m0-security-notice-acceptance-review-20261005.md` records scoped
  implementer review and the complete known residual ledger.
- `.github/workflows/ci.yml` covers both-OS readiness, real Windows cross-account
  ACL denial, release resolver/lint and all foundation Node tests. Final run:
  https://github.com/gungortarik/cryptex/actions/runs/37376952961
  Its outcome must be observed and recorded, not assumed from this link.

## Required residuals to retain on acceptance

Production identity/OS secure storage, generated-private-file permissions,
real signed event schema/canonical recovery, installed updater migration/crash/
power-loss boundaries, console-free final package login, signed/notarized release,
publisher verification and full delivered dependency/source-access/installer
notices remain mandatory product gates. The synchronous IPC whole-read/stalled
peer limit must be addressed before hardened production APIs. No real personal
or Cortex data or new public endpoint is authorized by M0 acceptance.

## Conditions before closing M0

- Required CI jobs green for the recorded stable candidate; any newer behavior
  change needs applicable fresh verification.
- Independent verifier checks actual gate/evidence/spec state and security
  auditor reviews findings; implementer self-review is not a substitute.
- Human reviews the concrete diff/decisions and explicitly accepts ADR-0012 and
  ADR-0004 within their revised M0 scopes, with residual product gates retained.
- Reconcile final checklist, ADR index, milestone and handoff. Do not check
  unexecuted proof, silently waive an authoritative invariant or infer M1 work
  authorization from an unaccepted proposal.

Only after those conditions may an evidence-backed M0 completion record be
prepared. No merge, rebase, main-branch update or distribution is authorized by
this packet. No new Mac physical test is required merely to review it.
