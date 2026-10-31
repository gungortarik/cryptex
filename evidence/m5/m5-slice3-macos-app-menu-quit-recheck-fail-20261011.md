# M5 Slice 3 -- macOS native recheck FAIL at ExitRequested-only fix

**Date:** 2026-10-11
**Branch:** `codex/m5-capture-timeline`
**Reviewed product SHA:** `3b0c6d4d0410ed963498b906c5da3e3967a6f82c`
**Host:** owner-reported macOS (native). Recorded on Windows; observations were not independently repeated here.

## Result

**FAIL** — application-menu **Quit Cryptex** and **Cmd+Q** still exited Desktop without confirmation and left the supervised daemon running.

## Root-cause note (source)

Pinned stack: Tauri `=2.12.0` / muda `0.20.0`.

- Tauri default macOS menu uses `PredefinedMenuItem::quit`.
- muda macOS maps `PredefinedMenuItemType::Quit` to native `sel!(terminate:)`.
- That path does **not** reliably reach `RunEvent::ExitRequested`.
- Therefore an ExitRequested-only guard at `3b0c6d4d0410ed963498b906c5da3e3967a6f82c` could not intercept app-menu / Cmd+Q Quit.
- Late `prevent_exit` is not the proven root cause of this failure.

Tray Quit Cancel/Confirm and close-to-tray / Dock reopen remained the previously passing behaviors from earlier native review; this recheck focused on the still-failing app-menu path.

## Follow-up

Product fix `bc3463bf2829be42d7bda41fdc3898ed4c759f1f` replaces predefined Quit with an explicit `MenuItem` (`cryptex-app-menu-quit`, `CmdOrCtrl+Q`) routed through the shared guarded shutdown path. See `evidence/m5/m5-slice3-app-menu-quit-windows-verification-20261011.md`.
