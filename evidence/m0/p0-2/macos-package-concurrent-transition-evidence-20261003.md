# Mac package-candidate concurrent transition parity

> **Historical status note (2026-10-05):** Status statements below reflect the state when this dated plan/result was recorded, before M0 closeout. Current M0 status is COMPLETE for the synthetic foundation scope; scoped ADR-0012/0004 are ACCEPTED. This note does not expand this artifact's evidence, satisfy production residuals, or authorize M1. See `evidence/m0/m0-closure-audit-20261005.md`.

**Result:** scoped PASS, reported by the Mac operator/Codex  
**Source:** `ad379892ebd26f74544d37aa56e2552cc71daeee`  
**Host:** macOS 27.0.1 arm64, Node `v24.19.0`

The Mac implementer executed the repository parity plan and returned results
to the Windows implementer without changing files or committing.

- Transaction suite: **31/31 passed**, zero failures or skips.
- Both deterministic concurrent-transition regressions passed.
- All five actual owned-worker SIGKILL boundaries passed.
- Engine/test syntax and `git diff --check` passed.
- No temporary test directory remained; branch was clean and synchronized.

This records the supplied Mac-host result; the Windows host did not rerun Mac
commands. No GUI, installer or GitHub Actions run was reported.

The two controlled interleavings are verified on both platforms. OS-level
process locking, cross-process stress, recovery-I/O failure, native executable
handover, signing, power-loss durability and canonical-data acceptance remain
open. ADR-0012 remains PROPOSED.
