# M8 Windows bootstrap adapter identity binding

**Scope:** replace owner-entered transport DeviceId with the identity exposed by
the configured, daemon-owned Syncthing adapter. No sync, incoming transport, or
canonical publication was enabled.

## Change

- The owned adapter's authenticated `system/status` observation is matched
  against the validated private profile receipt before the worker publishes its
  DeviceId.
- The read-only Desktop status IPC exposes that DeviceId only while the worker
  is active, not stopping, and at `LocalReady`; the returned value is shape
  checked and contains no credential or filesystem path.
- Bootstrap UI no longer accepts a manually entered transport DeviceId. It
  reads the active adapter identity before creating an offer or proof.
- The daemon independently rechecks the supplied ID against the current active
  worker at offer/proof creation. Missing or changed adapter identity fails
  closed.
- Today the worker can only be started through the development `lifecycle hold`
  harness. Normal Desktop startup does not yet start/own it; the UI correctly
  reports the adapter identity unavailable in that state.
- Unit exchange fixtures inject a synthetic adapter identity into a test-only
  context field; the same equality/shape gate rejects missing and mismatched
  identities. The worker readiness gate is tested separately.
- The ignored Windows native provision/reopen/collision test now also checks
  that the live worker identity equals the authenticated profile/REST identity
  and becomes unavailable after worker shutdown. It was attempted without the
  required `CRYPTEX_M8_NATIVE_FIXTURE`; it exited at the explicit fixture
  precondition before setup, so it is not native PASS evidence.

## Verification

- `cargo test -p cryptexd --lib --quiet`: **506 passed, 0 failed, 4 ignored**.
- `cargo check -p cryptexd --lib --target x86_64-pc-windows-msvc`: **PASS**.
- `cargo clippy -p cryptexd --lib -- -D warnings`: **PASS**.
- `cargo clippy -p cryptex-desktop --bin cryptex-desktop -- -D warnings`: **PASS**.
- `pnpm --dir apps/desktop run build`: **PASS**.
- `cargo fmt --all --check`: **PASS**.
- `git diff --check`: **PASS**.

## Limits / next gates

No Windows packaged/native run was performed in this change. Production
Desktop-to-worker lifecycle wiring and transport peer/folder activation are
still absent. macOS adapter
identity parity, inbound Syncthing-to-staging wiring, canonical publication and
replay durability, and independent security review remain open. This is not
M8 completion.
