# Mac concurrent recovery claim parity — scoped PASS

**Source:** `0ce9c93f621dceedd2edb78d736eb526f760a817`  
**Host:** macOS 27.0.1 arm64, Node `v24.19.0`  
**Evidence basis:** returned Mac operator/Codex report, recorded on Windows

The repository recovery-claim parity plan passed: **51 transaction + 4 health
adapter = 55/55 tests**, zero failures/skips, exit 0. All three new claim cases
passed: concurrent recovery/updater refusal, prevalidated updater refusal after
old-intent removal, and interrupted recovery retaining claim/journal while
refusing automatic reclamation. Prior recovery-I/O faults, owner guards,
process races and five updater SIGKILL boundaries also passed.

Four syntax checks and diff check passed. No fixture remained; owned subprocess
cleanup was error-free. Mac changed no files or committed/pushed as the plan
requested, and its branch was clean. Windows records the supplied host result;
Mac commands were not independently rerun on Windows.

The exclusive candidate claim is verified on both targets. An interrupted
recovery still leaves a retained claim requiring inspection; automatic repair
and native OS locking are not established. Broad stress, signing, installed
handover, canonical-data acceptance and power-loss durability remain open.
ADR-0012 remains PROPOSED.
