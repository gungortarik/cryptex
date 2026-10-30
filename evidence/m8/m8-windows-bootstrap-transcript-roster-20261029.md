# M8 Windows bootstrap transcript and local roster foundation — 2026-10-29

## Owner decision and boundary

The owner accepted ADR-0019 §2 bootstrap/onboarding and §§3–4 security-event
and trust/cutoff rules as the Windows implementation basis. ADR-0019 remains
PROPOSED overall. This does not waive independent protocol/security review and
does not activate peer trust, transport, retrieval, or publication.

## Implemented in this slice

- `VerifiedBootstrapTranscript` fields are private; external callers can only
  obtain the proof type through transcript verification. Read-only accessors
  expose verified fields, with a compile-fail doctest preventing external
  struct construction.
- Added a bounded JSON parser for bootstrap messages: 64 KiB raw message,
  incremental decoded-string byte bounds, closed root/payload fields, ASCII
  keys, decoded duplicate-key rejection, bounded depth/container counts, and
  rejection of noncanonical number forms.
- Added verification for the exact four-message offer/proof/bilateral-approval
  transcript: Ed25519 signatures, JCS/domain hashes, role/order/previous-hash,
  Brain and transaction binding, key-ID/public-key binding, and exact transport
  associations.
- A verified local `security.peer_approved` event can derive the direct peer
  key, fingerprint, and transport binding. Imported peer approval events do
  not grant local trust.
- Added a read-only verifier helper that selects the local stream from the
  supplied complete record set, authenticates it using only the local key,
  derives peer bindings from those local anchors, then verifies the complete
  set with a resolver restricted to the local key and those bindings. An
  unapproved device stream has no resolver key and fails closed.
- Added a separate lock-scoped canonical policy scan that holds the inventory
  and all device locks through scanning and verification, checks inventory
  completeness before and after locking/scanning, and rejects a device
  directory outside the derived trusted set. It is intentionally not called
  from retrieval/import authorization.
- Policy restrictions are evaluated after the entire local approval roster is
  derived, so authorization does not depend on cross-device sort order. Added
  a regression case where the approved peer sorts before the local device.
- Updated the handoff and ADR owner-decision wording to record §§2–4 as the
  implementation basis, while preserving the full-ADR and activation gates.

## Verification

- `cargo test -p cryptex-events --doc`: **1/1 passed**, including the
  compile-fail check that external code cannot construct a verified transcript.
- `cargo test -p cryptex-events --lib`: **79/79 passed**.
- `cargo test -p cryptexd --lib bootstrap_journal::tests`: **9/9 passed**.
- `cargo test -p cryptexd --lib trust_reducer::tests`: **13/13 passed**.
- `cargo test -p cryptexd --lib knowledge_materialization::tests`: **12/12
  passed**, including a lock-scoped canonical policy scan over a synthetic
  local-only stream.
- `cargo clippy -p cryptex-events --lib --tests --release -- -D warnings`:
  **PASS**.
- `cargo clippy -p cryptexd --lib --tests --release -- -D warnings`: **PASS**.
- `cargo check -p cryptexd --tests`: **PASS**.
- `cargo fmt --all -- --check`: **PASS**.
- `git diff --check`: **PASS**.

## Not implemented or established

The transcript verifier/helper are not wired to an owner-confirmed R1
immutable approval session, durable pending journal, native local OS-key
source. The scanner accepts a caller-supplied local public key and is not yet
called by retrieval/import authorization. No bootstrap event writer, peer
transport activation, `RetrievalSafetyState` integration, content filtering,
canonical publication/recovery, independent review, native Mac parity, or M8
completion is claimed.

No Actions, commit, or push.
