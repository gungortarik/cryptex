# M0 final CI attempts — 2026-10-05

## Attempt 1 — failed setup for Golden Node test

Run: https://github.com/gungortarik/cryptex/actions/runs/37376952961
Source: `e337bd38678875daed088ce32c3e83734bc966c7`.

macOS readiness failed in Node tests:
`Golden foundation creates deterministic sealed synthetic packs` at
`tools/m0-lifecycle-compare/scripts/m0-foundation.test.mjs:10`.
Actual error: spawn `target/debug/m0-storage-bench` ENOENT; Node step exit 1.
Cargo's benchmark test executable existed, but the workflow built only the
plain lifecycle helper, not the plain storage benchmark required by this test.
Local retained build artifacts had masked that workflow prerequisite omission.

Mac preceding format/strict workspace lint, storage/daemon/helper/IPC/Desktop
Rust checks and release build/resolver/strict lint steps passed. These do not
make the overall readiness job PASS. Windows actual cross-account ACL job passed.
Windows readiness was cancelled while compiling workspace lint to avoid paying
for the same known missing-binary prerequisite later; it is not counted PASS.
The package-only installer job was intentionally skipped.

## Correction and validation

Workflow now explicitly builds both `m0-lifecycle-compare` and `m0-storage-bench`
before the Node suite. No test or assertion changed; dependency versions,
security policy, runtime behavior and platform claims remain unchanged.

Windows exact prerequisite command PASS:
`cargo build --locked -p m0-lifecycle-compare -p m0-storage-bench`.
Focused `m0-foundation.test.mjs`: **6/6**, exit 0, no failures/skips.
Diff check PASS. This local correction is not a replacement for cloud readiness.
A new-source required run is needed; rerunning the old SHA would retain the bug.

No repeated physical Mac plan is required for this workflow-only correction.
The replacement run must verify both readiness jobs and current-source Windows
ACL job; do not count an older targeted build or first failed run as final CI.
This record does not accept ADRs, mark M0 complete or authorize M1.

## Attempt 2 — corrected-source run

Run: https://github.com/gungortarik/cryptex/actions/runs/37377520994
Source: `5c074115cd61ef7c1eb47e8ed2cf8c1105360679`.
Initial run is now completed/cancelled; its Mac failure and ACL success remain
historical. This is a new-source run, not a rerun of the missing-build workflow.

- macOS readiness job `111990481997`: **PASS**. Rust debug suites **75 tests**,
  compiled release resolver **1 test** (13 intentionally filtered), lifecycle
  Node **162/162**, storage/sidecar Node **8/8**. Format, workspace strict lint/
  check, frontend build, native helpers, release build/strict lint and daemon
  foundation command passed. No Node failures/skips.
- Windows actual cross-account ACL job `111990481524`: **PASS**.
- Windows readiness job `111990481811`: **PASS**. Rust debug suites **93 tests**,
  compiled release resolver **1 test** (20 intentionally filtered), lifecycle
  Node **160/160**, storage/sidecar Node **8/8**. Format, workspace strict lint/
  check, frontend/native builds, release build/strict lint and daemon foundation
  command passed. No Node failures/skips.
- Package-only installer job intentionally skipped; not a required job in this run.

Overall run **SUCCESS**, completed 2026-10-05 21:56:45 UTC (17:56:45 EDT).
All three required jobs passed on the exact source above. Release test filters
are intentional focused coverage, not skipped full debug suite tests. No test
was weakened. This closes candidate CI verification; it does not approve human
diff/ADR decisions or complete M0.

Subsequent result/plan commits are documentation only and are not themselves
claimed to have GitHub status checks at a newer SHA. Preserve this tested source
when evaluating the final closure/PR gate; any implementation/workflow change
requires applicable fresh CI. No further workflow was dispatched to record this
result. Independent verifier/security review and explicit human acceptance remain
open; plan: `m0-independent-review-plan-20261005.md`.

## Attempt 3 — embedded frontend correction and Mac visual pass

Run: https://github.com/gungortarik/cryptex/actions/runs/37387323751
Tested source: `c7fa011672973101cb2a5519b5883b26c6909f90` (includes code fix
`65ce7b9` and Mac visual evidence commit `c7fa011`). Trigger: one deliberate
`workflow_dispatch` after focused Mac parity and visual revalidation.

- Windows readiness job `112023710665`: **PASS**, completed 2026-10-05 23:35:47
  UTC. Rust debug suites total **95 tests**, release Desktop **24/24** including
  embedded-assets sentinel, Node **160/160 + 8/8**. Format, workspace strict
  lint/check, frontend build, native helpers, embedded release build, release
  mode tests/lint and daemon foundation command all passed. No failed steps.
- macOS readiness job `112023710918`: **PASS**, completed 2026-10-05 23:17:11
  UTC. Rust debug suites total **77 tests**, release Desktop **17/17** including
  embedded-assets sentinel, Node **162/162 + 8/8**. Format, workspace strict
  lint/check, frontend build, native helpers, embedded release build, release
  mode tests/lint and daemon foundation command all passed. No failed steps.
- Windows actual cross-account IPC ACL job `112023711083`: **PASS**.
- Windows unsigned Desktop package job was intentionally **SKIPPED**; it is
  manual-only and not a required job for this run.

Overall workflow **SUCCESS**. Required readiness and ACL jobs all passed on the
exact pushed source SHA. This is the final-source CI evidence for the candidate;
it does not itself accept ADR-0012/ADR-0004 or mark M0 complete. No tests were
weakened. No additional workflow run is needed for the recorded evidence.
