<!-- Owner-supplied Mac report, recorded on Windows; observations were not independently repeated here. -->

## M5 Slice 3 macOS native interactive — results

**Source HEAD:** `0e5b98bc2719b0d59f4209add4237d6fa7fef409` (docs tip; product `e874c066…` ancestor)
**Native build:** `tauri:package --bundles app` → exit 0
**Dialog buttons observed:** **Cancel** / **Discard and Quit** (title **Cryptex**)

### Pilots
1. `…/m5/macos-slice3-interactive-20261011T034452Z-bb84fe36-5490-430f-b608-57a0875c1541` — Desktop **98275** / daemon **98285**
2. `…/m5/macos-slice3-confirm-20261011T045023Z-a16ccfa7-9a1b-4a3e-82bb-be26f7f641f1` — Desktop **99761** / daemon **99769** (fresh runtime after app-menu Quit ended session 1)

### Interaction results

| Check | Verdict | Evidence |
|-------|---------|----------|
| Tray **Quit Cryptex** → **Cancel** | **PASS** | Physical tray; both PIDs stayed; draft `M5 synthetic draft — preserve me` / `public` unchanged |
| Close-to-tray | **PASS** | `close.request` → `window_closed_hidden`, `window_visible=false`; both PIDs alive |
| Reopen same session | **PASS** | `show.request` → same draft + classification |
| App menu **Quit Cryptex** → Cancel | **FAIL** | No confirm dialog; Desktop **98275** exited; orphan daemon stopped via `stop\n` (`clean shutdown released instance lock`) |
| Tray **Quit Cryptex** → **Discard and Quit** | **PASS** | Both PIDs gone; `note=clean_quit_from_tray`; lifecycle `clean shutdown released instance lock` |
| Archived unresolved after newer commit + Quit protect | **NOT VERIFIED** | Not safely reproducible in isolated pilot without fabricating committed/Uncertain state |

### Remaining gaps
- App-menu **Quit Cryptex** bypasses the capture Quit confirm path (desktop exits; draft lost).
- Tray menu still not operable via Accessibility; physical clicks required.
- Archive/unresolved UX not interactively verified (policy tests remain separate automated evidence).

No production changes, commit, push, Actions, merge, Slice 4, or M5-complete claim.