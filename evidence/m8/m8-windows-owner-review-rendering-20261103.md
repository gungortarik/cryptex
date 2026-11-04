# M8 Windows Owner Review Rendering — 2026-11-03

## Environment and action

- Second Windows VM, build `26200.9711` / 25H2, standard-user session.
- Source build before the fix: the published `a5f859a` Windows package-smoke
  source.
- Runtime: marked synthetic profile outside the repository at
  `%TEMP%\cryptex-m8-owner-638f41528100470eb48ccac291b9b4ce`.
- Desktop process `4796`, daemon process `7588` during the reported attempt.
- Owner selected **Retire this device** and clicked **Prepare owner review**.

## Observed failure

The Desktop WebView became entirely blank after preparation. The owner repeated
the action and observed the same result. Both processes stayed alive. The
synthetic runtime's `gui-proof.snapshot` reported `window_visible=true`, the
expected `runtime_root`, and a tray-restore note. No `start.error`,
`window.error`, or `restart.error` file was present. `Ctrl+R` restored the
initial form, losing the renderer's prepared-session handle. The owner did not
click **Confirm security change**.

## Root cause and fix

`ApprovalSnapshot`, `ApprovalIdentity`, and `TransportAssociation` are durable
journal structures whose Serde field names are snake_case. The Tauri command
returned those structures directly, but the React review renders camelCase
fields such as `snapshot.localIdentity.deviceId`. When preparation succeeded,
the component dereferenced the missing `localIdentity` field and the WebView
render failed.

The candidate adds dedicated camelCase Desktop DTOs and converts the durable
snapshot only at the Tauri IPC boundary. It does not alter persistent journal
serialization. A regression test checks camelCase at the Desktop boundary and
snake_case for the journal structure.

## Verification

- Focused Rust serialization regression: **1 passed**.
- Frontend TypeScript/Vite production build: **passed**.
- `cargo check --locked -p cryptex-desktop`: **passed**.
- `cargo clippy --locked -p cryptexd --lib -- -D warnings`: **passed**.
- `cargo fmt --all -- --check` and `git diff --check`: **passed**.
- `pnpm --filter @cryptex/desktop tauri:package-smoke`: **passed**, unbundled
  Release Desktop and daemon sidecar. Host used Node 26.13.0 / pnpm 11.28.0;
  package manager warned that the repo pins Node 24.19.0. Verify again with the
  pinned Node on the second VM.
- Full daemon library run: **519 passed / 2 failed / 4 ignored**. Both failures
  were the existing owned-process HTTP tests returning `PeerNotEstablished`
  on this development host. This fix did not weaken native TCP-state or
  process-ownership validation.

Build hashes:

- Desktop: `B9AFA1D304B8D99567237AFDE1AE822F35FB2411F43ADAA46FD397FEF220FEBC`
- Daemon sidecar: `DFE5C16E5206DCDAF0BB675D5EA7A7ED2A9057844A30F8CE64A32FBCB4462CAA`

## Scope and next step

This is a confirmed UI rendering defect and a locally verified code fix, not a
post-fix native Windows owner-flow pass. The approval session deadline is five
minutes; 15 seconds is only the Desktop-to-daemon connect timeout. The two
pre-fix attempts were not confirmed; because the WebView reload discarded the
session handles, their synthetic journal records may remain `OwnerPending`.
Use a fresh marked synthetic runtime when retesting the published fix. Verify
that the exact self-retirement snapshot appears, then choose **Cancel**. Do not
claim the owner flow passed until the review and cancellation are observed.
