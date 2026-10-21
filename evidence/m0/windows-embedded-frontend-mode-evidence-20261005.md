# Windows release frontend build-mode correction

Implementation source: `65ce7b91fe500cfe751aa3d13b39010f6dcc935e`.
Scope: M0 release build-mode guard / packaging regression; no renderer redesign,
new product API, startup setting or CSP relaxation. Mac visual recovery pending.

## Cause found in source

Pinned local Tauri 2.12.0 `src/lib.rs::is_dev` is
`!cfg!(feature = "custom-protocol")`; manager's `get_app_url` uses `devUrl`
under development cfg. `tauri-build 2.7.0::is_dev` documents the same dependency
feature boundary. Rust release optimization alone does not select embedded
assets. Old Mac namespace plan explicitly used raw Cargo release without this
feature. This is a confirmed build-mode defect, not yet confirmation of the
precise Mac screenshot cause.

Official standalone packaging path: [Tauri distribution documentation](https://v2.tauri.app/distribute/).
No dependency versions/security policy changed.

## Change and independent review

- Typed `FrontendBuildModeError::ReleaseUsesDevelopmentServer` from
  `frontend_build_mode::ensure_current` exits 2 with constant diagnostic before
  context/window/plugins/daemon. Invalid pilot arguments still reject first.
- Debug development remains supported. Packaged release must use embedded
  frontend (`tauri/custom-protocol`); README documents actual Tauri build path.
- CI release build/tests/lint now explicitly enable that dependency feature.
  Full release Desktop suite replaces single filtered resolver test.
- Separate verifier/security reviewers inspected guard/order/CI. No new security
  blocker. Verifier identified tautological mode-test coverage; added release-only
  sentinel asserting non-dev mode and guard Ok. Verifier confirmed final `65ce7b9`
  closes accidental feature-omission gap. No failing assertion weakened.

## Actual Windows checks

| Check | Result |
| --- | --- |
| Format | PASS after formatter repaired wrapping in newly added assertion; initial format-only exit 1 retained here |
| Desktop debug | 23/23 PASS |
| Desktop embedded-frontend release | 24/24 PASS, release sentinel executed, no ignored/filtered |
| Strict debug/release Clippy | PASS, all targets, no warnings |
| Raw wrong-mode release build | PASS compilation; negative startup rejection expected below |
| Current-source Tauri package-smoke / frontend / sidecar | PASS; exact `65ce7b9` package rebuilt |
| Existing negative pilot CLI tests | 4/4 PASS, no fail/skip |

### Deliberately wrong-mode release

Valid fresh owned pilot root supplied to plain Cargo release (captured subprocess,
5-second timeout, hidden console). Exit **2**, diagnostic
`RELEASE_FRONTEND_MODE_REJECTED: ReleaseUsesDevelopmentServer`.
Root remained marker-only, no Tauri/daemon start. This proves the guard behavior,
not a successful native renderer. Working implementation later committed as `65ce7b9`;
subsequent change before commit was release-test-only sentinel, not runtime code.

Report:
`C:\Users\Tarik\AppData\Local\Cryptex-Dev\p0-2\windows-frontend-mode-02115427-7c35-4823-a104-8535c46fdd7c\report.json`.
SHA-256 `f307b484a004aae52e9a50989ca7c2933d2daaa9599370dca2ed246e1fc4add9`.
Rejected binary SHA-256
`ad2ab97ffdc77cb47fdc0342e07bbcb0794d20e813eaecea24a24654390e1acd`.

### Supported package: one hidden positive session

Known source `65ce7b9` Tauri-built pair, existing native gate/session helper, fresh
isolated proof. Desktop **13776**, daemon **21244**, runner **29024**; exact
parent/executable paths verified. Actual protocol 1 `health.pong`/held lock.
Guard accepted supported package. No window shown or game UI disturbed;
`visual_verification=false`. No repeat of physical UI/tray/login/upgrade tests.

Clean exit **0**, recorded PIDs absent through helper cleanup, daemon lock/IPC
released and native gate released. Final read-only endpoint probe absent.
Report explicitly includes source SHA and gate-release result:
`C:\Users\Tarik\AppData\Local\Cryptex-Dev\p0-2\windows-embedded-frontend-f3e6bc17-45c9-4bee-812b-9e8e5479b9d2\proof\report.json`.
SHA-256 `c317b6b42ad18fd799a2a1fa12d29edefd1cd92fcd514593c9dc6bb6539d1ace`.

## Remaining

Mac focused 16 debug / 17 release / 4 CLI + actual fresh Tauri `.app` visual proof under
`macos-embedded-frontend-revalidation-plan-20261005.md`. If still blank, retain
FAIL and investigate asset/WebKit/rendering independently. Earlier `5c07411` cloud
PASS does not test this new implementation/workflow. Batch required fresh final
CI after parity/review; no Actions run for this slice. ADR/M0 acceptance stays open.
