# Remaining task: native M0 foundation visual review

**Superseded continuation:** operator fallback atdee43e2 showed a fully blank
Mac renderer (FAIL). Follow `macos-embedded-frontend-revalidation-plan-20261005.md`
first; do not reuse retained7f5ab2b raw Cargo release. Screenshot review methods
below remain applicable to the newly packaged, isolated session.

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

## Mac operator-screenshot fallback after Invalid app

Attempt at `9752798`: native health/cleanup proved, Computer Use `Invalid app`
prevented visual verification. See `macos-native-foundation-ui-attempt-20261005.md`.
Preserve NOT VERIFIED; do not repeatedly retry invalid app binding or change
accessibility/security policy.

1. Mac Codex confirms relevant application source still matches retained
   `7f5ab2b` build. If equal, no rebuild/full suite needed. Use the existing
   native gate/session helper with a **fresh** isolated runtime and known package
   paths; show the window at an agreed moment. Keep its owned runner/lease alive
   through review/cleanup. Do not reuse the already closed proof runtime.
2. Ask the operator for two actual whole-window screenshots using macOS's normal
   screenshot UI, supplied to Mac Codex as files:
   - **Top/current state:** title/probe heading, actual status/runtime path,
     populated Daemon sidecar and action controls. Click IPC health ping once
     so the capture shows actual PID/protocol; correlate with this new session.
   - **Bottom after scrolling:** complete lower action rows, Quit clean and
     bottom instruction text. Ordinary scrolling suffices; no theme/permission
     changes, new tray/login tests or edited/composed images.
3. Mac Codex reads these files with its image viewer; app recognition is not
   required for image-file inspection. Report readability, overlap/wrapping,
   long path accessibility and control reachability. Retain absolute screenshot
   paths/SHA-256, observed window size/theme and exact checkout/build SHAs.
   Keep any unestablished item NOT VERIFIED.
4. Cleanly stop only the owned session with the existing session helper/pilot
   quit, release gate and check exit/PID/lock/IPC/error state. Return new session
   report/digest and images/findings. Earlier PIDs/digest are not new proof.

No source modification, dependency change, installation, old lifecycle tests,
Actions or commit/push. If images cannot be supplied now, retain NOT VERIFIED
and cleanly close the owned session; never leave test daemons running.
