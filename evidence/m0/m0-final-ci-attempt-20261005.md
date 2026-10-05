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
