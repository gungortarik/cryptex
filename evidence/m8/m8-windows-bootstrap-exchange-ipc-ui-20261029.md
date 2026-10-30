# M8 Windows bootstrap exchange IPC and UI slice

Date: 2026-10-29
Branch/HEAD at start: `codex/m8-device-trust` / `ae61603b517f68b63d559ccb5ee77d744176e6a6`

## Implemented in the existing dirty worktree

- Authenticated Desktop IPC routes create or resume signed bootstrap offers
  and verify/persist received offers before creating or releasing responder
  proofs.
- Offer transaction IDs are generated with the OS CSPRNG before sending the
  request. Desktop keeps the ID for exact retries when the response is
  interrupted.
- Offer/proof bytes are returned only after the daemon journal update succeeds.
- Tauri commands and a copy/paste exchange UI connect the message builders to
  the existing owner-review/signing flow. The UI states that transport and sync
  are not started.
- The initiating installation can return its exact canonical public Brain
  identity JSON. A responder can explicitly join only when the signed offer's
  Brain ID matches that identity, local identity metadata does not conflict,
  and no unbound canonical/device state exists. New DeviceId/key enrollment
  is idempotent and does not activate trust.
- Wrong-Brain proof requests fail before adding an exchange journal entry.

## Verification run

- `cargo fmt --all` — PASS
- `cargo check -p cryptexd --lib` — PASS
- `cargo test -p cryptexd --lib ipc::tests::bootstrap_ -- --nocapture` — **10/10 PASS**
- `cargo test -p cryptexd --lib` — **504 passed / 0 failed / 4 ignored**
- `cargo test -p cryptex-events` — **84/84 PASS**, doc test **1/1 PASS**
- `cargo clippy -p cryptexd --lib -- -D warnings` — PASS
- `cargo clippy -p cryptex-desktop --bin cryptex-desktop -- -D warnings` — PASS
- `cargo check -p cryptex-desktop --bin cryptex-desktop` — PASS
- `pnpm --dir apps/desktop run build` — PASS
- `git diff --check` — PASS

The initiator UI displays and accepts a saved transaction ID, so the owner can
retry/resume the same durable offer after restarting Desktop. The responder can
paste the same signed offer again to recover the exact journaled proof.

## Boundaries and remaining gates

This proves a synthetic IPC/UI slice, including explicit fresh-device
`join-existing-brain` setup from a separately transferred exact public
identity file. The transport DeviceId is currently supplied by the owner; the
daemon does not yet source it from the configured adapter. No packaged
Windows/native UI run was performed for this slice. No transport, peer pairing,
sync, canonical publication, Actions, commit, or push was run.

M8 remains open for adapter-bound transport identity, macOS peer
identity/parity, transport-to-staging wiring, canonical publication
replay/durability, native packaged review, and independent security review.
