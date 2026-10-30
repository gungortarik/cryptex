# M8 Windows R1 approval-session capability foundation

Date: 2026-10-29
Scope: ephemeral single-use capability primitive only
Platform checked: Windows development host

## Implemented

- A process-local registry issues random 256-bit capabilities against the exact
  frozen `ApprovalSnapshot` digest.
- Admission snapshots can be constructed only from an authenticated
  `VerifiedBootstrapApprovalRequest`; the renderer does not supply identity or
  transport fields.
- Capabilities are bound to an opaque per-connection identity supplied by the
  future authenticated IPC adapter. The module does not authenticate a peer.
- Confirmation checks the connection, consumes the capability under one
  mutex, then checks every verified request binding before invoking the
  bootstrap signer. Snapshot mismatch or signing failure burns the grant.
- Cancellation, exact-connection disconnect invalidation, and process restart
  prevent later use. Wrong-connection, modified-digest, and replay attempts
  fail closed.
- Sessions expire after five minutes of daemon monotonic time. Expired entries
  are removed on use and purged before capacity checks.
- Verified initiator and responder approval prefixes can be recorded in the
  journal before signature generation. Identical byte replay is idempotent;
  changed bytes for the same transaction conflict.
- The journaled confirmation helper consumes a matching grant, calls the
  signer, persists the signed approval/transcript before returning the bytes,
  and burns the grant if signing or persistence fails.
- The number of active sessions is bounded globally and per connection.
- A read-only event signing-key loader is available for a future gated daemon
  signer path; it opens the existing lock without creation and refuses missing
  state. It does not initialize or repair keys.
- The bearer capability is redacted from `Debug` output and is never persisted
  by this module.

## Verification

- `cargo test -p cryptexd approval_session::tests --lib`: **12/12 passed**
- Read-only key loader tests: **2/2 passed**
- `cargo test -p cryptexd --lib`: **481 passed, 0 failed, 4 ignored**
- `cargo clippy -p cryptexd --lib --tests --release -- -D warnings`: **PASS**
- `cargo fmt --check`: **PASS**
- `git diff --check`: **PASS**

The journal-prefix regression exercises the durable store API: it reloads the
persisted state, confirms exact replay is stable, and confirms a changed prefix
is rejected without altering the saved transaction.

## Not implemented or claimed

- IPC stream handling that keeps the same authenticated connection open across
  display and confirmation.
- OS peer-credential authentication in the approval workflow.
- Desktop owner-confirmation UI and frozen-snapshot rendering.
- Production IPC integration/authentication, Desktop confirmation UI,
  canonical append, or startup reconciliation.
- The signing key is not yet used by the approval-session primitive or any IPC
  operation.
- macOS parity, independent security review, or M8 completion.

The primitive is not reachable from product IPC and cannot currently authorize
an operation. The next Windows slice must bind its connection identity only
after validating the OS peer credential, keep that connection open through
display/confirmation, invalidate grants on disconnect, and keep existing
content/capture protocol behavior unchanged.
