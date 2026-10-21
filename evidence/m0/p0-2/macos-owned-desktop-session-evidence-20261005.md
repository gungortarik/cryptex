# macOS owned Desktop session — scoped PASS after build correction

> **Historical status note (2026-10-05):** Status statements below reflect the state when this dated plan/result was recorded, before M0 closeout. Current M0 status is COMPLETE for the synthetic foundation scope; scoped ADR-0012/0004 are ACCEPTED. This note does not expand this artifact's evidence, satisfy production residuals, or authorize M1. See `evidence/m0/m0-closure-audit-20261005.md`.

Status: **SCOPED PASS** for same-package owned sessions. Initial failed attempt
is preserved below; version-changing update and rollback remain unverified.

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

## Mac retry — operator-reported PASS

Tested source: `6712269b6fffe5d39df81d273bf3e46585a4e0dd`.
macOS 27.0.1 arm64; Node v24.19.0; Rust/Cargo 1.98.1; pnpm 11.28.0.
The operator reported the following results; the retained Mac report was not
independently read from the Windows host.

| Check | Result |
| --- | --- |
| Format | PASS |
| Desktop Rust tests | 10/10 |
| Lifecycle helper tests | 14/14 |
| Strict Clippy and locked helper build | PASS |
| Current-source .app packaging | PASS; Desktop and daemon arm64 |
| Node tests | 97/97; no failures/skips |
| Five syntax checks and diff check | PASS |

All commands exited 0.

| Owned session | Desktop PID | Daemon PID | Desktop parent PID |
| --- | ---: | ---: | ---: |
| baseline-first | 72303 | 72313 | 72300 |
| baseline-reopen | 72322 | 72330 | 72300 |

Daemon parent PIDs matched their owning Desktops. Executable paths matched
the staged `proof/payload/Contents/MacOS/` binaries. Both sessions returned
protocol 1 `health.pong` with `lock_held=true`. Both stopped with exit 0,
absent daemon PID, released daemon lock and absent IPC endpoint. Native gate
was released cleanly. No fixture/child cleanup or flush error was reported.

Retained report:
`/Users/tarikgungor/Library/Application Support/Cryptex-Dev/p0-2/mac-owned-session-20261005-7E3551C1-50DC-48E3-8867-B0024C7E88CD/proof/desktop-session-report.json`.
Reported SHA-256:
`5d4e004a844f08fb7da2b1a6b43a7d8345badcb4fbe0fbba5fb4a4f917632883`.

Snapshots recorded `window_visible=false`; physical window flash or focus
changes were not separately observed. Mac repo remained clean; no source/evidence
edits, commit/push or Actions were performed on Mac.

## Continuation and limits

The corrected Mac build and same-package owned-session gate now pass. Next is
complete-package selection/intent coordination and failed-candidate baseline
restart using these sessions under the approved isolated pilot.
ADR-0012 remains PROPOSED. This result does not prove version-changing update,
rollback, signing, canonical-data acceptance or interrupted native handover.
