# macOS owned Desktop session — initial build failure and retry pending

Status: **NOT YET VERIFIED**.

## Initial Mac operator report

- Source: `8152267bff436c3ec5ddeeb8066da7c32e3bffa3`.
- macOS 27.0.1 arm64; Node v24.19.0; Rust/Cargo 1.98.1.
- Format and `git diff --check`: PASS.
- `cargo test -p cryptex-desktop -p m0-lifecycle-compare --locked`: exit 101
  during Desktop compilation, before test execution.
- Error: `symbol '_EMBED_INFO_PLIST' is already defined`.
- Both main.rs:880 and pilot_runtime.rs:194 expanded `generate_context!()`.
  The test executable included two embedded macOS Info.plist symbols.
- Clippy, packaging, Node 97 tests and live sessions were not attempted.
- No pilot processes/fixtures, edits, commit/push or Actions. Mac repo clean.

This record is based on the supplied Mac operator report, not a Windows
reproduction of the macOS linker/compiler diagnostic.

## Scoped correction and Windows checks

Fix source: `49e4e5d2342a8714ea92554cbf3f5130235537f8`.
The config unit test uses a plain Config with two explicitly visible, named
windows. It checks that configuration changes the identity, hides both windows
and preserves their labels/count. Main remains the single context generator.
No application runtime behavior or dependencies changed.

On Windows, the corrected source passed:

- `cargo fmt --all -- --check`: exit 0.
- `cargo clippy -p cryptex-desktop --all-targets --locked -- -D warnings`: exit 0.
- `cargo test -p cryptex-desktop --locked`: **17/17**, no failures/ignored tests,
  exit 0, including all six pilot tests.
- `git diff --check`: exit 0; source search found one context macro, in main.

The previous local Application Control 4551 failure is retained in Windows
session evidence; this new local execution succeeded without policy changes.
No Actions or GUI interaction was used for this correction.

## Continuation

Run `macos-owned-desktop-session-test-plan-20261005.md` from the beginning on a
clean current branch containing the fix. Report the exact tested SHA and all
actual results, including a failure if any. Mac Rust tests, current `.app`
packaging, Node suite and both owned live sessions remain unverified.
ADR-0012 remains PROPOSED; this correction does not prove update/rollback,
signing, canonical-data acceptance or interrupted native handover.
