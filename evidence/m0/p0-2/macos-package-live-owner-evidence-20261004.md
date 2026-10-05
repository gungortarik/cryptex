# Mac live-updater recovery owner guard — scoped PASS

**Source:** `b05be5e07b8289c6eedfd2fc6303a213511e4839`  
**Host:** macOS 27.0.1 arm64, Node `v24.19.0`  
**Evidence basis:** returned Mac operator/Codex report, recorded on Windows

The repository owner-guard plan passed: **48 transaction + 4 health adapter =
52/52 tests**, zero failures/skips, exit 0. Three live-updater recovery refusals,
baseline recovery after owner exit, continued updater commit after refusal,
legacy/malformed owner refusal and changed-token checks passed. Previous process
races, recovery-I/O faults and five SIGKILL boundary tests also passed.

Four syntax checks and diff check passed. No fixture remained; owned subprocess
cleanup was error-free. Mac changed no files or committed/pushed as the plan
requested, and its branch was clean. Windows records the supplied host result;
Mac commands were not independently rerun on Windows.

This verifies the candidate's conservative PID owner guard on both targets.
PID reuse may block recovery; ownerless legacy journals remain retained.
Simultaneous recovery callers, broad stress, native process locking, installed
handover, signing, canonical-data acceptance and power-loss durability remain
open. ADR-0012 remains PROPOSED.
