# Mac task — live-updater recovery owner guard

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

Expect **52/52** (48 transaction + 4 health adapter), no failures/skips.
Confirm all three `separate recovery refuses live updater` tests pass, including
unchanged journal/selection bytes and baseline recovery after owned process exit.
Confirm the refused recovery permits the live updater's later commit, legacy
ownerless and malformed owner tests fail closed, and changed owner token blocks
automatic rollback. Prior process races, I/O faults and five SIGKILL tests pass.
The two prior automatic rollback-I/O tests now expect external recovery refusal
while their owner PID is alive; do not weaken this expectation.

Return OS/architecture, Node version, source SHA, totals/exit status, syntax/diff
results and any fixture/owned-child cleanup errors. Do not edit files or
commit/push; Windows implementer records the supplied host results.
Do not rewrite retained schema-1 intents or test real personal data. PID reuse
and ambiguous liveness conservatively block recovery. Simultaneous recovery
callers, native process locks and production acceptance remain open.
