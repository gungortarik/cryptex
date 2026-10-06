# Mac package recovery I/O parity — scoped PASS

> **Historical status note (2026-10-05):** Status statements below reflect the state when this dated plan/result was recorded, before M0 closeout. Current M0 status is COMPLETE for the synthetic foundation scope; scoped ADR-0012/0004 are ACCEPTED. This note does not expand this artifact's evidence, satisfy production residuals, or authorize M1. See `evidence/m0/m0-closure-audit-20261005.md`.

**Source:** `9d9159c30af01b45af1c5c9f9994417f3beec611`  
**Host:** macOS 27.0.1 arm64, Node `v24.19.0`  
**Evidence basis:** returned Mac operator/Codex report; recorded on Windows

The repository recovery-I/O plan passed: **37 transaction + 4 health adapter
tests = 41/41**, zero failures/skips, exit 0. Four recovery mutation fault
tests and two failed automatic rollback cases passed. Existing controlled
concurrency and five SIGKILL boundary tests also passed. Syntax and diff checks
passed; no temporary fixture remained. The Mac branch was clean, and the Mac
implementer changed no files or committed/pushed, as the plan requested.

These are injected filesystem operation errors. Actual disk-full, native ACL
denial, power loss and directory-flush failure after intent removal are not
proved. Native process locking, installed updater handover, signing and
canonical-data acceptance remain open. ADR-0012 remains PROPOSED.
