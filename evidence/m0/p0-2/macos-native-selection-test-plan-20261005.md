# Mac task — complete native selection and owned rollback parity

Use clean current `m0/p0-2-cursor` containing
`13b435d35ac905dd3ffb94bcd29978d36133bc95`. Read handoff and approved
`native-package-handover-plan-20261005.md`. Report the full tested SHA.

Scope: two isolated real .app selection scenarios, compatible commit and
real IPC protocol rejection followed by baseline restart. Same package bytes
are intentional; do not describe this as a version upgrade. ADR-0012 remains
PROPOSED. No Actions, installer, login/tray testing, canonical data or retained
corpora. On PASS, do not edit files or commit/push. Stop on any failure/security
barrier; retain diagnostics and do not weaken tests or change host policy.

## Build and fresh input

Use an available interactive Mac GUI session. Preserve source checkout because
the pilot validates its compile-time repository boundary. Initially hidden
Desktop launches need no clicks; report any visible flash/focus change.

```sh
cargo fmt --all -- --check
cargo test -p cryptex-desktop -p m0-lifecycle-compare --locked
cargo clippy -p cryptex-desktop -p m0-lifecycle-compare --all-targets --locked -- -D warnings
cargo build -p m0-lifecycle-compare --locked
pnpm --filter @cryptex/desktop tauri:package --bundles app
```

Require Desktop **10/10**, helper **14/14**, no failures/ignored tests;
all exits 0. Verify Desktop and bundled daemon are arm64 for the reported host.
Do not continue after a failed check.

```sh
pilot_repo="$(pwd)"
pilot_base="$HOME/Library/Application Support/Cryptex-Dev/p0-2"
pilot_parent="$pilot_base/mac-native-selection-20261005-$(uuidgen)"
mkdir -p "$pilot_base"
mkdir "$pilot_parent"
mkdir "$pilot_parent/input"
ditto "$pilot_repo/target/release/bundle/macos/Cryptex.app" "$pilot_parent/input/Cryptex.app"
export CRYPTEX_PILOT_DESKTOP_TEST_BINARY="$pilot_parent/input/Cryptex.app/Contents/MacOS/cryptex-desktop"
node --test tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs tools/m0-lifecycle-compare/scripts/package-daemon-health.test.mjs tools/m0-lifecycle-compare/scripts/package-native-recovery.test.mjs tools/m0-lifecycle-compare/scripts/package-native-tree.test.mjs tools/m0-lifecycle-compare/scripts/package-handover-gate.test.mjs tools/m0-lifecycle-compare/scripts/package-desktop-session.test.mjs tools/m0-lifecycle-compare/scripts/package-desktop-pilot-cli.test.mjs tools/m0-lifecycle-compare/scripts/package-native-selection.test.mjs
unset CRYPTEX_PILOT_DESKTOP_TEST_BINARY
node --check tools/m0-lifecycle-compare/scripts/package-native-selection.mjs
node --check tools/m0-lifecycle-compare/scripts/package-native-selection.test.mjs
node --check tools/m0-lifecycle-compare/scripts/run-native-selection-proof.mjs
node --check tools/m0-lifecycle-compare/scripts/package-desktop-session.mjs
git diff --check
```

Require **110/110 Node**, zero failures/skips, exit 0; four syntax checks and
diff check PASS. This adds 13 selection regressions to the previous 97 tests.
Do not continue to live scenarios if tests fail.

## Actual native selection scenarios

Ensure unrelated Cryptex sessions are already cleanly closed. The runner refuses
a busy IPC endpoint; never terminate an unrelated process to free it. Use the
fresh proof root once, and do not flatten .app links to bypass validation.

```sh
node tools/m0-lifecycle-compare/scripts/run-native-selection-proof.mjs --source "$pilot_parent/input/Cryptex.app" --root "$pilot_parent/proof" --helper "$pilot_repo/target/debug/m0-lifecycle-compare" --desktop Contents/MacOS/cryptex-desktop --daemon Contents/MacOS/cryptexd
shasum -a 256 "$pilot_parent/proof/native-selection-report.json"
```

Require runner exit 0 and two recorded outcomes:

1. `compatible`: baseline exits cleanly, selection becomes `candidate`, actual
   candidate Desktop owns its bundled daemon; real protocol-1 IPC succeeds.
2. `protocol-reject`: same executable bytes declare expected protocol 2;
   actual helper negotiation rejects protocol 1, candidate cleanup is proven,
   selection returns to `baseline`, new baseline Desktop/daemon becomes healthy.

For both, verify Desktop parent is the runner, daemon parent is its Desktop,
actual executable paths match the selected complete slot, `health.pong`,
protocol 1 and `lock_held=true`. Baseline, rejected candidate and final successor
must exit 0, leave no owned daemon PID/held lock or IPC endpoint. Both native
gates must release; `native-intent.json` must be absent in each scenario.
`native-selection.json` must be candidate for compatible and baseline for
protocol-reject. Do not delete retained proof trees/reports as temporary fixtures.

Return exact SHA/tool versions, all test counts/check exits, scenario outcomes,
all Desktop/daemon PID pairs including rejected candidate, parent/path checks,
health/cleanup fields, report absolute path and SHA-256, and any flush/child
cleanup or visible-window issue. Pending intent blocks reuse; interruption
recovery is a subsequent slice, not an expected result of this plan.
