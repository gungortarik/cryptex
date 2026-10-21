# M5 Slice 5 — macOS native pagination continuation

**Status:** Completed, owner-reported macOS PASS. Result: `m5-slice5-macos-native-pagination-pass-20261012.md`. Windows equivalent PASS is recorded in `m5-slice5-windows-native-pagination-pass-20261012.md`. The checklist below is retained as the historical procedure; no rerun is requested.

## Task for the Mac Cursor session

Complete only the remaining M5 native Timeline pagination check on macOS. Read `docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md` first. Do not run GitHub Actions, push commits, merge, or start M6. Reuse the existing verified app and matching daemon; do not rebuild or rerun passed suites for this interaction-only check. Preserve local changes and prior pilot evidence.

1. Report macOS version/architecture, checkout SHA, and the app/daemon paths and hashes. The tested product must contain `ae6f5a64dbf2825080778462257068744197c501`. The known app location is `${TMPDIR%/}/cryptex-m5-s5-target/release/bundle/macos/Cryptex.app`, or the previously used `$CARGO_TARGET_DIR/release/bundle/macos/Cryptex.app`. Locate the existing verified artifacts before proceeding. If unavailable or their provenance is unknown, report that specific blocker.
2. Check for an existing Cryptex pilot. Close it through its supported Quit/Confirm path before launching another, preserving its evidence. Do not force-kill processes or remove lock files to bypass single-instance handling.
3. Create a fresh runtime outside Git under a unique temporary directory. Write only `pilot-runtime.marker` containing UTF-8 without BOM: `m0-native-desktop-runtime-v1` followed by LF.
4. Use the matching `cryptexd pilot-fixture timeline-v1 --runtime-root <absolute-runtime> --confirm-isolated-pilot` command. Preserve its output. Launch the verified app with `--cryptex-pilot-runtime <absolute-runtime>` only after successful fixture creation. Request its window through `show.request` if needed. Never seed canonical files or SQLite directly.
5. Through the ordinary Inbox UI, save twenty separate synthetic notes named `M5 paging synthetic note 01` through `M5 paging synthetic note 20`, each explicitly classified `public`. Confirm each reports durable Save. Together with the fixture, there must be 21 notes. Stop and report any rejected/uncertain outcome; do not fabricate successful state.
6. Open Timeline after derived catch-up. Confirm exactly 20 rows: numbered notes 20 through 01 in newest-first order, with `Load older notes` at the bottom. Retain accessibility text or other native observation evidence.
7. Click `Load older notes`. Confirm the fixture is appended, total count becomes 21, all first-page rows remain, no row duplicates or gaps occur, and the exhausted continuation button disappears.
8. Open the appended fixture's full note. Verify its text and Event/Brain IDs against the fixture command's output, and confirm provenance fields are present.
9. Quit through the supported app-menu/Confirm path. Verify both owned Desktop/daemon PIDs are gone; record `desktop-supervisor.state`, `lifecycle.status`, and any quit error. Preserve the isolated runtime and proof outside Git.
10. Return PASS/FAIL/BLOCKED with exact artifact identity, pilot path, observations, and shutdown evidence. Write a focused local evidence report if successful; do not push it. Do not claim M5 complete from this checklist alone.

## Interpretation

A 21-note pilot proves first-to-second-page native interaction and terminal-page behavior. It does not establish three-or-more-page interaction, concurrent-insert semantics, or power-loss durability. Existing automated projection and IPC coverage remains separate evidence. No screenshots are required from the owner when the agent can capture native observations itself.
