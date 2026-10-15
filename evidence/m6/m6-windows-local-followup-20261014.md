# M6 Windows local follow-up

**Date:** 2026-10-14  
**Host:** Windows 11 x64  
**Branch / HEAD:** `codex/m5-capture-timeline` / `87e9c4b0a49c5b670bdd7c6812d5a7c3fa768219`  
**Checkout:** M6 implementation remains in the uncommitted working tree; the HEAD alone does not contain the tested changes.

## Checks on the current working tree

| Check | Result |
|---|---|
| `cargo fmt --all` and `cargo fmt --all -- --check` | PASS |
| `cargo test --offline --locked -p cryptex-projection` | **59/59 PASS** |
| Desktop `vault_uri_tests` | **2/2 PASS** |
| Daemon `vault_destination_guard` | **1/1 PASS** |
| Daemon `vault_rebuild` | **2/2 PASS** |
| Desktop `cargo check --offline --locked --bin cryptex-desktop` | PASS |
| `cargo clippy --offline --locked -p cryptexd --all-targets -- -D warnings` | PASS |
| Desktop `cargo clippy --offline --locked --bin cryptex-desktop -- -D warnings` | PASS |
| `git diff --check` | PASS |

This follow-up covers the cross-platform path-comparison fix and Desktop generated-note target validation, including rejection of symlinked generated directories. The symlink regression is Unix-only and was not executed on Windows; owner-reported macOS automated verification includes the Unix regression, and the rebuilt Mac app passed a fresh isolated synthetic-vault native check. See `m6-macos-native-followup-20261014.md`.

No Actions, commit, push, merge, or Windows pilot launch was performed in this follow-up. M6 remains open: accepted retrieval-safety scope for v2 relationships, Mac cross-volume attachment evidence (or an explicit unavailable record), CI conflict with the owner's no-Actions instruction, and final human review remain unresolved.
