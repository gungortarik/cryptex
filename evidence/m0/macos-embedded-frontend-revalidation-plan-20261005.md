# Mac task — release frontend mode and blank-window revalidation

Use clean current branch containing `65ce7b9`. Read handoff, M0/DoD and
`macos-native-foundation-ui-blank-failure-20261005.md`. Report exact checkout SHA,
platform/tools and build command/features; source equality is not artifact equality.
This supersedes reuse of the `7f5ab2b` raw Cargo release for native visuals.

## Focused automated checks and negative build

Run in order, stopping on first unexpected failure:

```sh
cargo fmt --all -- --check
cargo test --locked -p cryptex-desktop
cargo clippy --locked -p cryptex-desktop --all-targets -- -D warnings
cargo build --locked --release -p cryptex-desktop
```

Expected Mac debug **16/16**, no errors/ignored. Last command intentionally builds
the wrong Tauri frontend mode for a negative **pre-Tauri** rejection check only;
never show/use it as the native visual package.

Mac Codex: preserve this just-built `target/release/cryptex-desktop` in a fresh
external Cryptex-Dev proof input, hashing it before later builds overwrite it.
Create a separate fresh ordinary absolute runtime containing only exact LF
`pilot-runtime.marker` (`m0-native-desktop-runtime-v1\n`). Execute that preserved
wrong-mode binary with `--cryptex-pilot-runtime <root>` using captured subprocess
output and a five-second timeout. Expected exit **2** and
`RELEASE_FRONTEND_MODE_REJECTED: ReleaseUsesDevelopmentServer`; root remains
marker-only, no window/daemon/runtime artifacts. Invalid-root priority remains
`PILOT_RUNTIME_REJECTED`; do not relax either expected error.

This negative guard proof is not a renderer PASS. No GUI/security settings needed.

## Actual embedded frontend package

```sh
pnpm --filter @cryptex/desktop tauri:package --bundles app
cargo test --locked --release -p cryptex-desktop --features tauri/custom-protocol
cargo clippy --locked --release -p cryptex-desktop --features tauri/custom-protocol --all-targets -- -D warnings
CRYPTEX_PILOT_DESKTOP_TEST_BINARY="$PWD/target/release/bundle/macos/Cryptex.app/Contents/MacOS/cryptex-desktop" node --test tools/m0-lifecycle-compare/scripts/package-desktop-pilot-cli.test.mjs
git diff --check
```

Expected release **17/17**, CLI **4/4**, no fail/skip/ignored, all exit0. Release-
only `supported_release_suite_requires_embedded_assets` must run, not be filtered.
The Tauri command rebuilds actual frontend and stages the target-matched sidecar.
Record emitted asset filenames/SHA-256, Info.plist version and hashes/architecture
of both `.app` executables. Do not copy a raw-Cargo executable over the packaged
Desktop, manually patch assets, disable CSP or start Vite to conceal the defect.

## One isolated native visual session

Use this **newly built .app**, existing native gate/session helper and fresh
owned pilot root. Establish actual parent/path/protocol/lock correspondence and
show at the agreed moment. Use the operator-screenshot fallback in
`m0-native-foundation-ui-review-plan-20261005.md`; app-binding automation is not
required. Request actual top/health and bottom-after-scroll captures, inspect
files, hash/report dimensions/theme and correlate displayed PID/protocol.

If still entirely blank: **FAIL**, one screenshot enough. Retain new package/
asset hashes and captured startup output. Do not assert the proposed diagnosis
was confirmed. If rendered, review readable typography/path/wrapping/buttons and
scrolling at narrow M0 scope. This is not a full theme/DPI/product-design gate.

Always cleanly stop only owned children, release gate and report exit/PID/daemon
lock/IPC/quit-error state. Keep old failing artifacts intact; fresh evidence does
not erase failure. Return exact SHAs, commands/features/counts/exits, artifact
paths/hashes, visual verdict and cleanup. No source/evidence edits or commit/push.
No old lifecycle/update/Syncthing suites, installation/startup/global changes or
Actions. Corrected-source final CI will be batched after parity/review.

## Reported outcome (2026-10-05)

Operator reports **SCOPED PASS** on checkout `e755601a136e0231cbbbef279112cad0597b6f7a`.
Debug 16/16, embedded release 17/17, CLI 4/4, lint/format/diff checks passed;
the wrong-mode binary rejected before Tauri startup with expected exit 2. A
fresh Tauri `Cryptex.app` rendered and supplied top/health and scrolled-bottom
captures, with owned session cleanup. Details and hashes are in
`macos-embedded-frontend-visual-evidence-20261005.md`. Raw Mac artifacts were
not transferred to this checkout for independent re-hashing. This closes the
scoped blank-renderer visual gate, not general theme/DPI or production gates.
