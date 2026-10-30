# M8 Windows v1/v2/v3 trust-history bridge — 2026-10-29

## Scope and implementation

Owner accepted ADR-0019 §§3–4 as the Windows implementation basis. This slice
connects authenticated event records to deterministic trust/cutoff reducer
input without enabling retrieval, import publication, or transport.

- `verify_security_history` authenticates supported v1/v2/v3 records with a
  caller-supplied trusted-key resolver, deduplicates exact replica bytes,
  rejects conflicting event IDs and unsupported v2 semantics, sorts records by
  device ID bytes and sequence, and verifies each mixed-version device chain.
- `verify_security_history_cancellable` checks cancellation between record
  verification and chain advancement; cancellation never returns a partial
  result.
- `build_verified_trust_history` includes restrictions only from the local
  device or caller-supplied direct approvals. It binds each nonzero restriction
  cutoff to the exact target record's sequence, content hash, and signing-key
  ID. Zero cutoff requires the all-zero hash.
- Self-retirement must bind to the immediately preceding record in that
  device's verified stream, or to the zero genesis boundary, and no later
  record may follow it. Security transaction IDs must be unique per author
  stream.
- Peer-approval transcript contents are not trusted by this builder. The
  four-message bootstrap signature/agreement verifier and direct-approval
  roster derivation remain separate gates.

## Verification

- `cargo test -p cryptexd knowledge_materialization::tests --lib`: **12/12
  passed**, including a lock-scoped canonical scan whose security history is
  reduced to the local DeviceId with an empty direct-approval roster.
- The scan's current legacy v2-history verification still rejects v3 before
  the M8 reader can consume a v3 canonical stream.
- `cargo test -p cryptex-events --lib`: **74/74 passed**.
- `cargo test -p cryptexd trust_reducer::tests --lib`: **11/11 passed**.
- `cargo clippy -p cryptex-events --lib --tests --release -- -D warnings`:
  **PASS**.
- `cargo clippy -p cryptexd --lib --tests --release -- -D warnings`: **PASS**.
- `cargo check -p cryptexd --tests`: **PASS**.
- `cargo fmt --all -- --check`: **PASS**.
- `git diff --check`: **PASS**.

Synthetic tests include a reordered v1→v2→v3 stream, sequence competition,
exact target cutoff match/mismatch, terminal retirement, and cancellation.

## Not established

The verifier cannot prove that its supplied byte set is exhaustive; the
canonical scan now supplies its inventory/device-lock-scoped frame set. No
direct-approval roster is derived because bootstrap transcripts are not yet
verified. The retained history/reducer method is not called from retrieval or
import paths and does not feed `RetrievalSafetyState`, content filtering,
publication, or transport. The existing M4 reader continues to hold v3 as
unverifiable. Independent review and native macOS/Windows trust gates remain
open. No Actions, commit, push, or activation.
