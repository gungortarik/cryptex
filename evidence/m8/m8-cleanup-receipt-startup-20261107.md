# Receipt-bound cleanup and policy-bound startup recovery

Date: 2026-11-07. Base revision: `af4c36b`.

## Authorized scope

The owner explicitly approved implementing the cleanup receipt proposed in
`docs/proposals/m8-import-cleanup-receipt.md`. The normative Windows decision
table in ADR-0019 section 5.1.1 records that limited amendment. No full-ADR,
production transport publication, release or platform-durability approval is
implied.

## Implemented behavior

- A closed, canonical JSON cleanup receipt (72 KiB maximum) embeds the complete
  prepared manifest and committed marker. IDs and manifest SHA-256 bind them.
  Parsing rejects unknown/duplicate fields, invalid paths/versions, noncanonical
  bytes, oversized records, aliases and conflicting transaction evidence.
- A verified final receipt permits missing cleanup companions. Every surviving
  companion and every canonical destination must match before cleanup resumes.
  A temporary receipt requires the final marker, prepared manifest and staging
  companion, with every destination matching. Partial receipts hold unchanged.
- Staging files/directories, manifest and committed marker are removed before
  the receipt. Parents are synced, including when a previous crash removed a
  companion before its sync. Existing canonical events are never edited.
- Recovery follows instance ownership, canonical inventory/device locks, then
  the importer slot. Receipt resume uses bounded canonical-lock acquisition.
  The final-marker path releases its initial importer lock before canonical
  locking, then rechecks journal state after reacquiring the slot.
- Windows startup explicitly passes its live instance guard to local-state
  recovery before identity/materialization, managed transport or IPC starts.
  Partially/all-visible unmarked prepared imports retain full pre-/post-import
  policy locks through remaining no-replace moves and marker placement.
- Temporary committed markers now require full local policy verification, an
  exact manifest binding and every destination already present. They cannot
  authorize missing moves. Context-free recovery holds this state. Final
  committed decisions and receipts do not re-admit under newer trust policy;
  retrieval still applies current policy.
- Non-Windows startup continues to hold policy-dependent recovery branches.
  Normal transport publication remains inactive on all platforms.

## Executed checks

`cargo test --locked -p cryptexd --lib import_staging::tests -- --test-threads=1`
passed **63 tests** after startup integration. This includes worker entry tests.

- Fifteen complete cleanup interruption points: before receipt creation;
  temporary sync; placement before parent sync; final sync; within staged-file
  cleanup; file/directory/transaction removal before sync; staging cleanup;
  manifest/marker deletion both before and after sync; receipt removal before
  and after sync. Each real subprocess handshake/exit 97 is checked, followed
  by two fresh recovery passes with exact canonical bytes unchanged.
- Empty/incomplete receipt after creation holds with companions preserved.
- Twenty-six malformed/conflicting/aliased/missing-evidence scenarios preserve
  all remaining non-lock file bytes across two recovery attempts.
- Import-slot, canonical inventory and device-lock contention preserve evidence;
  bounded receipt recovery refuses and succeeds after release.
- Signed startup fixtures cover no-visible abort, partial and all-visible replay,
  and temporary-marker completion. Temporary decisions reject incomplete
  destinations, corrupted signatures even with updated manifest hashes, missing
  secure-store keys, wrong runtime ownership and mismatched marker digest.
- Four signed replay interruption boundaries plus two policy-bound temporary
  marker-promotion boundaries recover without duplicate canonical data.

`cargo test --locked -p cryptexd --bin cryptexd -- --test-threads=1`
passed **14 tests**, including startup refusal before identity creation and
instance ownership during enrollment.

`cargo test --locked -p cryptexd --lib incoming_validation::tests -- --test-threads=1`
passed **9 tests**, including ownership, signature and interrupted-validator
regressions. Strict daemon library/binary/tests Clippy (`-D warnings`), Rust
format check and diff whitespace checks passed.

Windows Application Control intermittently blocked intermediate test binaries
with OS error 4551. Subsequent ordinary builds after source/lint changes executed
the tests, including expanded inventory/device contention checks. No Windows
protection settings were changed and no alternate execution path was used.

## Remaining gates and exact continuation

These are synthetic development-host process-crash checks, not sudden-power-loss
durability or native peer import evidence. No new packaged VM build, full daemon
library suite, macOS run, CI run or independent review is claimed. Native
nonzero-cutoff peer restriction still needs a supported imported peer stream;
do not enable transport publication merely because startup recovery tests pass.
Existing final/temporary cleanup receipts intentionally make older binaries hold;
do not downgrade and manually remove unfamiliar recovery metadata.
