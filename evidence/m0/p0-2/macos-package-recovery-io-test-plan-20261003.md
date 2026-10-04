# Mac task — package recovery I/O parity

Read the current handoff and ADR-0012 candidate proposal. Use the clean current
`m0/p0-2-cursor` branch containing this plan, and report its full source SHA.
Preserve local changes; do not merge/rebase or switch branches without approval.
Use pinned Node v24.19.0. No GUI, installer, Rust rebuild, physical login/tray,
real-daemon health repeat, or GitHub Actions is required.

Run from repository root:

```sh
node --version
node --test tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs tools/m0-lifecycle-compare/scripts/package-daemon-health.test.mjs
node --check tools/m0-lifecycle-compare/scripts/package-transaction.mjs
node --check tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs
git diff --check
```

Expect **41/41**: 37 transaction + 4 health adapter tests, zero failures/skips.
Four new recovery cases inject writeFileSync/ENOSPC, fsyncSync/EIO,
renameSync/EACCES and unlinkSync/EACCES. Verify their journal/slot preservation,
typed diagnostics and idempotent retry tests pass. Two new automatic rollback
cases must reject restoration claims after failed rollback for rejected and
throwing health. Existing concurrency and five SIGKILL tests must also pass.

Return OS/architecture, Node version, full source SHA, totals, exit status,
syntax/diff results and any temporary-root cleanup error. Do not modify files
or commit/push; Windows implementer will record the returned host evidence.
Faults are injected; do not claim actual disk-full, ACL or power-loss proof.
ADR-0012 remains PROPOSED and native updater integration stays open.
