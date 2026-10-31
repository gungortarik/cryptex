# Windows complete native selection and owned rollback — scoped PASS

> **Historical status note (2026-10-05):** Status statements below reflect the state when this dated plan/result was recorded, before M0 closeout. Current M0 status is COMPLETE for the synthetic foundation scope; scoped ADR-0012/0004 are ACCEPTED. This note does not expand this artifact's evidence, satisfy production residuals, or authorize M1. See `evidence/m0/m0-closure-audit-20261005.md`.

Source: `13b435d35ac905dd3ffb94bcd29978d36133bc95`.
Scope: approved isolated M0 P0-2 pilot; ADR-0012 remains PROPOSED.
Windows x64, Node v24.19.0. Desktop/helper Rust source unchanged in this slice.

## Implementation

Separate native-tree slots preserve complete baseline/candidate payloads and
their manifests. A cooperating runner holds the native OS gate across baseline
startup, clean stop, exclusive flushed intent, atomic selection replacement,
candidate startup/IPC and commit or baseline restart. No running payload is
overwritten. The original two-binary transaction schema remains unchanged.

Baseline stop must prove exit 0, released lock and absent endpoint before any
selection advance. Candidate failure authorizes rollback only with proven
cleanup. Unknown cleanup retains intent and reports failure. Rollback launches
and verifies baseline before deleting intent. Changed intent is never deleted
as if owned. Unresolved intent blocks further startup/handover; interrupted
native-session recovery is deliberately not implemented in this slice.

`package-desktop-session.mjs` now supplies typed startup failure plus structured
cleanup evidence. OS command exit status is separate from the diagnostic code.
Only the test runner reads selection; installed Desktop/startup are unchanged.

## Local checks

- Full Node suite: **110/110**, no failures/skips, exit 0 on final source.
  Includes 13 selection tests plus previous 97 tests. Prior run also passed.
- Rust Desktop **17/17** and helper **14/14**, no failures/ignored tests, exit 0.
- `cargo fmt --all -- --check`: PASS, exit 0.
- Strict all-target Clippy for Desktop/helper with `--locked`: PASS, exit 0.
- Four syntax checks (selection engine/test/runner and session adapter): PASS.
- `git diff --check`: PASS.

Selection regressions cover ordering, commit, cleaned rejection, uncertain
cleanup, stop failure, resource mutation before/after launch, pending intent,
rollback failure, lost lease, schema/path refusal and changed intent.

## Actual retained release sessions

Existing pilot-enabled loose release input retained from the earlier proof:
`C:\Users\Tarik\AppData\Local\Cryptex-Dev\p0-2\desktop-session-input-20261005-3efd2eb3a25c42179ddcb6d878e1cdc8`.
Desktop runtime is the prior `dd5021b` build; later Desktop change was test-only.
The current source Node coordinator was exercised; no fresh release rebuild
was required for an unchanged runtime. This is not installer acceptance.

Final proof root:
`C:\Users\Tarik\AppData\Local\Cryptex-Dev\p0-2\native-selection-proof-20261005-3a37dfee7e4f4e8a832c7e0711e402cf`.
Report: `native-selection-report.json` under that root.
SHA-256: `3c99e364efec88933e848edb6aadfbb4e63b80e31830eff8304e714b6e7f2025`.
Runner exit: 0. Staged full inventories are retained in each slot's manifest.

| Scenario | Baseline Desktop/daemon | Successor Desktop/daemon | Outcome |
| --- | --- | --- | --- |
| Compatible | 33112 / 1812 | 7056 / 32832 | committed candidate |
| Declared protocol 2 vs real protocol 1 | 31360 / 888 | 2324 / 4992 | baseline restored and restarted |

Rejected candidate Desktop/daemon: 28116 / 28256. Real helper IPC command exited
2, candidate cleanup exited 0, lock released and endpoint absent before rollback.
Final successors had actual OS executable paths under their selected complete
slots, Desktop parent runner 7688 and daemon parent its own Desktop. Protocol-1
`health.pong` and `lock_held=true` passed. All baseline/final stops exited 0 with
gone daemon PID, released lock and absent IPC endpoint. Native gates released;
committed/restored selections remain and pending intents are absent.

An earlier passing proof is retained at root suffix
`b8161682500541bbba4064bd97e776ec` (before diagnostic exit/code normalization),
report SHA-256 `583a16246c820011dfdc74237037b7c8cc13eff3af1e58de2fdc076035acb778`.
It is not substituted for the final-source result.

## Limits and continuation

Baseline and candidate have identical executable bytes; this proves selection
handover and health-rejection rollback, not version-changing update. Protocol
metadata differs in the rejection scenario; real protocol negotiation rejects it.
No physical flash/focus observation, signing, canonical-data acceptance,
same-user adversarial resistance or native interruption recovery is claimed.
Windows directory fsync/power-loss durability remain unverified. A final POSIX
directory flush error after unlink cannot guarantee journal retention and never
counts as successful commit/rollback.

No Actions, installed app changes, login/tray interactions or policy changes.
Mac parity subsequently passed at `b18c28b`; see
`macos-native-selection-evidence-20261005.md` for the operator report.
Next slice is interrupted native handover recovery after owned-child absence,
with structured refusal for ambiguous ownership/state.
