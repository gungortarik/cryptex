# M16 hardening campaign

M16 is open. The implementation baseline is the M15 closeout at `50869d6`. The campaign uses disposable identities and synthetic records. Each run below identifies the revision it executed. Acceptance requires a reviewed candidate, both native platforms, recorded limits and the [Definition of Done](../../docs/21_DEFINITION_OF_DONE.md).

## Failure coverage

The section numbers refer to the [failure matrix](../../docs/17_TEST_AND_FAILURE_MATRIX.md). Earlier acceptance records identify existing checks that must be considered in the final review.

| Sections | Failure or boundary | Runtime evidence and outstanding work |
| --- | --- | --- |
| 8, 9 | Interrupted canonical write, incompatible or damaged record, checkpoint loss, stale index and rebuild | [Storage baseline](../m2/m2-closeout-l1-l2-f4-f6-20261007.md), [projection baseline](../m3/m3-final-review-20261008.md), [M16 signed-pack reconstruction](https://github.com/gungortarik/cryptex/actions/runs/45119773599). All six scale jobs passed. |
| 10 | Storage layout and platform-specific scale | [Original packaging comparison](../m0/p0-1/) and the new signed-pack campaign. The M16 workload measures verified reading, rebuilding and FTS lookup. Writer throughput and Obsidian projection require separate evidence. |
| 11 | Window close, login, duplicate writer, service crash, reconnect and failed update | [Installed platform review](../m14/platform-closeout-review.md) includes real temporary-account receipts. [Installed update rehearsal](https://github.com/gungortarik/cryptex/actions/runs/45119157833). |
| 12, 15 | Local endpoint, role, capability, incompatible or malformed request, restricted retrieval and explicit consent | [General CI and cross-account Windows ACL](https://github.com/gungortarik/cryptex/actions/runs/45120098263), [local MCP rehearsal](https://github.com/gungortarik/cryptex/actions/runs/45119158085), [prior endpoint review](../m1/m1-final-review-20261007.md). Final review must also trace renderer and Doctor outputs. |
| 13, 14 | Managed transport isolation, unavailable peer, delayed delivery, duplicate replay, restriction and repeated reconnect | [Trust and replication review](../m8/m8-final-candidate-review-20261118.md), [packaged repeated partition run](https://github.com/gungortarik/cryptex/actions/runs/45120714690). Live OS suspend and resume requires a separate observation. |
| 16 | Interrupted backup, damaged generation, wrong recovery key, restore and reconstruction after source loss | [Installed recovery](https://github.com/gungortarik/cryptex/actions/runs/45119158424), [successful bidirectional baseline](https://github.com/gungortarik/cryptex/actions/runs/45119112938), [bidirectional rejection and retry](https://github.com/gungortarik/cryptex/actions/runs/45120234060). Review large objects, independent Kit selection and recovery of import receipts. |
| 17 | Purged history, stale replica, shared attachment, unreachable copies | [Forget and purge rehearsal](https://github.com/gungortarik/cryptex/actions/runs/45119157942), [scope of cleanup](../m10/forget-purge-closeout-review.md). Review interactions with backups and accepted imports. |
| 18, 19 | Displayed health during service loss, maintenance and restoration, privacy of diagnostics | [Desktop and Doctor rehearsal](https://github.com/gungortarik/cryptex/actions/runs/45120098524), [desktop review](../m13/desktop-shell.md). Verify that a configured endpoint alone cannot produce remote durability or active status. |
| 20–23 | Required CI, traceable receipts, complete hardening scope | Final platform CI, reviewed material risks, a physical sleep/wake soak and human acceptance are open. |

## Reproduction

- **Signed history.** Run the exact ignored daemon test `projection::scale_test::signed_pack_projection_recovers_at_scale` in Cargo release mode. Set `CRYPTEX_M16_EVENTS` to 10000, 100000 or 500000. One synthetic device contributes signed notes in packs of 1,000. For each count the harness measures two independent rebuilds, reopens SQLite, checks every pack digest, the signed chain tip, the exact note count and an FTS match at the end of every pack. The recorded times are workload observations.
- **Repeated partitions.** Run the M16 repeated offline sync workflow. Each OS hosts two authenticated native transports bound to loopback. A disposable standard account runs the Windows test. The same pair retains its enrolled identities across six cycles of four new notes per author. Each phase starts a fresh daemon test process. After each online publication the receiver retries the same signed pack, compares the canonical bytes, and checks guarded search again after both transports exit. This exercises 48 new notes plus two genesis notes.
- **Encrypted peer recovery.** Run the M11 cross-platform restore workflow. The recipient tries a truncated ciphertext and a valid generation with a different disposable Kit. It checks for any newly published or residual staging entries after each failure. It then uses the original generation and correct Kit at the same destination, verifies the signed inventory through the daemon and checks the reconstructed notes. Only synthetic ciphertext and public exchange material cross CI hosts.

## Results and open gates

### Signed history measurements

All six native release jobs at `4a87c53` passed. Each timed rebuild includes opening the derived database and verifying the signed history. The query below selects a known note from every pack after closing and reopening the database. A displayed zero means less than one millisecond at the timer's reporting resolution. [Native job logs](https://github.com/gungortarik/cryptex/actions/runs/45119773599)

| Platform | Signed notes | Missing DB rebuild (ms) | Corrupt DB rebuild (ms) | Query after missing / corrupt rebuild (ms) |
| --- | ---: | ---: | ---: | ---: |
| macOS | 10,000 | 1,400 | 1,328 | <1 / <1 |
| macOS | 100,000 | 18,623 | 22,846 | <1 / <1 |
| macOS | 500,000 | 84,710 | 84,225 | 1 / 1 |
| Windows | 10,000 | 2,471 | 2,450 | <1 / <1 |
| Windows | 100,000 | 29,550 | 24,102 | 1 / 1 |
| Windows | 500,000 | 96,538 | 96,354 | 4 / 3 |

### Security and recovery review

The MCP tool list and dispatch use explicit names for health, requesting review and searching public notes. The daemon checks the approved stream, client role, capability, deadline and profile admission. The reader filters visible public notes before returning up to ten excerpts. Read access to a recovered profile is held when the MCP reader cannot verify both histories. The successful native MCP and forget rehearsals exercise the restricted and stale-index cases. [Bridge dispatch](../../tools/local-mcp/server.mjs) · [Daemon authorization](../../services/cryptexd/src/ipc/public_context.rs) · [Guarded context](../../services/cryptexd/src/knowledge_materialization/ai_context.rs)

The Windows worker's retained first-failure record uses an explicit schema with integer fields for the subsystem and optional OS code. The recorder preserves the first error across later shutdown and returns the original failure even when it cannot persist the diagnostic. The Desktop declares an explicit invoke list. The installed-update rehearsal exercised retained package fingerprints, the service connection after rollback and canonical note preservation on both native systems. [Worker diagnostics](../../services/cryptexd/src/managed_profile/worker/diagnostics.rs) · [Desktop registration](../../apps/desktop/src-tauri/src/main.rs) · [Update and rollback run](https://github.com/gungortarik/cryptex/actions/runs/45119157833)

At `cf2cf3a` both platforms rejected the truncated peer backup and a valid backup with the incorrect Kit. Both then opened the original generation with the correct Kit and verified two signed notes via the native daemon. [Cross-platform recovery run](https://github.com/gungortarik/cryptex/actions/runs/45120234060)

### Remaining decisions and observations

The M15 acceptance record describes two recovery limits. Purging a note linked to an accepted Cortex batch can prevent the next backup of that profile. Losing all import evidence before creating a recovery selection can eliminate the evidence identifying Cortex origin. Review the effects on surviving notes and on the retained independent recovery selection against risks R-17, R-18 and R-20 before a personal cutover. [M15 evidence and limits](../m15/cortex-import-review.md#evidence-and-limits)

The final review also needs current evidence for privilege boundaries, dependency and native-package provenance, redaction, unattended operation after a real system sleep, and the distinctions between a locally ready transport, completed replication and verified recovery. [Risk register](../../docs/22_RISK_REGISTER.md)
