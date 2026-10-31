# M6 Slice 2 — guarded vault rebuild and copy-only attachment projection

**Date:** 2026-10-13  
**Host:** Windows 11 x64  
**Status:** focused implementation checkpoint; M6 remains open

## Delivered

- Added an owned `Cryptex/` subtree under an explicitly selected existing vault directory. Rebuild stages output beside the destination, publishes by rename, and touches no user-owned files outside that subtree.
- Generated notes are deterministic `notes/<event-id>.md` files with escaped YAML frontmatter and the verified note body.
- A manifest and ownership marker allow rebuild to verify the existing generated tree. Edited, missing, untracked, symlinked, or invalid generated output fails closed.
- Added a daemon entrypoint that runs the existing Brain-wide `RetrievalSafetyState` gate, retains the canonical locks during publication, and only projects verified v1 records. Stale or unsupported history is withheld before the vault is touched.
- Added a stable synthetic attachment input boundary. Projected files are copied into the generated subtree, never hard-linked; source and staged-copy SHA-256 values are compared before publication. Names are sanitized and traversal/reserved names are rejected.

## Focused verification

| Check | Result |
|---|---:|
| `cargo test --offline --locked -p cryptex-projection` | **59/59 PASS** |
| `cargo test --offline --locked -p cryptexd --lib knowledge_materialization::` | **11/11 PASS** |
| `cargo fmt --all -- --check` | PASS |
| `git diff --check` | PASS |
| `cargo clippy --offline --locked -p cryptexd --lib -- -D warnings` | Not run: Windows Application Control blocked `cargo-clippy` (OS error 4551) |

Tests include deterministic rendering, YAML escaping, malformed-event rejection, user-file preservation, refusal of edited files and untracked empty directories, stale retrieval refusal without changing the prior vault, attachment copy independence, unsafe-name refusal, and copy-failure cleanup.

## Not complete

- No Tauri/daemon IPC or Desktop control yet; no native vault picker or Open Vault/Graph command.
- Synthetic fixtures do not connect to canonical attachment storage (M9).
- macOS verification of this implementation has not run.
- M6 completion gates, Actions, M7 write-back, and M9 blob storage remain out of scope for this checkpoint.
