# Mac task — separate-process package race parity

**Result:** scoped PASS at `6b8776c479efcb3091985a7c90f7987ba74d2e2e`,
45/45 tests. Evidence: `macos-package-process-race-evidence-20261004.md`.

Read the current handoff and ADR-0012 candidate proposal. Use the clean current
`m0/p0-2-cursor` source containing this plan and report the full commit SHA.
Preserve local changes; do not merge/rebase or switch branches without approval.
Use pinned Node v24.19.0. No GUI, installer, Rust build, real-daemon lifecycle
repeat, login/tray test or GitHub Actions is required.

Run from repository root:

```sh
node --version
node --test tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs tools/m0-lifecycle-compare/scripts/package-daemon-health.test.mjs
node --check tools/m0-lifecycle-compare/scripts/package-transaction-process.mjs
node --check tools/m0-lifecycle-compare/scripts/package-transaction-contender.mjs
node --check tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs
git diff --check
```

Expected: **45/45**, comprising 41 transaction + 4 health adapter tests, no
failures/skips. All four `separate updater processes:` cases must pass. They
use distinct owned subprocesses with bounded IPC checkpoints, checking stale
commit refusal, pending-intent preservation and one simultaneous winner.
Prior recovery faults, concurrency regressions and five SIGKILL cases must pass.

Return OS/architecture, Node version, full source SHA, totals/exit status,
syntax/diff results and any fixture or owned-child cleanup error. Do not edit
files or commit/push; Windows implementer records the supplied Mac results.
This establishes four controlled process races only. Broad stress, recovery
competing with an active updater, native process locks, installed handover,
signing and canonical-data acceptance remain open; ADR-0012 is PROPOSED.
