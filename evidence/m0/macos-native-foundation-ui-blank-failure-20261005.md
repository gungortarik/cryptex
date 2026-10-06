# Mac native foundation renderer — FAIL

Owner/executor reports an entirely empty native window with no buttons; no
second scrolling capture possible. **Visual FAIL**, not merely unavailable UI
automation. Source checkout: `dee43e209beaab5623fb57fce2e7ac472942e37a`.
Retained build: `7f5ab2bdaa72e213f5cb16312bca0292c436db18`; relevant application
source reported identical. Mac-local artifacts were not opened on Windows.

Retained proof root:
`/Users/tarikgungor/Library/Application Support/Cryptex-Dev/p0-2/mac-native-ui-fallback-20261005-6C83F00A-92DE-4099-AA63-A4CDEE8A33F0/proof`.
Files: `operator-blank-window.png`, `ui-session-report.json`. Digests were not
supplied in this report; do not borrow the earlier attempt's digest.

Reported clean exit **0**, daemon absent and lock/IPC cleanup successful.
No source edits, commits, old tests or Actions. A healthy backend/clean exit
does not establish a functioning renderer. Mac visual gate remains **FAIL/open**.

## Source diagnosis and correction

The retained build's repository plan used `cargo build --locked --release -p
cryptex-desktop`, without `tauri/custom-protocol`. Pinned Tauri 2.12.0 defines
`is_dev()` as absence of that feature, independently of Rust release optimization.
Its manager uses configured `devUrl` in that mode; our config points to
127.0.0.1:1420. Thus matching Rust/frontend source alone is insufficient to
establish equivalent embedded-frontend artifacts.

This establishes a build-mode defect and a strong explanation for the failed
retained executable. **The exact Mac blank-window cause/remediation remains
unconfirmed until a proper fresh Tauri package renders.** No stale JS/asset,
WebKit or rendering failure is silently ruled out.

Correction `65ce7b91fe500cfe751aa3d13b39010f6dcc935e`: typed pre-Tauri release-mode
guard, CI embedded-frontend features and expanded full release test suite. Debug
Vite development preserved, CSP unchanged. Original failure retained.
Continuation: `macos-embedded-frontend-revalidation-plan-20261005.md`.

## Superseding focused revalidation

The failure was subsequently followed by a **SCOPED PASS** using a fresh Tauri
`.app` on checkout `e755601a136e0231cbbbef279112cad0597b6f7a`. The renderer
populated; visible contents, asset/package hashes, automated counts and clean
shutdown are recorded in
`macos-embedded-frontend-visual-evidence-20261005.md`. This resolves the blank
renderer for that package path while preserving the initial failure. Raw Mac
artifacts were not copied to Windows for independent hash verification.
