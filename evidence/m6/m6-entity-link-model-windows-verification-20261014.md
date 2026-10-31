# M6 entity-link model — Windows local verification

**Date:** 2026-10-14
**Host:** Windows 11 x64
**Branch:** `codex/m5-capture-timeline`
**Base HEAD:** `555698c` (M6 product changes were in the working tree during verification)
**Status:** Focused Windows checks PASS; macOS verification remains required.

## Delivered behavior

- One entity note per representative UUID at `Cryptex/Entities/<uuid>.md`.
- Current representative labels are presentation data, not path identity.
- Facts render as escaped typed content. Only explicit reduced M4 relations create outgoing WikiLinks; no reverse relation is inferred.
- The daemon maps only representative entities, active facts, and active relations from the reduced `KnowledgeState` and uses the existing shared `RetrievalSafetyState` gate. This change does not bypass or weaken that gate.
- Desktop accepts generated entity-note targets while retaining UUID, regular-file, and symlink-parent checks.

## Checks

| Check | Result |
|---|---:|
| `cargo fmt --all -- --check` | PASS |
| `cargo test --offline --locked -p cryptex-projection` | **64/64 PASS** |
| `cargo test --offline --locked -p cryptexd vault_view_uses_only_representative_entities_and_reduced_relations` | **1/1 PASS** |
| `cargo test --offline --locked -p cryptexd guarded_vault_rebuild_publishes_verified_history_and_holds_stale_history` | **1/1 PASS** |
| Desktop `vault_open_target_accepts_only_existing_generated_note_paths` | **1/1 PASS** |
| Strict Clippy for projection + daemon, all targets | PASS |
| Strict Clippy for Desktop, all targets | PASS |

Projection tests cover stable UUID paths, duplicate labels, explicit one-way links, label changes, escaped Markdown data, unresolved endpoints, deterministic rebuild, and manifest ownership. The guarded daemon test confirms stale canonical history leaves the existing vault unchanged.

## Remaining gates

- Run the same focused checks on macOS against this exact source commit after it is pushed.
- No Actions run, merge, M7 write-back, or M9 blob backend is included.
- This evidence does not claim M6 completion or native entity-graph verification.
