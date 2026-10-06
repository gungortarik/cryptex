# Mac task — concurrent recovery claim parity

> **Historical status note (2026-10-05):** Status statements below reflect the state when this dated plan/result was recorded, before M0 closeout. Current M0 status is COMPLETE for the synthetic foundation scope; scoped ADR-0012/0004 are ACCEPTED. This note does not expand this artifact's evidence, satisfy production residuals, or authorize M1. See `evidence/m0/m0-closure-audit-20261005.md`.

**Result:** scoped PASS at `0ce9c93f621dceedd2edb78d736eb526f760a817`,
55/55 tests. Evidence: `macos-package-recovery-claim-evidence-20261004.md`.

Read current handoff and ADR-0012 candidate proposal. Use the clean current
`m0/p0-2-cursor` source containing this plan; report its full commit SHA.
Preserve local changes and branch strategy. Use pinned Node v24.19.0.
No GUI, installer, Rust build, real-daemon lifecycle repeat or Actions required.

Run from repository root:

```sh
node --version
node --test tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs tools/m0-lifecycle-compare/scripts/package-daemon-health.test.mjs
node --check tools/m0-lifecycle-compare/scripts/package-transaction.mjs
node --check tools/m0-lifecycle-compare/scripts/package-transaction-process.mjs
node --check tools/m0-lifecycle-compare/scripts/package-transaction-contender.mjs
node --check tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs
git diff --check
```

Expect **55/55** (51 transaction + 4 health adapter), zero failures/skips.
Confirm all three new cases pass:

- `separate recovery claim refuses concurrent recovery and updater`
- `recovery claim blocks a prevalidated updater after old intent removal`
- `interrupted recovery retains claim and refuses automatic reclamation`

Normal completion must release claim and allow successor commit; old recovery
must preserve a newer updater intent. Interrupted recovery must refuse retries
and retain claim/journal bytes. Do not delete the claim to force success.
Prior I/O, owner, process-race and five updater SIGKILL tests must also pass.

Return OS/architecture, Node version, source SHA, totals/exit status, four syntax
and diff results, plus fixture/owned-child cleanup errors if any. Do not edit
files or commit/push; Windows implementer records the supplied results.
The retained interrupted claim exists only until owned fixture cleanup after
all subprocesses exit. Never remove a retained operator experiment's claim.
Native process locks, automatic interrupted-recovery repair, broad stress and
production acceptance remain open. ADR-0012 stays PROPOSED.
