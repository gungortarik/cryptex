# M8 bootstrap approval-prefix verification and signer primitive

Date: 2026-10-29
Scope: signed bootstrap prefix verification and message construction only
Platform checked: Windows development host

## Implemented

- Role-specific prefix verification accepts exactly `offer + proof` for an
  initiator approval and `offer + proof + initiator approval` for a responder
  approval.
- Every included message is parsed with the bounded closed-schema parser and
  signature-verified before deriving the approval context.
- The derived private context binds Brain, transaction, local and peer full
  key identities, transport associations, agreement digest, and the exact
  role-specific preceding message hash.
- `cryptexd` derives the exact `ApprovalSnapshot` from that context and matches
  every field against it during the owner-confirmation attempt.
- Approval signing checks that the provided private key matches both the
  verified local public key and its signing-key ID, then emits the exact
  role-specific wire message.
- The daemon session method consumes the connection capability before
  signing; request mismatch and signing failure do not restore it.
- Tests recreate both signed approval messages byte-for-byte from the complete
  transcript fixture, verify the completed transcript, and reject altered
  prefixes and mismatched local keys.

## Verification

- `cargo test -p cryptex-events --lib`: **79/79 passed**
- `cargo test -p cryptexd approval_session::tests --lib`: **7/7 passed**
- `cargo test -p cryptexd --lib`: **474 passed, 0 failed, 4 ignored**
- `cargo clippy -p cryptex-events --lib --tests --release -- -D warnings`: **PASS**
- `cargo fmt --check`: **PASS**
- `git diff --check`: **PASS**

## Not implemented or claimed

- Product IPC does not invoke the signer. The session-to-signer method is
  covered only by daemon unit tests.
- No Desktop UI displays the frozen approval snapshot or obtains confirmation.
- No signing key is loaded or used by a product approval operation.
- No bootstrap-journal transition, canonical security event append, peer trust,
  transport activation, or M8 completion is claimed.

The daemon must first authenticate the IPC peer, issue a snapshot-bound
connection capability, render that immutable snapshot, and consume the
capability before invoking this signer. The signer alone does not prove human
approval.
