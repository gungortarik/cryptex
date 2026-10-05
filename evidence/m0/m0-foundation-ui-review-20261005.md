# M0 foundation UI review

Source: `3de9d000f131cecf5d816f8dd16937eb101fcee8`.
Scope: temporary blank foundation/probe shell, not product UI/P1 acceptance.
Authority: M0 non-goals, DoD sections 9/11 and `docs/14_DESKTOP_UX_AND_DESIGN_SYSTEM.md`.

## Windows-host renderer observation

Current frontend rebuilt successfully with `pnpm --filter @cryptex/desktop build`
(exit 0). Static output served temporarily on loopback 127.0.0.1:1437 and viewed
in a **hidden** in-app browser, 1280 x 720. No Desktop/daemon, login setting or
global configuration changed; game focus and visible screen were untouched.
Tab closed and owned static server terminated after observation.

Actual screenshot and accessibility observation showed a readable single
foundation card, explicit M0/P0-2 label, nine native-purpose controls, wrapping
action rows and no overlap/horizontal overflow. DOM confirmed `Daemon sidecar`
input `readOnly=true`. No framework error overlay appeared. Browser-only absence
of Tauri correctly appeared as an invoke error in the status text; blank sidecar
is expected here. This is **not** a successful native IPC/autostart session.
An unrelated favicon request returned 404; application assets returned 200.
Screenshot observation is retained in this review's tool transcript, not falsely
described as a committed native capture.

Source review: status uses actual command results/errors, text is React-escaped,
backend-selected sidecar is display-only, layout is a single probe surface rather
than broad product navigation. Start/Stop and technical details are explicitly
temporary developer probes; normal product controls must follow UX section 5.
Retained both-host tray/close/reconnect/login evidence supports historical native
behavior, not a new current-source visual review.

## Actual Windows native visual review

Owner authorized the real window on the left display, then explicitly offered
to move it there because Computer Use has no hidden-window placement API.
Computer Use targeted only the returned isolated pilot window. Monitor identity
is owner-coordinated, not independently inferred from the screenshot origin.

Current unchanged application source rebuilt by
`pnpm --filter @cryptex/desktop tauri:package-smoke` (exit 0). Fresh known
Desktop/daemon pair staged in a new Cryptex-Dev proof, with native gate held and
pilot isolation. Real Desktop PID **19376**, daemon **1116**, runner parent
**31372**; paths and child-parent chain verified by existing session helper.
Initial hidden snapshot and actual protocol-1 health/held lock verified before
showing. No install, login setting or global runtime/configuration changed.
Build HEAD was `3de9d00` with only review-documentation edits in progress;
application/workflow source unchanged from the green candidate. The retained
session report contains manifest/identity/health/cleanup, not a source-SHA field
or gate-release field. Source/build provenance comes from this run record;
native-gate release comes from holder transcript and successful runner exit.

An initial attempt to import the Node helper directly in the UI REPL failed
before native launch because that sandbox lacks the `process` global. The
same repository helper was then run by a normal external Node holder; no
implementation, assertions or security policy changed to bypass the error.

Observed dark native window: screenshot **962 x 672** (configured client
960 x 640); DPI scale not separately measured. Typography/labels were readable,
no overlapping controls observed, long runtime text wrapped, sidecar field had
the complete backend path in accessibility text (its one-line visual viewport
truncates normally). Nine buttons arranged in wrapping rows. Bottom caption
was initially below the viewport and became fully visible with normal vertical
scrolling; Quit clean and lower controls remained reachable.

Clicked actual **IPC health ping**: renderer displayed
`IPC health.pong · daemon pid=1116 · protocol=1`, matching measured daemon.
No optimistic health substituted for configuration. The same compiled frontend
asset/source carries the independently checked read-only sidecar property.
No path edit, Start/Stop, login, tray or old upgrade test repeated.

Clicked actual **Quit clean**; runner exit **0**, Desktop and daemon absent,
daemon lock released, IPC endpoint absent, native gate cleanly released. Snapshot
`note=clean_quit`, `daemon_pid=none`; no forced cleanup or quit error.

Retained proof directory:
`C:\Users\Tarik\AppData\Local\Cryptex-Dev\p0-2\windows-native-ui-20261005-4a3e3bfb-d466-425b-bb31-fcf5a44fc0c0\proof`.

| Artifact | SHA-256 |
| --- | --- |
| `native-top.jpg` | `9e4c28501e7c463676e21590092084b3c4df58473ddbbb63ffe3ef675d37222f` |
| `native-health.jpg` | `fed1ac0397547166e460d902a782a8ff2277dd2b49b6dc3294cc5acd325a6d71` |
| `native-bottom.jpg` | `c0242695719ca37e334b98fc0a910cb31d308abebe50cc6e87f9db9392eaa63f` |
| `ui-session-report.json` | `0768bc5961ab8c2a07892ae6efc02db0fad316f6b060775551d0e757cb7fb11f` |

Independent verifier subsequently opened all three images, recomputed all four
digests and read the report. Scoped Windows visual verdict **PASS**: no overlap,
readable probe typography, long-path wrapping and reachable controls. Read-only
process query also found Desktop19376/daemon1116/runner31372 absent. Mac/native
theme/DPI/production design acceptance was not inferred.

## Outcome

- **SCOPED PASS:** current renderer layout/read-only delta in the observed Windows-host browser.
- **SCOPED PASS:** actual Windows native foundation visual review and clean
  owned-session cleanup, with populated long path/status and reachable controls.
- **NOT YET VERIFIED:** equivalent explicit current Mac native visual review.
- **OUT OF SCOPE:** broad product visual design/P1 shell, theme/high-DPI/Retina
  matrix and production Health/Memory/navigation.

Do not mark the both-platform UI-review gate complete before the Mac result.
Continuation: `m0-native-foundation-ui-review-plan-20261005.md`. No full suite,
old login/tray tests or Actions repetition is requested for this remaining review.
