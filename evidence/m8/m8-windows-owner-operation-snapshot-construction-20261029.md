# M8 Windows owner-operation snapshot construction

Date: 2026-10-29

## Scope

This slice builds immutable owner-visible request snapshots from a complete,
verified canonical trust scan while the inventory and per-device canonical
locks remain held. It does not confirm, sign, append, publish, or recover an
owner operation.

## Implemented

- The policy snapshot retains the `VerifiedTrustHistory` reconstructed from
  this installation's local approval anchors.
- A restriction request can target only a currently trusted, directly approved
  peer with a verified approval binding.
- The target's event signing key, public-key fingerprint, and local/peer
  transport association come from that verified binding.
- A requested restriction cutoff must not exceed the reduced current cutoff.
  A nonzero cutoff binds to the exact verified target-stream record and its
  approved signing key; zero uses the defined all-zero hash sentinel.
- A self-retirement request binds to the exact latest verified local sequence
  and content hash.
- Both request builders reject transaction IDs already used by canonical
  security events and validate the closed ADR-0019 approval snapshot digest.
- Verified peer binding reconstruction retains the local and peer Syncthing
  identities from the signed bootstrap transcript and rejects inconsistent
  local adapter associations across direct approvals.

## Verification

- `cargo test -p cryptexd --lib --bins`: library **515 passed, 0 failed, 4
  ignored**; binary **12 passed**.
- Focused local-only snapshot test in
  `knowledge_materialization::tests::daemon_rebuild_scans_verified_v1_frames_without_changing_canonical_bytes`:
  PASS. It validates the self-retirement request against the exact synthetic
  local chain tip and refuses an unapproved restriction target.
- `knowledge_materialization::tests::restriction_request_binds_approved_peer_and_exact_verified_cutoff`:
  PASS. It verifies a synthetic peer event, constructs a direct approval
  binding, and checks the successful restriction snapshot binds that key,
  adapter IDs, cutoff sequence/hash, and digest; an absent cutoff boundary is
  refused. This is a constructor test, not native UI or on-disk publication
  evidence.
- Focused roster test in
  `trust_reducer::tests::only_a_verified_local_peer_approval_anchor_builds_the_direct_roster`:
  PASS, including both verified transport IDs.
- `cargo clippy -p cryptexd --release --lib --bins -- -D warnings`: PASS.
- `cargo fmt --all -- --check`: PASS.
- `git diff --check`: PASS.

## Still open

- Native Windows owner confirmation UI and authenticated IPC capability flow.
- Operation-specific signing, locked-history revalidation at append, durable
  append/recovery reconciliation, and crash injection.
- Windows native filesystem/process-identity release gates, macOS parity, and
  independent security review.

No canonical writer or production route was enabled. No Actions, commit, push,
or merge was performed. M8 is not complete.
