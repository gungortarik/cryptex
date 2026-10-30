# M8 Windows strict v3 security-event reader — 2026-10-29

## Scope

Owner accepted ADR-0019 §§3–4 as the Windows implementation basis. This slice
adds an isolated parser/authenticator for the proposed v3 `security.*` event
records plus an explicit cross-version chain-verifier entry point. It does not
accept the full addendum or enable trust, transport, retrieval, or publication.

## Implemented

- Closed v3 envelope and per-event payload shapes; unknown fields and event
  types are rejected.
- Record byte, JSON nesting, container, and string limits; duplicate JSON keys
  are rejected.
- Canonical lowercase UUID/transaction/key/hash encodings and canonical decimal
  sequence fields; security classification must be `internal`.
- Device-restriction target public-key bytes must decode as an Ed25519 key and
  derive the declared signing-key ID.
- v3 JCS, content hash, signature domain, and Ed25519 signature are verified.
- `VersionedChainVerifier::verify_security_v3_next` checks the device binding,
  increasing sequence, zero genesis link, and predecessor hash for explicitly
  parsed v3 records.
- Default `read_versioned_record` continues to return v3 as `Unverifiable`, so
  existing readers remain fail-closed.

## Verification

- `cargo test -p cryptex-events security_v3 --lib`: **10/10 passed**.
- `cargo test -p cryptex-events --lib`: **71/71 passed**.
- `cargo clippy -p cryptex-events --lib --tests --release -- -D warnings`:
  **PASS**.
- `cargo fmt --all -- --check`: **PASS**.
- `git diff --check`: **PASS**.

## Not established

This parser does not verify the four signed bootstrap messages or their
agreement/transcript hashes. It does not resolve a key against the local
direct-approval roster, prove complete history, detect cross-record forks or
duplicate transaction IDs, bind cutoffs to exact target-chain events, feed
`RetrievalSafetyState`, or authorize cleanup/publication. `peer_approved` only
checks that the transcript is an array of four objects; it is not trusted
without the separate bootstrap verifier. Independent review and macOS native
gates remain open. No Actions, commit, push, or activation.
