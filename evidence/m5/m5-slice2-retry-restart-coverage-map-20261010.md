# M5 Slice 2 -- Retry and Restart Safety coverage map

**Date:** 2026-10-10
**Branch:** `codex/m5-capture-timeline`
**Base tip:** `6eba9b15aa0047eb003d7b0a758c1aa7526b881e`
**Authority:** `docs/milestones/M5_CAPTURE_TIMELINE.md`,
`evidence/m5/m5-kickoff-plan-20261010.md` item 2,
`evidence/m5/m5-slice1-capture-reconciliation-contract-20261010.md`.
**Authorization:** Owner authorized Slice 2 (retry/restart safety) on 2026-10-10.

## Slice 1 coverage reused (in-process)

| Requirement | Existing Slice 1 proof | Gap for Slice 2 |
|---|---|---|
| Tip failure after durable append then resolve | `tip_failure_after_append_is_uncertain_then_resolve_commits` | Same scenario across process restart |
| Seal failure before tip then resolve | `seal_failure_before_tip_is_uncertain_then_resolve_commits` | Process restart variant optional if tip path covered |
| Receipt persist failure after append then resolve | `receipt_persist_failure_after_append_is_uncertain_then_resolve` | Process restart variant |
| PENDING absent resolve = not_committed; capture appends | `pending_absent_resolve_not_committed_then_capture_appends` | Durable PENDING then kill before append, then restart |
| Lost-response style replay (Committed receipt present) | `successful_capture_commits_and_replays` | Fresh process must replay without shared heap/latches |
| Concurrent matching capture_ids | `concurrent_matching_capture_ids_append_once` (threads) | Process-boundary concurrency |
| Distinct capture_ids, identical text | `intentional_identical_text_with_distinct_capture_ids` | Keep regression; optional xproc smoke |
| Receipt missing / conflict / pack confirm / identity fail-closed | Several Slice 1 unit tests | Keep fail-closed under restart (corrupt/missing) |
| Staged exclusive PENDING no-replace | `pending_receipt_is_staged_exclusive_no_replace` | Partial stage must not yield false Committed after restart |
| Canonical recovery without SQLite | `receipt_loss_resolve_uncertain_and_note_still_in_packs` | Explicit restart proof without projection wiring |

## Slice 2 remaining gaps (this slice)

1. Cross-process restart harness with file-backed test SecureStore and env fault points.
2. Durable PENDING before append -> restart resolve absent; matching capture resumes once.
3. Durable append before tip/COMMITTED receipt -> restart reconciles to Committed, one event.
4. Durable COMMITTED before response delivery -> lost-response replay in fresh process.
5. Partial/staged receipt publication across process boundary.
6. Concurrent matching requests across process boundary remain single-event.
7. Missing/corrupt receipt remain fail-closed after restart.
8. No projection/SQLite wiring; process termination is not claimed as power-loss proof.

## Out of scope (unchanged)

Desktop capture IPC/UI, timeline, extraction, projection-as-saved, later slices,
Actions, merge, production v2, extending receipt-loss duplicate prevention.
