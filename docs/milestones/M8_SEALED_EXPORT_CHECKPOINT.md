# M8 admitted sealed export checkpoint

This implements the already accepted direction in
`docs/proposals/m8-product-flow-activation-20261115.md`. It does not introduce a
new trust protocol, signed remote receipt or canonical format. Native acceptance
and the real Windows–Mac network test remain separate evidence gates.

## Authority and normal scheduling

The daemon connection coordinator schedules export only while ReadyLocally,
with an owned transport reader and current transport generation. Paused,
address-needed, unenrolled and held profiles do not export. Each batch retains
operation admission before acquiring the existing canonical inventory/device
locks. It loads the existing local signing identity, verifies the complete
canonical union, and derives current ExchangePeers. Neither saved preferences
nor an adapter identity alone authorize export.

The export plan borrows the locked snapshot's complete physical security frames,
including security records. It never uses the content-only projection as pack
bytes. Exact frame offsets, original bodies and canonical frame headers establish
the expected complete file length and SHA-256. Source files must match that plan,
be regular single-link files and remain stable while read. Every source and
destination operation retains original directory capabilities. Windows performs
no-replace rename on the opened file handle; Mac uses original directory
descriptors and exclusive rename. Filesystem aliases are not adopted on retry.

## Seal and copy

Only the local device's verified active tail can be sealed, under its already
held writer lock. The checkpoint atomically renames the complete verified `.open`
to the corresponding `.seg`; it allocates no sequence, signs nothing and changes
no event bytes. The rebuildable writer tip is reconciled from canonical files by
the next writer, as after an interrupted ordinary seal. Peer active tails are
never sealed or rewritten by this device.

Copies go through a single private `transport/export-staging/segment.tmp` leaf,
outside the shared tree, and are flushed and re-read before no-replace publication
into `transport/incoming/<device-hex>/<20-digit-index>.seg`. Canonical data is
never hardlinked into transport. Existing destinations, including a concurrent
delivery that wins publication, must match the complete expected length/hash.
Conflicts are retained and stop this coordinator with Attention; no overwrite or
automatic conflict repair is performed. Remote deletion can cause safe
repopulation, never canonical deletion.

On retry, only that exact private derived scratch leaf can be discarded, through
its opened file/original directory capability. No arbitrary imported pathname
selects a cleanup target. A partial copy or completed publication requires no
new canonical journal: retry validates the current canonical source and existing
destination anew. I/O failures during copying/flushing carry error kind and OS
code; diagnostics contain no content, credential or private key.

## Limits, withheld packs and truthful status

- Scheduled at most once per five seconds in the connection actor.
- One batch: at most 16 physical packs, 1 GiB per pack and 4 GiB aggregate,
  matching importer admission limits; at most one staged pack consumes scratch.
- A 20-second cooperative deadline and cancellation/generation checks apply to
  history scanning, plan construction, hashing and each 64 KiB copy iteration.
  Blocking OS filesystem calls are not claimed forcibly interruptible.
- A process-local cursor advances over completed, identical and withheld packs;
  later packs cannot be starved by the first 16 already-present entries. It resets
  after a full traversal. Restart revalidates from the beginning.
- A whole physical pack containing a record beyond its effective cutoff is
  withheld, not sliced under the old immutable filename. The cursor advances and
  other eligible exchange continues. A structured aggregate `cutoff_boundary`
  diagnostic reports partial export; full convergence is not claimed for that
  profile. Already transmitted bytes are not claimed erased.
- Local restriction/retirement and whole-Brain holds retain the existing stop
  rules. This checkpoint does not itself guarantee dissemination of a newly
  created local self-retirement after its transport is disabled.

ReadyLocally continues to mean local transport readiness only. Neither copying a
pack nor Syncthing completion proves peer canonical publication or Protected.

## Required evidence

Unit/platform cases cover exact local sealing and subsequent append, idempotent
reopen, full-disk/interruption boundaries, conflicting destinations, hardlinks,
repopulation, bounded cursor progress, revoked admission and a genuinely signed
cutoff with another eligible peer. The signed Mac coordinator fixture must prove
pause, offline append, resume export and unchanged prior packs after shutdown.
The real-device package must still verify daemon canonical digests and derived
results after network delivery and restart; copying segment files between hosts
is not an acceptance substitute.
