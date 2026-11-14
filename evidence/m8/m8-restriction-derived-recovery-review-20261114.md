# M8 restriction and derived-state recovery — November 14

Verified source: `c6e07d6ab5acccabd73258780bc1acf95ede6333`.
Exact-source native verification passed on Mac and Windows.
This slice changes tests only; no product gate or protocol policy is activated.

## Actual path and checks

Two isolated daemon profiles enroll real synthetic keys, exchange all four
signed pairing messages and append their own canonical approval anchors through
the daemon writer. A sealed peer pack is copied byte-for-byte under canonical
locks as fixture transport. This is import-visible state, not live Syncthing.

The receiving profile builds the real on-disk SQLite M3/M4 database and a selected
Obsidian vault. Successful guarded search initially exposes three notes: the
local sequence1 and peer sequences1/3. The vault initially contains those same
three notes and a separate owner-owned file.

For each cutoff0/1, an actual immutable owner request binds the approved peer,
verified cutoff hash and transaction. The journal records confirmation, then
the daemon writes a signed v3 restriction. An existing failure seam interrupts
after the durable canonical append and before the tip/journal completion.
The operation must be `AppendUnknown` with exactly one append attempt.

Search, timeline, detail and vault rebuilding must all return the precise
`MaterializationStateMissingOrStale` Brain-wide hold. The existing vault files
remain untouched. Existing canonical prefixes remain byte-identical; the
restriction is additive. Old exported files are not remotely erased by a hold.

Closing SQLite, reloading the journal/identity and running startup reconciliation
must find the existing event and mark it anchored with the same single attempt.
The reopened stale DB must still hold all four surfaces. A SQLite trigger then
aborts the M4 rebuild after the separate M3 transaction has committed. The test
requires the exact injected error message, raw M3 detail proves excluded content
has gone, and all guarded surfaces must still hold with the old vault unchanged.

After removing the synthetic fault, two independent DB reopen/rebuild rounds
must expose exactly the admitted IDs in search, timeline and detail. Managed
vault entries must be regular files with exactly the admitted `<event_id>.md`
names. Cutoff0 keeps only the local note; cutoff1 also keeps the first peer note.
The excluded peer tail disappears from the refreshed managed vault, while the
owner-owned file survives. Repeated startup reconciliation and rebuilds must
not append or modify canonical bytes.

A third test reopens actual paired histories with an empty, truncated or
garbage-suffixed journal. Startup reconciliation must return a typed verification
error without rewriting journal/canonical evidence or minting a key. After
restoring the valid journal, deleting the local signing seed at the fake
SecureStore facade must prevent a new resolver/startup on two attempts, preserving
public metadata, journal, canonical bytes and secure-store put count. Restoring
only that synthetic fixture's exact original seed permits fresh verification.

## Scope and exclusions

This is daemon-service recovery using real files and SQLite, not a Desktop
process restart, physical power-loss test, installed product acceptance or actual
OS credential deletion/reinstall. SQLite rollback is injected through a derived
DB trigger; it does not claim arbitrary filesystem power-failure coverage.
Canonical events are written only by existing daemon paths. Test runtime stays
outside the repository; fixture transport creates new destination files under
the established lock order. Complete protocol acceptance, configured Mac product
image policy, selected OS-floor enforcement and final supported product flows
remain separate.

## Review and native verification

Independent read-only source review found one assertion gap: filename stems
alone would accept wrong extensions/directories. The final test checks regular
files and exact `.md` names. Review otherwise found no remaining actionable issue
in this slice. Agent source review is not an external human audit.

Local Rust remains blocked by Application Control4551. Native CI is used without
bypass. Runs43626119302 and43626304661 stopped at native formatting; exact
formatter output was applied without disabling tests or weakening assertions.
Final [CI43626437079](https://github.com/gungortarik/cryptex/actions/runs/43626437079)
passed Mac/Windows readiness and Windows cross-account IPC ACL on the exact
source above. Daemon library: Mac538passed/7ignored, Windows659passed/5ignored,
zero failures. All three new tests passed both platforms; their exact names,
job IDs, source-file SHA-256 and limits are in the
[machine receipt](m8-restriction-derived-recovery-native.json).
Required workspace formatting/lint, release Desktop and Node recovery/packaging
checks also passed. Separate manual native Desktop/identity/installer jobs were
not rerun. Ordinary Mac readiness does not run Windows-only incoming publication.

The same run confirms existing common approval-domain failed-delivery/completion
tests and journal pre-replace, post-replace and unchanged-witness sync faults on
both platforms. Independent coverage reconciliation found no new lower-layer
algorithm campaign. The nine-row §5 map retains required supported-product
crash/uncertainty checks without turning evidence exclusions into automatic
new installed-Desktop, OS-store-reinstall or physical power-loss campaigns.
