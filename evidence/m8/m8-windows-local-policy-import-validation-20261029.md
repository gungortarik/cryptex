# M8 Windows local-policy import validation

## Scope

Adds daemon-side staged-import validation using the locally anchored M8 trust
history, plus an internal no-replace publication primitive that consumes its
M3/M4 lock-bearing proof token. The publisher is exercised only with synthetic
fixtures; managed transport does not call it, and peer trust activation or
Windows power-loss durability is not claimed.

## Behavior

- Scans the complete canonical device inventory while holding inventory and
  existing device writer locks.
- Verifies the local stream first and derives peer signing keys only from
  locally signed, transcript-verified approval anchors.
- Rejects candidate devices outside the reduced locally trusted set.
- Derives starting tips from the complete authenticated security history;
  approved devices with no prior stream start at genesis.
- Rejects a canonical starting tip or staged segment end above the device's
  effective cutoff.
- The import path consumes a successful M3/M4 `RetrievalSafetyState` and
  validates against that exact snapshot. It returns an unforgeable
  `ImportPolicyVerifiedBatch` that retains the inventory/device locks for the
  duration of the validation decision.
- For a newly approved first-stream device, the scanner checks local trust
  before creating its canonical directory, then reacquires the complete sorted
  device-lock set under the inventory lock and rescans. An unapproved device
  does not get a canonical directory.
- While the token owns the canonical locks, a read-only publication preflight
  rechecks staged identity/length/hash, scans for canonical file-ID aliases,
  and rejects an occupied sealed destination or matching `.open` path.
- The internal publisher performs same-volume no-replace moves, syncs and
  reads back each destination, then writes a canonical committed marker through
  a unique temp, file sync, no-replace placement, parent-sync attempt, and
  final read-back. The token is retained throughout.
- Startup classifies a marker companion where moved staging files may be absent
  only when the matching canonical destination exists. Recovery remains
  fail-closed and preserves the marker, prepared manifest, staging tree, and
  canonical files; it does not complete replay or cleanup.

## Tests

- `guarded_search_allows_only_current_verified_and_materialized_v1_projection`
  — verifies the local-only roster and lock-scoped local stream tip.
- `local_policy_batch_verification_rejects_unapproved_candidate_device` —
  verifies that a staged, cryptographically valid candidate from an unapproved
  device is rejected by the local-anchor policy path.
- `local_policy_batch_verification_accepts_contiguous_local_candidate` —
  verifies the positive path for a locally trusted device, including the
  canonical starting tip, sequence/hash continuity, signature, cutoff, M3/M4
  safety-state binding, lock retention against a competing writer, and
  publication-preflight rejection of a staged hard-link alias.
- `approved_first_stream_lock_creation_stays_under_inventory_guard` — checks
  that a first-stream canonical lock is created while inventory ownership is
  retained, so a competing writer cannot race the inventory snapshot.
- `xproc_publication_crashes_preserve_canonical_and_journal_evidence` —
  interrupts a synthetic Windows process after a canonical segment move, after
  committed-marker temp sync, and after final marker placement/read-back; each
  restart remains held and preserves evidence.

Windows host verification after adding the M3/M4-bound publisher and process
interruption coverage: `cargo test -p cryptexd` passed (490 library, 12 binary, 9 lifecycle, 3
supervisor, and 2 update tests; 4 library tests ignored). Strict release
Clippy, formatting, and `git diff --check` passed.

These are local Windows-host process-interruption results, not physical
power-loss evidence or a supported-filesystem durability guarantee. No packaged
Desktop interaction, production transport wiring, replay/cleanup, macOS parity,
independent final review, Actions, commit, or push is claimed here.
