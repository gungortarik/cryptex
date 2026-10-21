# macOS sidecar boundary evidence

> **Historical status note (2026-10-05):** Status statements below reflect the state when this dated plan/result was recorded, before M0 closeout. Current M0 status is COMPLETE for the synthetic foundation scope; scoped ADR-0012/0004 are ACCEPTED. This note does not expand this artifact's evidence, satisfy production residuals, or authorize M1. See `evidence/m0/m0-closure-audit-20261005.md`.

**Result: scoped PASS — operator/Codex reported.**

Plan: `macos-sidecar-boundary-test-plan-20261005.md`.
Tested source: `388e46a7e5f750d88e7fc786e0c108dcea5329e4`.
Environment: macOS 27.0.1 arm64; Rust/Cargo 1.98.1; Node v24.19.0;
pnpm 11.28.0.

The owner supplied the Mac Codex result in this conversation. The Windows
implementer records that report; these commands were not rerun on Windows as
Mac evidence. No separate raw Mac artifact or artifact digest was supplied.

| Check | Reported result |
| --- | --- |
| Desktop Rust | 13/13 PASS; no failures/skips/ignored |
| Pre-Tauri CLI Node | 4/4 PASS; no failures/skips |
| Format and strict Clippy | PASS |
| Frontend and locked release builds | PASS |
| Diff check | PASS |
| Every plan command | Exit 0 |
| Fixture cleanup | Error-free |

Backend-selected path validation, refusal of renderer substitution for a backend
developer override, and rejection of a real POSIX symlink alias passed. These
are validator and pre-Tauri CLI checks, not physical renderer invocation proof.
The prior runtime proofs retain their original source and scope.

The Mac repo was clean; no files, commits, pushes or Actions were created.
No visible application or daemon was launched; earlier plans were not repeated.
The new Mac regression gate is closed. Human ADR/M0 acceptance, signing and
distribution, and final CI remain separate and open.
