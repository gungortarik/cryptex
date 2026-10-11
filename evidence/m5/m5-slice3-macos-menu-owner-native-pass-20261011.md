# M5 Slice 3 macOS menu-owner native recheck

**Date:** 2026-10-11
**Provenance:** Owner-supplied Mac automated and native reports in this conversation. Not independently executed on this Windows host; raw proof artifacts were not attached here.
**Tested HEAD:** `3c17c7b50886139061f5ddc3651a19ea4d2f04a5`
**Product ancestor:** `b9b3e4cdfdc22077c5dd5ec78b0dbbf22969863f`
**Host:** macOS 27.0.1 (26A434), arm64.
**Target:** `${TMPDIR}/cryptex-m5-s3-menu-owner-target`

## Automated report

Format, strict Desktop-bin Clippy, frontend build and native app packaging PASS. Exit lifecycle/menu 11/11, Quit guard 5/5, capture-policy 16/16 and search-policy 2/2 PASS. This is focused verification of the changed Desktop behavior; earlier full daemon Mac checks retain their own tested SHA.

## Native report

| Check | Result | Reported evidence |
|---|---|---|
| App-menu Quit Cancel, later Cancel, then Confirm | PASS | P1 Desktop 17276 / daemon 17288; one dialog per request; Cancel preserved draft/public classification; Confirm recorded clean_quit_from_app_menu, both processes exited, lock released, no quit.error |
| Cmd+Q Cancel, later Cancel, then Confirm | PASS | P2 18562 / 18572; one dialog per request; session preserved until Confirm; clean_quit_from_app_menu and lock released |
| Close-to-tray and Dock reopen | PASS | P3 18853 / 18862 unchanged; window_closed_hidden then window_restored_via_dock_reopen; draft/public classification preserved |
| Physical tray Cancel, later request, then Confirm | Owner-reported PASS | Owner performed physical sequence because Accessibility could not open status menu. Intermediate dialog counts were not independently measured by AX. Confirm shutdown corroborated by process/status observations below |

Tray Confirm observations:

```text
note=clean_quit_from_tray
desktop_pid=18853
daemon_pid=none
daemon_pid_observation=not_running
state=clean_quit_from_tray
supervisor_alive=false
lifecycle note=clean shutdown released instance lock
```

Both P3 process IDs were reported gone; no quit.error. The reported end state establishes cleanup, not an independently measured process-exit ordering trace.

## Scope and remaining verification

This supersedes the failing native app-menu/Cmd+Q verdict for the corrected candidate only. The historical failure at 3b0c6d4 remains valid evidence.

Archive/unresolved Quit UX remains **NOT VERIFIED** interactively; automated policy tests are separate evidence. Windows native verification of the changed Quit/menu paths remains open. No forced-kill/OS-shutdown protection is claimed.

No test reruns on this host, production changes, Actions, merge, Slice 4 implementation or M5 completion are implied by recording this report.
