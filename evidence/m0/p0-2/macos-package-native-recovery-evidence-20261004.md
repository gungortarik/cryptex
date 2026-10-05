# macOS native interrupted recovery cleanup — scoped PASS

**Initial source:** `486d86c55ed5bd7e41ba23707619f7a129511124`  
**Formatting commit and tested source:** `7011fc7b316c8abcaf3f761663f42d0e04bb5a68`  
**Platform:** macOS 27.0.1 arm64  
**Toolchain:** Rust/Cargo 1.98.1; Node v24.19.0  
**Scope:** isolated M0 P0-2 candidate; ADR-0012 remains PROPOSED

Recorded by the Windows implementer from the Mac operator's returned result for
`macos-package-native-recovery-test-plan-20261004.md`. These Mac commands were
not independently rerun on Windows.

## Verification

The initial format check failed. The Mac agent applied the plan's authorized
formatter repair only to `tools/m0-lifecycle-compare/src/package_recovery_claim.rs`
and committed/pushed it as `7011fc7`. Windows inspected the commit: only that
module changed, with formatter layout, punctuation and equivalent match-arm
block changes; no behavior change was observed.

Checks on the formatted source:

- Rust lifecycle tool: **13/13 PASS**.
- Node: **61/61 PASS** (51 transaction + 4 health adapter + 6 native recovery).
- No failures or skips; test exit codes **0**.
- Format, strict Clippy, locked build: PASS.
- Both native adapter/test Node syntax checks and diff check: PASS.

Six native tests passed, covering stale claim cleanup, live/malformed claim
refusal, competing cleaners and OS-lock release after a native holder's SIGKILL.
No POSIX flush or subprocess cleanup error was reported; no fixture remained.
The Mac branch was clean and synchronized with origin. Apart from the authorized
formatting repair, the Mac agent changed no files and committed no evidence.
No physical lifecycle tests or GitHub Actions were run.

## Limits

This closes the requested Mac parity and source-format checks for the test-only
helper. The historical Windows cargo-fmt Application Control failure (4551)
remains recorded; the policy was not bypassed. Native locking covers stale-claim
cleanup only, and the explicit adapter subsequently uses guarded core recovery.
Installed Desktop/daemon handover, publisher signing, broader stress, canonical
data acceptance, Windows directory fsync and machine power-loss durability remain
open. These results do not approve a production updater or accept ADR-0012.
