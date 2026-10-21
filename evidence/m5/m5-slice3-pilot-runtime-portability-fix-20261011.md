# M5 Slice 3 -- pilot runtime portability fix (packaged Desktop)

**Date:** 2026-10-11
**Branch:** `codex/m5-capture-timeline`
**Product / source SHA:** `eba337fcf2e81da8163254ff2092690f2a695ce3`
**Required product ancestor:** `b9b3e4cdfdc22077c5dd5ec78b0dbbf22969863f` (still an ancestor)

## Bug

Packaged `cryptex-desktop.exe` rejected isolated pilots on the Windows VM with
`PILOT_RUNTIME_REJECTED: Io` because `validate()` unconditionally canonicalized
`env!("CARGO_MANIFEST_DIR")/../../..` from the build host. That path is absent
on the destination machine.

## Fix

In `apps/desktop/src-tauri/src/pilot_runtime.rs`:

- Resolve the build-time repository hint portably: `NotFound` skips hint
  containment; permission/unexpected I/O still returns `PilotError::Io`.
- Reject runtimes under any Git checkout via ancestor `.git` detection so
  inside-checkout rejection still holds when the build tree is absent.
- Preserve absolute path, `ParentDir` ban, ordinary ancestor (symlink/reparse)
  checks, exact marker ownership, and fresh marker-only runtime.

Other Desktop startup paths do not embed compile-time `CARGO_MANIFEST_DIR`
containment. Packaged `cryptexd` already uses optional runtime
`CARGO_MANIFEST_DIR` in `paths::assert_outside_repository` and skips when unset.

## Verification (host)

| Check | Result |
|---|---|
| `cargo test pilot_runtime` | 8 passed |
| `cargo fmt` | PASS |
| `cargo clippy --locked --bin cryptex-desktop --release -- -D warnings` | PASS (debug rebuild blocked by OS Application Control 4551 on webview2 build-script; release profile used) |

## Staged package (for VM transfer)

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `cryptex-desktop.exe` | 11096064 | `2ed7be494b9ba8dd9d988e83e4c1bd80c930298f8477d45ddc81ec90190bfbf1` |
| `cryptexd.exe` | 4573696 | `780c9bd4b00823f25f695059432c3be8bbd78573ff9cd80ff94509c59ae8ada7` |
| zip | 5839709 | `1174ca0a0ecd381885ad5baaac953d8504d5586703be686081f0228804c5fd94` |

Stage: `%LOCALAPPDATA%\Cryptex-Dev\m5-s3-win-native\Cryptex-M5-S3-Windows-Native-eba337fcf2e8\`
Zip: `D:\VMs\Win11\Cryptex-M5-S3-Windows-Native-eba337fcf2e8.zip`
Transfer URL (host-only, one-time): see `D:\VMs\Win11\m5-s3-transfer-route.txt`

No Actions, merge, Slice 4, or M5-complete claim. Native Quit pilots remain to be run after guest refresh.
