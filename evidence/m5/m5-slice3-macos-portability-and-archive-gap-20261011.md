# M5 Slice 3 -- macOS portability verification and archive UX status

Date: 2026-10-11. macOS 27.0.1 (26A434), arm64.

## Sync

- HEAD: `62342d88865d0ac0fe54d53b805c316daeea662c`
- Product ancestor `eba337fcf2e81da8163254ff2092690f2a695ce3`: PASS
- Menu-owner ancestor `b9b3e4cdfdc22077c5dd5ec78b0dbbf22969863f`: PASS
- Working tree was clean before sync; no reset or stash reported.

## Portability verification

Owner-reported results at the above source:

- Desktop Cargo fmt check: PASS
- `pilot_runtime`: 8/8 PASS, including isolated runtime with absent build-time repository
- Release Desktop strict Clippy: PASS

This verifies the portability fix on macOS. It does not constitute a new native
Quit interaction run; earlier scoped native Quit results remain recorded at
their tested product SHA.

## Archive / unresolved capture UX

Automated coverage is reported for archive tracking, late A commit, deliberate
new B acknowledgement, resolve-archive, and Quit protection for archived or
unknown states. These are automated results, not native UX evidence.

Still NOT VERIFIED interactively on macOS:

- Cause an in-pilot Uncertain A through supported product paths, then deliberate
  new B with duplication acknowledgement; verify A appears under
  “Earlier unresolved captures” with Resolve.
- Quit Cancel preserves both the draft and archive row, with both processes alive
  and one dialog; Confirm cleanly stops both processes and discards session-only
  pending/archive state.
- Optional close-to-tray and reopen retains that archive row.

Stop if the product acknowledgement path cannot be reached without fabricating
daemon or committed state. Do not add a production fault hook to manufacture
the scenario. The smallest safe next step is to try this sequence in a fresh,
isolated synthetic pilot and record the precise point reached. If it cannot be
reproduced through supported product paths, report the blocker and leave the
native archive UX gate open.

## Windows evidence reconciliation

A contemporaneous Mac continuation report described Windows native Quit as
OPEN. That status is superseded by owner-observed Windows guest evidence at
`eba337fcf2e81da8163254ff2092690f2a695ce3`, recorded in
`m5-slice3-windows-owner-native-quit-pass-20261011.md`: X/tray reopen preserved
the synthetic draft, tray Quit Cancel/retry preserved it with one prompt per
attempt, and tray Confirm cleanly stopped Desktop and daemon. Windows
archive-specific UX remains unverified.

## Status

M5 Slice 3 is not closed. Archive-specific native UX disposition remains open;
Windows and macOS generic native Quit coverage is recorded separately. No
Actions, merge, Slice 4, or M5-complete claim.
