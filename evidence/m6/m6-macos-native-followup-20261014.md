# M6 macOS native follow-up

**Date:** 2026-10-14  
**Host:** macOS 27.0.1 arm64  
**Branch / HEAD:** `codex/m5-capture-timeline` / `87e9c4b0a49c5b670bdd7c6812d5a7c3fa768219`  
**Product changes:** uncommitted M6 working tree; the recorded HEAD does not include the local changes.  
**Pilot:** `~/Library/Application Support/Cryptex-Dev/m6-native-0811f5828e3542fcb92ed3117f8b9949`  
**Synthetic vault:** `m6-vault-0811f5828e35`  
**Fixture event:** `7b9dc3c0-1514-4e70-89c3-11923795b151`

## Results

| Check | Result |
|---|---|
| Fresh isolated vault opened in Obsidian before refresh | PASS |
| Cryptex picker refresh wrote note and manifest | PASS |
| Generated note appeared in this vault's `workspace.json` `lastOpenFiles` after refresh | PASS |
| Obsidian Graph showed this vault | PASS |
| Synthetic `owner-note.md` sentinel unchanged | PASS; SHA-256 `cd82ad3a…a27ca1cc` |
| Cryptex clean app-menu Quit | PASS; `clean_quit_from_app_menu` |
| Daemon instance lock released | PASS; `INSTANCE_LOCK=RELEASED` |

The first AX window title remained `New tab`; note opening is evidenced by the fresh vault's `workspace.json`, and Graph was separately confirmed for the unique vault. No personal vault was used.

Actions, commit, push, and merge were not run. This closes the M6 macOS native refresh/open/Graph check for the current local candidate. It does not close the remaining M6 scope, P1 cross-volume, CI-policy, or final-review gates.
