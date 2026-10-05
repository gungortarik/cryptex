# macOS complete native selection and owned rollback — scoped PASS

Tested source: `b18c28b39428bd87b4117e4bc1b461dc0798ccee`.
Plan: `macos-native-selection-test-plan-20261005.md`.
Scope: approved isolated M0 P0-2 pilot; ADR-0012 remains PROPOSED.

This record transcribes the Mac operator/Codex report. The retained Mac JSON
was not independently accessed from Windows.

## Environment and checks

macOS 27.0.1 arm64; Node v24.19.0; Rust/Cargo 1.98.1; pnpm 11.28.0.

- Desktop Rust **10/10**, lifecycle helper **14/14**, Node **110/110** PASS;
  no failures/skips.
- Format, strict Clippy, locked helper build and current arm64 .app packaging PASS.
- Four syntax checks and diff check PASS.
- All plan commands and live runner exited 0.

## Actual complete-slot sessions

| Session | Desktop PID | Daemon PID |
| --- | ---: | ---: |
| Compatible baseline | 73913 | 73921 |
| Compatible candidate | 73937 | 73947 |
| Protocol-reject baseline | 73959 | 73967 |
| Rejected candidate | 73977 | 73984 |
| Restarted baseline | 73993 | 74000 |

Compatible handover was `committed`, selected slot `candidate`. The other
candidate expected protocol 2 but real IPC reported protocol 1; helper exit 2
was the expected rejection. Candidate cleanup succeeded before rollback.
Rollback was `rolled-back`, selected slot `baseline`, and restarted baseline
IPC health passed.

All Desktop parent PIDs matched runner 73910; each daemon parent was its own
Desktop. Executable paths matched the selected complete slot. Healthy sessions
returned protocol 1 `health.pong` with `lock_held=true`.

All five sessions exited cleanly with code 0; recorded PIDs were gone, daemon
locks and both native gates released, and IPC endpoint absent. Both scenario
`native-intent.json` files were absent. No flush or cleanup error was reported.
Snapshots verified hidden windows; physical flash/focus was not independently
observed.

Retained report:
`/Users/tarikgungor/Library/Application Support/Cryptex-Dev/p0-2/mac-native-selection-20261005-6AFF5A78-9675-4B5F-BBBF-BC87493B741A/proof/native-selection-report.json`.
Reported SHA-256:
`4c45d110156d8f37357333cb7cda195af86bedeba5d19a87da21e232fe1ef38c`.

Mac repo was clean. No source/evidence edits, commit or Actions were performed.

## Limits and continuation

Same executable bytes were used in both slots. This closes Mac parity for
complete native selection and health-rejection rollback, not version-changing
update, installed updater, signing, canonical-data acceptance, power loss or
interrupted native-session recovery. ADR-0012 remains PROPOSED.
Next implementation is interrupted native handover recovery with proof of
owned-child absence and typed refusal for ambiguous ownership/state.
