# Terminal retention and canonical recovery review — November 13

Verified implementation: `40da5f7dc0a3139828b915f37eecab9f2d9c1a15`.
Owner decision: compact receipts under the existing one-MiB journal cap, keeping
cancelled IDs reserved and uncertain evidence intact. macOS26+ was separately
selected as sufficient; OS deployment/runtime enforcement remains separate.

## Changes and independent review

- Closed wire3 separates at-most-sixteen active operations from bounded terminal
  receipts. Every lookup sees one validated unified view. No terminal IDs are
  evicted and no canonical event is changed. Legacy1/2 input validates before
  lazy durable migration; legacy anchors need fresh canonical proof to compact.
- Anchored receipts retain exact operation/transcript/position/key binding.
  Cancelled-before-release receipts cannot recreate the exchange. Released or
  uncertain rows remain full. Full storage yields typed capacity refusal.
- Independent review found a pre-existing bilateral recovery bug: the same
  transaction intentionally exists in both authors' streams, but old lookups
  treated the peer's record as a local duplicate. Transaction uniqueness now
  follows the specified local-author namespace across all security types;
  event-ID uniqueness remains global.
- Startup uses one complete canonical-locked history and indexes both ID
  namespaces. All selected receipts must match before a journal-lock reload
  compares the entire pre-scan view and commits once. A late invalid witness
  cannot leave earlier legacy upgrades partially written. Unchanged receipts
  re-sync the existing file/Unix parent without replacement, preserving typed
  uncertainty if that durability step fails.
- Separate agent source review of2a1af78 found no remaining actionable finding.
  Follow-up40da5f7 contains native rustfmt changes only. This is agent source
  review, not an external human audit or final product-flow acceptance.

## Executed evidence

[Full CI43518226858](https://github.com/gungortarik/cryptex/actions/runs/43518226858)
passed Mac readiness, Windows readiness and Windows cross-account IPC ACL.
Daemon library: Mac532passed/7ignored, Windows653passed/5ignored, no failures.
Each platform passed14terminal tests,4real bilateral/recovery tests and existing
self-retirement durable-append recovery. Required format, strict workspace and
release-only lint, release Desktop builds/tests, Node recovery/packaging and
schema drift checks passed in their workflow scopes.

The seventeen-anchor fixture uses actual separately enrolled keys, four signed
messages per fresh transaction and genuine daemon appends. One sealed counterpart
arrives through a daemon-owned locked fixture copy; it is not a live Syncthing
test. Both fresh domain reconciliations assert one policy scan, zero journal
rewrites, unchanged canonical/journal bytes and less than the existing ten-second
native startup budget. That limit covers each reconciliation only. Fixture setup
is outside it (the Windows test reported running for over60seconds while creating
the histories). This is not a claim that a full Desktop process launches with
seventeen transactions in ten seconds.

[Scoped native CI43518652758](https://github.com/gungortarik/cryptex/actions/runs/43518652758)
passed on the exact same source. All49required Mac markers, eight native endpoint
cases, five HTTP boundary cases per platform, actual signed Desktop startup,
real Syncthing identity/pairing/restart/loss and exported durable/transport/
combined-loss launchers passed. Existing ignored cases remain outside ordinary
readiness totals. No unchanged human Mac/VM package was requested again.

Local105Python tests passed with5pre-existing platform skips; all8PowerShell
groups passed. Local Rust tools remain blocked by Application Control4551;
no policy bypass, renamed executable or weakened deadline was used. Runtime and
downloaded job logs remain outside the repository; the pre-existing untracked
`scripts/m8/__pycache__/` was preserved.

## Failed attempts and limits

Preserved failures:30504aa CI43512759064 formatting;277849b CI43513157963 strict
clippy;5e1c1c2 CI43513743214 missing test Value import;2a1af78 CI43518008167
formatting. The corrected baselineadad524 passed CI43514494333; final40da5f7
passed both runs above. Tests and expected protocol bytes were not weakened.

The [machine receipt](m8-terminal-journal-native-40da5f7.json) pins source, jobs,
test inventory and artifact digest. The [recovery map](m8-recovery-coverage-map-20261113.md)
distinguishes existing evidence from uncovered rows. This closes the selected
retention/recovery slice. Candidate schema artifacts, old-product signed-v3
Brain hold, full signed target-history extrema, complete protocol/product
decisions and final seven-area acceptance remain open. Default Mac product
routes and publisher factory stay disabled. M8 remains OPEN; M9 has not begun.
