# Mac task — focused package filesystem candidate parity

> **Historical status note (2026-10-05):** Status statements below reflect the state when this dated plan/result was recorded, before M0 closeout. Current M0 status is COMPLETE for the synthetic foundation scope; scoped ADR-0012/0004 are ACCEPTED. This note does not expand this artifact's evidence, satisfy production residuals, or authorize M1. See `evidence/m0/m0-closure-audit-20261005.md`.

## Follow-up after `12f76ac`: default temporary-path regression

**Follow-up status:** PASS at `fc31489`, 29/29 tests, zero failed/skipped,
original default TMPDIR with no override; six syntax checks passed and no
owned fixture roots remained. Exact output is appended to the Mac evidence.

The fixture allocator now resolves the OS temporary directory before allocation;
engine link/junction rejection is unchanged. On the commit containing
`windows-package-fixture-path-fix-20261003.md`, rerun the ordinary command below
with the host's original default TMPDIR, **without** the real-path override.
Record both `os.tmpdir()` and its real path, exact source SHA and test output.
Expect **29 passed, zero failed/skipped**, including the new temporary-alias
regression and the five actual SIGKILL boundaries.

Also syntax-check `package-transaction-fixtures.mjs` and
`package-transaction.test.mjs` along with the four files in the original plan.
Confirm no owned fixture roots were left by this run. Add a follow-up to the Mac
evidence and handoff, preserving the prior default-path failure and overridden
28-test PASS. Commit/push. Do not repeat optional binary staging, GUI/lifecycle
tests or Actions for this fixture-only correction.

The original procedure below describes the 28-test source and is retained as
historical context. Follow-up Mac default-path verification is complete.

**Historical status:** scoped PASS at `780c047` with canonical process-local `TMPDIR`;
default `/var` temporary path fails closed (`UNSAFE_PATH`). See
`macos-package-transaction-candidate-evidence-20261003.md` for both runs.
**Scope:** M0 P0-2 test-only filesystem/process-interruption candidate.

Mac Codex: read current handoff, accepted update spec and
`docs/adr/ADR-0012-update-orchestration-candidate.md` first. Use the commit
containing this plan and record its exact source SHA. Follow repository branch
rules; do not merge/rebase or accept ADR-0012 without authorization.

## Required run

1. Confirm the pinned Node from `.node-version` (`24.19.0`). Use the existing
   toolchain; do not upgrade dependencies. Record macOS version and source SHA.
2. From the repository run:

   ```sh
   node --test tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs
   node --check tools/m0-lifecycle-compare/scripts/package-transaction.mjs
   node --check tools/m0-lifecycle-compare/scripts/package-transaction-crash.mjs
   node --check tools/m0-lifecycle-compare/scripts/package-transaction-worker.mjs
   node --check tools/m0-lifecycle-compare/scripts/run-package-transaction-proof.mjs
   git diff --check
   ```

3. Expect **28 passed, zero failed/skipped**. Confirm all five actual owned-worker
   SIGKILL boundaries ran, old-pair recovery before commit, candidate selection
   after commit, idempotence, corrupt-baseline refusal and unchanged fixture
   inventories. Do not replace any assertion with a skip to obtain a PASS.
4. POSIX directory fsync must succeed. Report the exact error if it fails; do not
   swallow it or claim power-loss durability from process-kill evidence.
5. Record results in a new
   `evidence/m0/p0-2/macos-package-transaction-candidate-evidence-20261003.md`.
   Update evidence README and the current handoff's candidate continuation.
   Preserve Windows evidence and remaining gates. Commit/push these documents;
   report SHA, exact test count and any limitations. If code repairs are needed,
   keep them scoped, add regressions and describe them before claiming parity.

## Optional existing-binary-copy run

Only if two retained Desktop/daemon pairs are already available, run
`run-package-transaction-proof.mjs` with absolute paths for `--root`,
`--baseline-desktop`, `--baseline-daemon`, `--candidate-desktop`,
`--candidate-daemon`. Use a fresh root under
`~/Library/Application Support/Cryptex-Dev/p0-2/`; retain the resulting report.
Use the executable bytes inside a retained `.app` only as file-copy input; the
runner does not copy or validate the whole app bundle. Record source paths and
hashes, and whether both pairs are identical. Expect 8 scenarios and unchanged
two-file synthetic inventory. Do not rebuild solely for this optional run or
claim package-resource/signing verification.

## Do not repeat or expand

No GitHub Actions, physical login/tray/Stop tests, GUI or installed-package change
is needed. Do not touch retained P0-1 corpora, personal memory, product runtime,
login registration or installed Desktop. These tests use new temporary fixtures
and only kill subprocesses they spawned. Temporary unit-test roots are cleaned;
optional proof-run roots are retained.

PASS closes only Mac parity for this serialized candidate. Actual package helper,
runtime compatibility, publisher trust, concurrent transitions, machine power
loss and production canonical-data acceptance remain open. Keep ADR-0012 PROPOSED.
