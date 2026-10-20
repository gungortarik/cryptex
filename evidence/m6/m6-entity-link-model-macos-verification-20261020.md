# M6 entity-link model — macOS local verification

**Date:** 2026-10-20
**Host:** macOS arm64
**Branch:** `codex/m5-capture-timeline`
**Verified HEAD:** `74bc3b14127962579ae03838bafea9336612a033`
**Status:** Focused macOS checks PASS (matching Windows verification in `m6-entity-link-model-windows-verification-20261014.md`).

## Checks

| Check | Result |
|---|---:|
| `cargo fmt --all -- --check` | PASS |
| `cargo test --offline --locked -p cryptex-projection` | **64/64 PASS** |
| `cargo test --offline --locked -p cryptexd vault_view_uses_only_representative_entities_and_reduced_relations` | **1/1 PASS** |
| `cargo test --offline --locked -p cryptexd guarded_vault_rebuild_publishes_verified_history_and_holds_stale_history` | **1/1 PASS** |
| `cargo test --offline --locked -p cryptex-desktop vault_open_target_accepts_only_existing_generated_note_paths` | **1/1 PASS** |
| `cargo clippy --offline --locked -p cryptex-projection -p cryptexd -p cryptex-desktop --all-targets -- -D warnings` | PASS |
