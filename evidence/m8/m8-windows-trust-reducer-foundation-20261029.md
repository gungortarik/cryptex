# M8 Windows trust reducer foundation — 2026-10-29

## Owner decision and scope

The owner accepted ADR-0019 §§3–4's security-event and trust/cutoff reducer
model as the Windows implementation basis. This does not accept ADR-0019 as a
whole, waive independent security review, or activate trust, Syncthing
transport, retrieval, or canonical publication. The reducer is not yet fed by
the verified v3 event reader and is not wired to `RetrievalSafetyState`.

## Implementation

Added a pure deterministic reducer over a complete pre-verified policy-history
input. It adds direct local approvals plus the local device to the trusted set,
ignores restrictions authored by non-approved peers, applies self-retirement
caps, resolves acyclic restrictions in deterministic condensation order, and
returns a Brain-wide typed hold for missing/unverifiable history, competing
forks, invalid policy input, or unresolved restriction cycles. It has no file,
IPC, writer, transport, or retrieval side effects.

The input boundary is important: this reducer does not parse events, verify
signatures, validate cutoff boundary hashes, or establish that the supplied
history is complete. Those responsibilities remain with the not-yet-integrated
v3 reader and verified-history builder. Do not use this module as publication
authorization.

## Verification

- `cargo test -p cryptexd trust_reducer::tests --lib`: **9 passed**.
- `cargo test -p cryptex-core`: **30 passed**.
- `cargo clippy -p cryptexd --lib --tests --release -- -D warnings`: **PASS**.
- `cargo fmt --all -- --check`: **PASS**.
- `git diff --check`: **PASS**.

Reducer fixtures exercise the seven required SCC/cutoff cases in ADR-0019 §6,
plus incomplete/forked history and restrictions from unapproved peers. These
are synthetic algorithm tests, not native or independent security review.

## Still open

- Strict v3 security-event parser, cross-version chain verification, trusted
  key resolution, and cutoff boundary binding.
- Independent review of the reducer and golden vectors.
- `RetrievalSafetyState` integration and fail-closed propagation.
- Windows/macOS connection-bound approval IPC and native platform tests.
- Canonical publication/recovery, transport controller, pairing UX, and full
  M8 verification.

No trust/transport activation, Actions, commit, or push.
