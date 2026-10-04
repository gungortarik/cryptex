# macOS serialized package transaction candidate evidence

**Date:** 2026-10-03 (America/Toronto)
**Latest result:** scoped PASS at `fc31489`, 29/29 with the original default
TMPDIR and no override. Earlier results and their path prerequisite are
retained below as historical evidence.
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

## Follow-up: default temporary-path regression — PASS

Source: `fc31489fc619b9b4b12f93cd350b215efa7f3c7c`, clean checkout.
Host: macOS `27.0.1`, `arm64`; Node `v24.19.0` (pinned, unchanged).
The fixture allocator resolves only the OS temporary directory before creating
owned roots; engine link/junction rejection remains unchanged. Cleanup is
registered before initialization. No source repair was made during this run.

Original environment before and after execution:

```text
TMPDIR=/var/folders/x6/4s8wgb5s1sv3btr0kdzhcnnr0000gn/T/
os.tmpdir()=/var/folders/x6/4s8wgb5s1sv3btr0kdzhcnnr0000gn/T
realpath=/private/var/folders/x6/4s8wgb5s1sv3btr0kdzhcnnr0000gn/T
ownedRootsBefore=[]
ownedRootsAfter=[]
```

Ran the ordinary command without changing or unsetting TMPDIR:

```sh
node --test tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs
```

Exit 0. Exact test output:

```text
✔ temporary directory alias resolved before allocation; engine still rejects alias (59.564375ms)
✔ compatible pair commit retains complete previous pair (66.367667ms)
✔ declared incompatible pair rejected before any staging (27.660166ms)
✔ payload digest mismatch rejected before staging (26.796084ms)
✔ non-true health (false) automatically restores old pair (70.46825ms)
✔ non-true health (undefined) automatically restores old pair (81.703459ms)
✔ non-true health (true) automatically restores old pair (83.002208ms)
✔ health exception restores old pair and exposes typed failure (77.989625ms)
✔ health-time payload tampering restores baseline (83.552375ms)
✔ health callback is mandatory (29.682083ms)
✔ path traversal and unsupported schema rejected (30.602291ms)
✔ existing experiment never overwritten (30.92825ms)
✔ existing slot never reused, even if staging was partial (29.906625ms)
✔ malformed selection is an explicit error (31.1835ms)
✔ malformed journal retained for diagnosis (28.729209ms)
✔ foreign journal relationship refused without changing selection (38.871125ms)
✔ source symlink or junction rejected (30.051708ms)
✔ repository experiment path rejected before creation (0.367333ms)
✔ actual updater-process kill at staged: recover complete pair (97.408125ms)
✔ actual updater-process kill at intent: recover complete pair (94.781ms)
✔ actual updater-process kill at selected: recover complete pair (100.53225ms)
✔ actual updater-process kill at healthy: recover complete pair (98.113666ms)
✔ actual updater-process kill at committed: recover complete pair (99.692459ms)
✔ corrupt baseline during pending recovery fails closed and keeps journal (81.35725ms)
✔ successive failed update restores most recent good pair (99.119ms)
✔ unknown manifest protocol and missing component rejected (28.734708ms)
✔ incomplete active slot cannot be selected (27.30925ms)
✔ root ownership marker cannot be silently adopted (28.136958ms)
✔ new transition refuses unresolved intent (117.237541ms)
ℹ tests 29
ℹ suites 0
ℹ pass 29
ℹ fail 0
ℹ cancelled 0
ℹ skipped 0
ℹ todo 0
ℹ duration_ms 1746.31
```

All six syntax checks exited 0 with no output: `node --check` on
`package-transaction-fixtures.mjs`, `package-transaction.test.mjs`,
`package-transaction.mjs`, `package-transaction-crash.mjs`,
`package-transaction-worker.mjs`, `run-package-transaction-proof.mjs`
under `tools/m0-lifecycle-compare/scripts/`. `git diff --check` passed.

The five owned-worker SIGKILL boundaries, old-pair recovery before commit,
candidate selection after commit, idempotence, corrupt-baseline refusal and
unchanged fixture inventories passed again. POSIX directory fsync completed
without an error. No owned fixture roots remained. The default-path repair is
now verified on Mac; an external real-path override is no longer required.
The prior failure and overridden 28-test PASS remain historical, not erased.

No optional binary staging, GUI/login/tray/Stop test, installed-package change,
dependency change, rebuild or GitHub Actions ran. Production gates and
ADR-0012 PROPOSED status remain unchanged. This is process-interruption and
synthetic-fixture evidence, not power-loss or canonical-data acceptance.
