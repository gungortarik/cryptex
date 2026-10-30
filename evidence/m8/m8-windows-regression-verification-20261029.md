# M8 Windows regression verification

**Date:** 2026-10-29
**Branch:** `codex/m8-device-trust`
**Source HEAD:** `ae61603b517f68b63d559ccb5ee77d744176e6a6` (working tree changes uncommitted)

## Verification

- `cargo test -p cryptexd --quiet -- --test-threads=1` — PASS:
  - library: **507 passed, 0 failed, 4 ignored**
  - binary: **12 passed**
  - lifecycle: **9 passed**
  - supervisor: **3 passed**
  - rollback: **2 passed**
- `cargo test --workspace --exclude cryptex-desktop --lib --quiet -- --test-threads=1`
  — PASS: **764 passed, 0 failed, 4 ignored** across workspace libraries.
- `cargo clippy -p cryptexd --all-targets --release -- -D warnings` — PASS.
- `cargo fmt --all -- --check` — PASS.
- `git diff --check` — PASS.
- `pnpm --dir apps/desktop build` — PASS.
- `pnpm --dir apps/desktop test:capture-policy` — **16/16 PASS**.
- `pnpm --dir apps/desktop test:search-policy` — **2/2 PASS**.
- `cargo check --locked -p cryptex-desktop` — PASS.
- `pnpm --dir apps/desktop run tauri:package-smoke` — PASS; Windows release
  Desktop and daemon sidecar built without creating an installer.

Build outputs:

- `target/release/cryptex-desktop.exe` SHA-256:
  `a70a27bdaf378124f74ab152d903ff5a0552842d149c0a463b1637607fee2920`
- `apps/desktop/src-tauri/binaries/cryptexd-x86_64-pc-windows-msvc.exe`
  SHA-256: `fa247194a04b3bf4497cdac03d13116d7610ab801c53cf183ee6fb3321a8a28c`

The initial package run encountered a global Windows IPC endpoint collision:
the synthetic `m8-win-native` pilot was already holding the named pipe. Its
runtime and process identity were checked, then its own stop file was written.
The pilot recorded `instance=stale` and `note=clean shutdown released instance
lock`. The full package run was repeated serially and passed, including all
nine lifecycle tests.

## Pinned Windows native fixtures

The pinned Syncthing **2.1.5** Windows amd64 release archive was obtained from
the official [Syncthing v2.1.5 release](https://github.com/syncthing/syncthing/releases/tag/v2.1.5).
Before execution, `syncthing.exe` was checked against the repository's pinned
SHA-256: `36a0f7bc372f64fa7cc4f5654fa324c0dd9f7fef2e07565e00c6e1cf73f50344`
(27,448,104 bytes). A fresh, user-only-ACL test fixture was created outside
the repository at:

`%LOCALAPPDATA%\Cryptex-Dev\m8-verification\m8-owned-native-c71ec7c9485b43d48b013d396ee92363`

Fixture preparation launched only Syncthing's local `generate` role; the
prepared config was loopback-only and contained no active folder or peer. The
four explicit native tests passed serially:

- `native_syncthing_lifecycle_fixture` — PASS; two clean start/stop runs,
  unauthorized REST requests rejected with HTTP 403.
- `native_syncthing_private_profile_and_os_credential_fixture` — PASS; two
  clean runs using Windows Credential Manager; synthetic credential deleted.
- `pinned_package_owned_generation_fixture` — PASS; package pinning,
  tampering rejection, and owned-generation cleanup checks passed.
- `real_pinned_profile_provision_reopen_and_collision_recovery` — PASS;
  native provisioning, loopback endpoint collision refusal, worker lifecycle,
  identity preservation, and instance-lock release checks passed.

The fixture's `native-result.json`, `native-private-profile-result.json`,
`native-pinned-package-result.json`, and `native-provision-result.json` record
detailed outcomes under that AppData proof root. No personal Syncthing profile,
peer pairing, remote transport, canonical import/publication, or personal vault
was used. Native processes exited cleanly; no `syncthing` or `cryptexd`
process remained after verification.

## Limits

This verifies Rust host tests and isolated native fixtures on Windows. It is
not packaged owner-dialog evidence, a live peer-transfer test,
publication/recovery activation, macOS parity, independent security review, or
M8 completion. No peer transport was started. No Actions, commit, or push.
