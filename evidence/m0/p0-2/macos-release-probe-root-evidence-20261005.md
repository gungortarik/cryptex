# macOS M0 release probe root — scoped PASS

Tested SHA: `7f5ab2bdaa72e213f5cb16312bca0292c436db18`.
macOS 27.0.1 arm64; Rust/Cargo 1.98.1; Node v24.19.0.
Plan: `macos-release-probe-root-test-plan-20261005.md`.

The project owner supplied the Mac executor's result. Windows records that
report; no Mac artifact was opened from Windows and no raw artifact digest was
supplied for this focused run.

| Check | Reported result |
| --- | --- |
| Desktop debug tests | 14/14 PASS |
| Desktop release tests | 14/14 PASS |
| Negative pre-Tauri CLI tests | 4/4 PASS |
| Format, debug/release strict Clippy | PASS |
| Locked release build and diff check | PASS |
| All plan commands | Exit 0 |
| Fixture cleanup | Error-free |

No failures/skips/ignored. Compiled-default resolver executed in both profiles:
release `~/Library/Application Support/Cryptex-Dev/runtime/p0-2`, debug retained
comparison root. Missing-HOME, sidecar and pilot-root validations passed.

Repo clean; no edits/commit/push/Actions. No app/daemon opened, previous plan
repeated, or old runtime/startup/global configuration changed. This closes the
new Mac path/lint parity gate. It is resolver/build/negative CLI evidence, not
physical login or installed-update proof. Final CI, human ADR/M0 acceptance and
mandatory production gates remain separate.
