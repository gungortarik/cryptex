# macOS serialized package transaction candidate evidence

**Date:** 2026-10-03 (America/Toronto)
**Result:** scoped PASS with an explicit temporary-path prerequisite.
**Source:** `780c0473bdb85b52abc994661f10671cc85e4992`
**Branch:** `m0/p0-2-cursor`, clean at execution start.
**Host:** macOS `27.0.1`, `arm64`; existing Node `v24.19.0`, matching `.node-version`.
**Plan:** `macos-package-transaction-candidate-test-plan-20261003.md`.

Read the current handoff, accepted `docs/26_VERSIONING_MIGRATIONS_AND_UPDATES.md`
and proposed `docs/adr/ADR-0012-update-orchestration-candidate.md`. No production
architecture was selected and ADR-0012 remains PROPOSED.

## Default-path failure retained

The exact unprefixed plan command:

```sh
node --test tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs
```

exited 1: 28 tests, 1 passed, 27 failed, 0 cancelled/skipped/todo.
Failures occurred in fixture initialization, before crash checkpoints:

```text
Error [PackageError]: Expected ordinary file/directory
    at plain (.../package-transaction.mjs:21:83)
    at safeAncestors (.../package-transaction.mjs:26:32)
    at outsideRepository (.../package-transaction.mjs:35:3)
    at initialize (.../package-transaction.mjs:129:3)
    at setup (.../package-transaction.test.mjs:24:3)
code: 'UNSAFE_PATH'
```

`os.tmpdir()` returned `/var/folders/x6/4s8wgb5s1sv3btr0kdzhcnnr0000gn/T`;
`fs.realpathSync` returned the same suffix under `/private/var`. `lstat('/var')`
confirmed a symlink. The candidate deliberately refuses symlink ancestors.
The default-path invocation therefore remains unsupported on this host; this
is not an fsync failure and is not hidden as a passing default invocation.

## Canonical temporary-path run — PASS

Used only a process-local environment override to select the existing real
temporary directory. No source change, symlink-check weakening or skipped
assertion was made:

```sh
TMPDIR=/private/var/folders/x6/4s8wgb5s1sv3btr0kdzhcnnr0000gn/T node --test tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs
```

Exit 0, duration `1704.586292 ms`:

```text
tests 28
suites 0
pass 28
fail 0
cancelled 0
skipped 0
todo 0
```

All five actual owned-worker SIGKILL boundaries passed: `staged`, `intent`,
`selected`, `healthy`, `committed`. The helper asserts the observed checkpoint,
positive worker PID and returned `SIGKILL` signal. Recovery selected baseline
before commit and candidate after commit; repeated recovery was idempotent.
Corrupt baseline recovery refused with `PAYLOAD_MISMATCH` and retained the
journal. All fixture teardown inventory assertions passed, comparing file
membership and SHA-256 bytes of the synthetic data fixtures.

On macOS, directory flushes execute `openSync`, `fsyncSync`, `closeSync` without
a swallowed fsync error. Initialization, staging, selection, commit and recovery
completed through those paths. This proves successful POSIX directory fsync
calls in this run, not machine-power-loss durability.

## Other required checks and cleanup

Each of the following syntax checks exited 0 with no output:

```sh
node --check tools/m0-lifecycle-compare/scripts/package-transaction.mjs
node --check tools/m0-lifecycle-compare/scripts/package-transaction-crash.mjs
node --check tools/m0-lifecycle-compare/scripts/package-transaction-worker.mjs
node --check tools/m0-lifecycle-compare/scripts/run-package-transaction-proof.mjs
```

`git diff --check` passed before and after documentation changes.
The successful suite removed its temporary roots. The failed initial setup
left 27 roots because initialization failed before teardown registration.
Their creation times matched the failed run at `2026-10-04T03:07:17Z`;
exact membership and all four synthetic file contents were verified before
explicit file/directory cleanup. No matching fixture roots remained afterward.
No broad or recursive cleanup was used for those leftover roots.

The optional retained-binary-copy experiment was not run: two retained Mac
Desktop/daemon pairs were not established for this task; no rebuild was made.
No installed application, product runtime, login registration, P0-1 corpus or
personal memory was touched. No GUI/physical lifecycle test or GitHub Actions
ran. Only evidence, plan status, evidence index and handoff were changed.

This closes Mac parity for the serialized filesystem/process-interruption
candidate when given a non-symlink temporary path. Native package helper,
actual runtime compatibility/IPC readiness, publisher trust/signing/resources,
concurrent transitions, machine power loss and production canonical-data
acceptance remain open. Synthetic inventory safety is not canonical-data proof.
