# Mac separate-process package race parity — scoped PASS

**Source:** `6b8776c479efcb3091985a7c90f7987ba74d2e2e`  
**Host:** macOS 27.0.1 arm64, Node `v24.19.0`  
**Evidence basis:** returned Mac operator/Codex report, recorded on Windows

The repository separate-process parity plan passed: **41 transaction + 4 health
adapter = 45/45 tests**, zero failures/skips, exit 0. All four separate updater
process race cases passed, along with prior recovery/concurrency tests and five
SIGKILL boundaries. Three syntax checks and diff check passed. No fixture
remained, and owned subprocess cleanup completed without error.

Mac changed no files or committed/pushed, as requested by the plan. Its branch
was clean. The Windows implementer records the supplied host results; Mac
commands were not independently rerun on Windows.

This verifies four controlled process races on both platforms. Broad stress,
recovery competing with an active updater, OS-level process locks, installed
updater handover, signing, power-loss durability and canonical-data acceptance
remain open. ADR-0012 remains PROPOSED.
