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
- Development-host `pnpm --filter @cryptex/desktop tauri:package-smoke`:
  **passed**, unbundled Release Desktop and daemon sidecar. Host used Node
  26.13.0 / pnpm 11.28.0; pnpm warned that the repo pins Node 24.19.0.
- Full daemon library run: **519 passed / 2 failed / 4 ignored**. Both failures
  were the existing owned-process HTTP tests returning `PeerNotEstablished`
  on this development host. This fix did not weaken native TCP-state or
  process-ownership validation.

Second-VM build and owner-flow retest:

- Source `b733e436b0f9f69ded13699aa012fc6b2901650d`; Windows 26200.9711 / 25H2,
  standard user; Node 24.19.0, Cargo 1.98.1, pnpm 11.28.0.
- Initial package build failed at the long temp path with MASM
  `A1009: line too long`. Re-extraction under `%TEMP%\cx-b733e43` resolved it;
  package smoke passed and built an unbundled Release Desktop plus daemon.
- Desktop SHA-256:
  `2ADB6443E58A0B8CA893BB6EE17423311C8897A3D42E0B73C68189A295BAC6CD`
- Daemon SHA-256:
  `2B8431464AB8C94CE22AAA03D9C2F54A0E245D16DBC285DC0B7FA5525C556E1A`
- A fresh marked synthetic fixture returned Brain
  `18b2f76c-eae5-46ff-b7b9-8ac3f110ff46` and seed event
  `08fb17ca-f651-4e08-82c5-2ae718128062`. The corrected Desktop rendered
  `self_retirement`, transaction
  `8effe1659f080bb983c85445076ea395f4d390484e56717d16c0d3db19ae5c1e`, issuing
  device `102ca9af-4077-4de2-bc58-3bf436c9cf02`, signing key
  `d49ab5902c64736d82b654986afbdaac`, last-authorized sequence `1`, and hash
  `9f142876c5969a1f5bdbb41ec48c8b53dc8a8f9c441960a04aeb07a236f1db06`.
- Owner selected **Cancel**; the UI reported **“Operation cancelled. No
  canonical event was written.”** No confirmation was performed.

## Scope and next step

The rendering defect is fixed and the corrected Windows owner review plus
cancellation passed on the second VM. This does not cover confirmed append,
denial/retry, exact nonzero cutoff boundaries, recovery, a bundled installer,
or M8 completion. The approval session deadline is five minutes; 15 seconds is
only the Desktop-to-daemon connect timeout. The two pre-fix attempts were not
confirmed; their lost renderer handles mean their old synthetic journal
records may remain `OwnerPending`. That profile was not reused; the corrected
test used a fresh marked runtime. No canonical trust event was written by the
cancelled operation.
