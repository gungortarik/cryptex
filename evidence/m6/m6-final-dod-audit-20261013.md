# M6 interim Definition-of-Done audit

**Date:** 2026-10-14  
**Checkout:** `codex/m5-capture-timeline` at `87e9c4b0a49c5b670bdd7c6812d5a7c3fa768219`; M6 product changes remain uncommitted in the working tree.  
**Verdict:** M6 is **not complete**. This is a current-gate audit, not owner acceptance.

**Owner direction (2026-10-14):** Keep the accepted M6 scope; do not narrow its safe-link criteria. This does not itself authorize changes outside M6.

## Delivered scope

- Deterministic v1 `memory.note` Markdown renderer with safe UUID paths and quoted YAML provenance.
- Staged rebuild of only the owned `Cryptex/` projection subtree, manifest/marker validation, copy-only synthetic attachment boundary, and fail-closed handling for unsafe or modified generated content.
- Daemon retrieval-safety verification and authenticated `content.vault.rebuild` Desktop capability; source-repository/runtime destinations are guarded.
- Desktop native folder selection, refresh, generated-note opening in Obsidian, and Graph handoff. No canonical write-back or M9 blob backend was added.

## Scope and security boundary

The current renderer accepts only v1 `memory.note` events and emits no Markdown links. ADR-0017 §5.4 and `evaluate_retrieval_safety_until` place a Brain-wide retrieval hold on supported v2 records until their privacy/classification effects are integrated across retrieval surfaces, including M3 FTS, Desktop, and Obsidian. Exporting M4 v2 entities or relations into the vault now would bypass the accepted safety gate. No such export was added. The owner confirmed that the accepted M6 scope must not be narrowed; fulfilling its complete-vault/safe-link criteria therefore requires resolving this shared-safety integration without violating ADR-0017. Changes beyond M6 need explicit authorization.

## Verification recorded

| Gate | Result |
|---|---|
| Windows projection tests | 59/59 PASS |
| Windows daemon materialization tests | 11/11 PASS |
| Windows authenticated IPC tests | 42/42 PASS |
| Windows Desktop URI/path tests | 2/2 PASS |
| Windows format, checks, frontend build, package smoke | PASS |
| Windows package native vault refresh | PASS; one generated note opened and matched the synthetic fixture |
| Windows Obsidian Graph | PASS; isolated graph showed the generated note and test owner note |
| Windows clean pilot shutdown | PASS; runtime records `clean_quit_from_app_menu`, released lock, and no remaining Cryptex process |
| Windows Clippy | PASS on 2026-10-14 local M6 working tree; strict `cryptexd --all-targets` and Desktop binary Clippy |
| macOS automated implementation verification | PASS, owner-reported at the same local HEAD; fmt, projection 59/59, focused vault IPC 3/3, Desktop vault URI 3/3 (including Unix symlink-parent regression), strict daemon/Desktop Clippy |
| macOS packaged/native synthetic vault refresh, note open, Graph, sentinel, clean quit/lock release | PASS, owner-reported at same local HEAD; unique isolated vault; see `m6-macos-native-followup-20261014.md` |
| Mac cross-volume P1 case | PASS, owner-reported on a separate temporary APFS image; hard-link EXDEV and verified copy fallback; see `m6-p1-attachment-projection-macos-cross-volume-20261014.md` |
| Required CI | OPEN; owner instruction prohibits Actions |
| Final human review | OPEN |

The native Windows run used an isolated synthetic vault only. Its `owner-note.md` still contained the expected sentinel after refresh; no pre-refresh hash was captured, so this does not establish a byte-for-byte before/after hash comparison. No real vault was selected.

## Remaining gates

1. Resolve how to complete M6's accepted safe-link scope while preserving ADR-0017's shared retrieval safety across M3, Desktop, and Obsidian.
2. Resolve the conflict between the M6 completion gate's required CI and the owner's no-Actions instruction before claiming completion.
3. Complete human review of the final diff.
4. Do not expose M4 v2 records in Obsidian until ADR-0017's shared retrieval-safety gate permits them.

No Actions, commit, push, merge, M7 write-back, or M9 canonical blob implementation was performed. Scratch files and unrelated untracked files were preserved.
