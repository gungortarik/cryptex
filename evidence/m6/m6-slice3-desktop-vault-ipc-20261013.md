# M6 Slice 3 — Desktop vault selection and daemon IPC

**Date:** 2026-10-13  
**Host:** Windows 11 x64  
**Status:** Implementation and focused Windows checks complete. Packaged Windows native vault refresh and generated-note opening PASS on 2026-10-13. macOS verification and remaining M6 DoD gates remain open.

## Scope delivered

- Added exact Desktop-only `content.vault` capability and `content.vault.rebuild` operation.
- Added a single bounded vault-rebuild worker and five-minute request/response deadline.
- Daemon accepts only a bounded, explicit absolute destination that is an existing real directory. It rejects a symlink destination, the source repository, and the runtime tree. It does not echo or log the selected path.
- Before publication, daemon catches up derived state and reruns the retrieval-safety gate. The verified canonical locks remain held through the generated `Cryptex/` subtree rebuild.
- Desktop uses a native folder picker, invokes only the daemon operation, and requests Obsidian to open the selected vault through `obsidian://open?path=...`. The user can then use Obsidian Graph; Cryptex does not implement a separate graph engine.
- Native pilot follow-up found Obsidian rejects the vault-directory root as a `path` target (`Vault not found`). The URI contract expects a file path within a registered vault. Desktop now receives the first deterministic generated note path from the daemon and opens only that UUID-named generated Markdown file; the Tauri side validates the relative path and regular-file target before forming the URI. Empty vaults have no note target and therefore are not falsely reported as opened.
- No renderer filesystem access, canonical write path, M7 write-back, or M9 blob backend was added.
- Fixed the startup catch-up IPC test's Windows cleanup race by waiting for the worker-held SQLite handle to close; assertions and behavior were not relaxed.

## Focused verification

| Check | Result |
|---|---:|
| `cargo fmt --all -- --check` | PASS |
| `cargo test --offline --locked -p cryptexd --lib ipc::tests:: -- --test-threads=1` | **42/42 PASS** |
| `cargo test --offline --locked -p cryptex-projection` | **59/59 PASS** (after note-target fix) |
| `cargo test --offline --locked -p cryptexd --lib ipc::tests:: -- --test-threads=1` | **42/42 PASS** (after note-target fix) |
| `cargo test --offline --locked -p cryptex-desktop --bin cryptex-desktop vault_uri_tests` | **2/2 PASS** (after note-target fix) |
| `cargo check --offline --locked -p cryptexd --lib` | PASS |
| `cargo check --offline --locked -p cryptex-desktop --bin cryptex-desktop` | PASS |
| `pnpm --dir apps/desktop build` | PASS |
| `pnpm --dir apps/desktop tauri:package-smoke` | PASS — built `target/release/cryptex-desktop.exe` with the release daemon sidecar; no bundle was produced |
| `git diff --check` | PASS |
| `cargo clippy --offline --locked -p cryptexd --lib -- -D warnings` | **Not run** — Windows Application Control blocked `cargo-clippy` (OS error 4551) |

An initial parallel/default-thread IPC run exposed a pre-existing Windows test cleanup race in `daemon_startup_worker_reconstructs_missing_derived_state`; after adding bounded teardown waiting, the exact test and serial IPC suite passed.

## Packaged Windows native verification

- Host: Windows 11 x64; isolated pilot only, under `%LOCALAPPDATA%\Cryptex-Dev\m6-native-af06deb8e4c745969610d44ba04ae9e7`.
- Selected the isolated synthetic vault through Cryptex's native folder picker. The picker showed the test vault's `.obsidian` folder and `Cryptex` generated subtree.
- Cryptex refreshed the vault and reported `Vault refreshed with 1 notes and opened in Obsidian`.
- Obsidian opened `Cryptex/notes/6bc43ede-ba79-40eb-b11a-77c8fb88003e.md`; the visible note title, `event_id`, `brain_id`, `public` classification, and synthetic body matched the fixture.
- The synthetic `owner-note.md` remained present with its expected sentinel text after refresh. Its observed SHA-256 was `288965457686952BB7CD5187DB75F11F6098F7893B8D73386ED2F7DA14BB355E`; no pre-refresh hash was captured, so this is presence/content evidence, not a hash-based before/after comparison.
- Obsidian Graph opened in the isolated vault and showed the generated UUID note and `owner-note` as graph nodes.
- Cryptex was closed through **File → Quit Cryptex → Yes** after the owner explicitly authorized discarding the isolated synthetic session. Runtime reports `clean_quit_from_app_menu`, `daemon_pid=none`, `supervisor_alive=false`, and `clean shutdown released instance lock`; no `cryptex*` process remained.
- The generated-note URI and Graph view are natively verified.

This was a local release-package pilot from the dirty working tree, not an Actions artifact. No real vault was selected or modified.

## Remaining gates

- macOS fmt/test/clippy/build and packaged native verification.
- Finish required M6 P1 cross-platform evidence and perform the scoped M6 Definition-of-Done review before declaring M6 complete.
- Resolve complete generated-vault coverage and safe link representation. Current renderer emits v1 memory notes only and no relationship/attachment links; see `m6-final-dod-audit-20261013.md`.

No Actions, merge, commit, or push performed. Actions remain disabled by owner instruction.
