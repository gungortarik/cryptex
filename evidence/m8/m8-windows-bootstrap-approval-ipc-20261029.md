# M8 Windows bootstrap approval IPC and Desktop review slice

Date: 2026-10-29
Platform: Windows development host
Scope: authenticated local review/signature release only

## Implemented

- `cryptexd` routes the closed `bootstrap.approval.*` wire envelope only after
  the existing authenticated Desktop process check. Requests have exact role,
  capability, request ID, operation shape, bounded frame size, and transcript
  verification.
- Prepare verifies the signed role-specific transcript prefix, checks the
  local Brain/device/event-key identity against existing runtime state, freezes
  the owner snapshot, records the verified prefix in the daemon journal, then
  issues a one-use connection-bound capability.
- Confirm and cancel must use the same IPC connection, request ID, role,
  transcript, and approval digest. Confirm consumes the capability before
  loading the matching existing signing key, journals the signed message, and
  only then returns it. Cancel persists the terminal cancellation without
  signing. Disconnect and timeout invalidate the in-memory capability.
- Desktop retains the connection and bearer capability in process memory; the
  renderer receives only an opaque local handle and the frozen display
  snapshot. The panel displays the Brain, transaction, both DeviceIds, full
  SHA-256 public-key fingerprints, transport association, role, and operation
  before explicit Approve or Cancel.
- The UI accepts compact signed messages pasted one per line. It does not
  exchange messages with Syncthing or the other device. Transport IDs are
  transcript association labels only; the operator must compare full values
  over a separate trusted channel.

## Verification

- `cargo test -p cryptexd --lib`: **485 passed, 0 failed, 4 ignored**
- Focused `bootstrap_approval_ipc` tests: **4 passed**
- Synthetic IPC round trip: verified frozen snapshot, explicit confirm,
  journal-before-release, and the returned signature as a valid initiator
  approval prefix
- `cargo check -p cryptex-desktop --bin cryptex-desktop`: **PASS**
- `pnpm --dir apps/desktop run build`: **PASS**
- `cargo clippy -p cryptexd --lib --tests --release -- -D warnings`: **PASS**
- `cargo clippy -p cryptex-desktop --bin cryptex-desktop --release -- -D warnings`: **PASS**
- `cargo fmt --all -- --check`: **PASS**
- `git diff --check`: **PASS**
- `pnpm --dir apps/desktop run test:search-policy`: **2/2 pass**
- `pnpm --dir apps/desktop run test:capture-policy`: **16/16 pass**
- `pnpm --dir apps/desktop run build`: **PASS**

## Native launch gate

- The locally packaged Windows pilot was prepared under an isolated
  `%LOCALAPPDATA%\Cryptex-Dev\m8-approval-win-*` root with a marker-only
  runtime. Launch was blocked by Windows Code Integrity event **3077** because
  `cryptex-desktop.exe` does not meet the machine's Enterprise signing-level
  requirements. No process started, and no policy change or bypass was made.
- Therefore packaged native UI and owner-dialog interaction remain
  **UNVERIFIED** on this host.

## Not implemented or claimed

- No native interactive Windows owner-approval session was run.
- This UI does not create a new Brain or join an existing Brain; it requires an
  already enrolled local identity whose existing event key matches the signed
  transcript.
- No canonical `security.peer_approved` event is appended. A returned signed
  message does not activate trust, create a trusted roster entry, authorize
  retrieval/import, or enable transport.
- No Syncthing message exchange, live peer/transport identity binding, startup
  journal reconciliation, append idempotency/locking, importer publication,
  macOS parity, independent final review, or M8 completion is claimed.
