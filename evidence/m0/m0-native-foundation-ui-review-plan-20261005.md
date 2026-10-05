# Remaining task: native M0 foundation visual review

Read handoff, M0 contract, DoD sections 9/11, UX design specification and
`m0-foundation-ui-review-20261005.md`. Report exact source/build SHA and platform.
This is a narrow visual review, not a new lifecycle/upgrade/login test.
Windows actual native review now scoped PASS in the result above; do not repeat
it. **Remaining executor task is macOS only.** Both-host instructions below
retain the common isolation/review contract.

On each host, use a current-source foundation Desktop build with the backend-
selected target-matched sidecar. Prefer a retained current build with known SHA;
if rebuilding is required, use existing package-smoke instructions. Do not use
an older editable-sidecar shell or change dependencies/security policy.

1. Coordinate a safe visual-review moment with the owner. Windows owner may be
   gaming: do not steal focus, use the left monitor only when available. If no
   safe native view exists, report NOT VERIFIED rather than disturbing the game.
2. Use the validated isolated pilot runtime mechanism already documented in
   `p0-2/macos-owned-desktop-session-test-plan-20261005.md` / Windows session
   evidence, with a fresh Cryptex-Dev root and exact marker. Pilot starts hidden;
   show its main window only at the agreed moment. Never mutate product runtime,
   existing login registration, global Syncthing or retained corpora.
   The older plan is a reference for pilot isolation/helper semantics only:
   **do not execute its full build/test/lifecycle command sequence**. Create
   a fresh ordinary absolute directory outside Git with only
   `pilot-runtime.marker`, containing exactly UTF-8 LF
   `m0-native-desktop-runtime-v1\n` (one final newline). Launch the known
   current executable with `--cryptex-pilot-runtime <absolute-root>`. Wait for
   its real launched snapshot/readiness before writing `show.request` in that
   owned root to display the pilot. Do not put request files in the initial root
   before launch: preflight requires marker-only membership.
3. Visually inspect heading/probe scope, typography and contrast, complete
   populated read-only sidecar path, wrapped status/errors, action wrapping,
   reachable bottom controls and ordinary vertical scrolling. Record viewport/
   scaling/theme used and actual screenshot path/digest. Do not claim a full
   theme/DPI matrix from one view. Verify no overlap/unreachable control.
4. The UI reviewer should relate this foundation view to UX/DoD while preserving
   M0 non-goals. Explicit developer controls are not normal product UI acceptance.
5. Cleanly quit only the owned isolated session using its existing clean quit
   control/mechanism; record process/lock cleanup. Do not repeat login, tray,
   update, rollback, Syncthing or entire automated suites. No Actions.
   A fresh `pilot-quit.request` containing `quit\n` in this owned runtime is
   the documented file-control clean-quit mechanism. Check actual Desktop exit,
   absent owned daemon PID/released lock/IPC, and any `pilot-quit.error`; no
   broad process-name kill or forced-cleanup-as-PASS. Keep screenshot/report
   artifacts outside the initially marker-only runtime (e.g. a sibling proof directory).

Return scoped PASS / FAIL / NOT VERIFIED, exact SHA/platform, screenshots and
findings, plus cleanup. Stop on security barrier or layout defect and report it;
never weaken tests, silently accept architecture or change implementation.
Mac executor: no source/evidence changes or commit/push; Windows implementer
records the returned result. Final human ADR/M0 acceptance remains separate.
