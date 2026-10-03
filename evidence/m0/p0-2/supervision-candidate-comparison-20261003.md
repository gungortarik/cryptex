# P0-2 supervision candidate comparison — 2026-10-03

**Status:** Evidence comparison only; no architecture selected  
**ADR-0012:** UNRESOLVED  
**Scope:** Current repository evidence for Windows and macOS lifecycle candidates

This comparison consolidates recorded P0-2 evidence. It does not turn a
test-only harness into production integration and does not accept ADR-0012.
Where the evidence differs by platform or test class, the distinction remains
explicit.

## Candidate comparison

| Capability | Native per-user startup / supervision (Mode A) | Desktop process supervisor (Mode B) | Evidence consequence |
|---|---|---|---|
| Start at login | macOS test LaunchAgent physically survived login/reboot. Windows test HKCU Run start and ten-minute survival physically verified; a separate earlier login dispatch failure still has no established cause. | Does not start the Desktop at login by itself; needs an OS login registration or an always-open Desktop. | Native startup has direct test evidence on both platforms, but its Windows Run candidate is login-only and its historical failure remains unexplained. |
| Daemon crash detection/restart | macOS LaunchAgent `KeepAlive.SuccessfulExit=false` restarted the test daemon. Windows HKCU Run and the attempted no-elevation `schtasks` candidate do not provide a verified no-elevation crash-restart path; `schtasks /Create` was denied. | Process-supervisor crash/restart passed in physical harness runs on both platforms. | Mode B has cross-platform harness evidence for restart; Mode A crash-restart evidence is asymmetric. |
| Desktop renderer reconnect during restart | Not demonstrated as a native-supervisor-owned restart. | Actual Tauri renderer showed the replacement PID during externally supervised Mode B restart on Windows (`35224 → 3432`, 32 ms) and macOS (`24966 → 24987`, 30 ms). | Renderer reconnect is VERIFIED on both platforms for this external Mode B experiment. It does not prove production Desktop ownership of the supervisor. |
| Window close / daemon availability | Native daemon lifecycle is independent of Desktop window close. | Tauri `CloseRequested` hide/restore path and daemon survival have test evidence on both platforms. Literal macOS traffic-light interaction remains unverified due Accessibility permission; Windows proof used the controlled CloseRequested event path. | The event path is evidenced; native user input and product tray UX are not fully established. |
| Single instance / cleanup | Test registration and daemon lock checks exist; platform proofs record duplicate rejection and scoped cleanup. | Duplicate rejection, clean stop, and cleanup passed in harness/UI proof flows on both platforms. | Test-level invariants have evidence; production registration/package integration remains open. |
| Update and rollback | No platform-native updater decision or production integration proven. | Synthetic compatible replacement, bad-candidate rejection, rollback, and marker preservation passed on macOS and Windows harnesses. | Test fixture behavior is verified; signed production package, actual Desktop+daemon coordination, and production canonical-data safety remain open. |
| Registration privilege / user session | macOS LaunchAgent uses the user session. Windows HKCU Run needs no elevation; Windows `schtasks /Create` failed with Access Denied in the normal user session. | Desktop runs in the interactive user session but still needs a login-start mechanism. | Task Scheduler via the tested `schtasks` path is not a no-elevation candidate. Do not infer that one failed candidate selects Mode B. |

## Evidence references

- Windows native startup and survival: `windows-reboot-login-survival-20261002.md`
- Windows Task Scheduler permission result: `windows-mode-a-task-scheduler-permission-20261002.md`
- Windows historical/instrumented login findings: `windows-instrumented-login-result-20261002.md` and `windows-run-dispatch-followup-20261002.md`
- Windows Mode B lifecycle and GUI event path: `windows-desktop-supervision-evidence.md` and `windows-mode-b-gui-window-close-20261002.md`
- Windows renderer reconnect: `windows-tauri-renderer-ipc-reconnect-20261003.md`
- macOS native login/reboot and crash restart: `macos-native-supervision-evidence.md`
- macOS Mode B lifecycle and close event path: `macos-desktop-supervision-evidence.md` and `macos-mode-b-gui-window-close.md`
- macOS renderer reconnect: `macos-tauri-renderer-ipc-reconnect-20261003.md`
- Synthetic update/rollback: `windows-update-rollback-evidence-20261002.md` and `macos-update-rollback-evidence.md`

## Remaining P0-2 gates

1. Decide and implement production Desktop/daemon supervision ownership, login
   startup integration, and close/quit behavior for both platforms.
2. Prove compatible Desktop **and** daemon update coordination, including
   controlled error/rollback and canonical-data preservation through the real
   product lifecycle. Current marker fixtures are synthetic.
3. Complete the cross-platform comparison against those production-level
   proofs and record the selected approach in ADR-0012 before M1/M2.

The separate historical Windows HKCU Run failure is still unexplained. It must
not be mislabeled as a current failed login proof: the later test-only reboot /
login survival experiment passed. It remains relevant diagnostic history, not
a reason to discard the later verified result.
