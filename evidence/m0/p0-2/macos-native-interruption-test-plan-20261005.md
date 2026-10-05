# Mac task — integrated M0 foundation and native interruption parity

Use clean current `m0/p0-2-cursor` containing
`7daadaad6cb83ad9753d6ca803e2c8e36e6332b3`. Report full tested SHA.
Read handoff, accepted ADR-0013 and approved native handover pilot plan.
This branch now integrates accepted P0-1; do not rerun old benchmarks or restart
retained P0-1 Syncthing. No Actions, installer, login/tray tests or personal data.
On PASS do not edit/commit/push. Stop and report failures; do not weaken tests,
change policy, flatten links or discard diagnostics.

## One combined verification run

```sh
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked
pnpm --filter @cryptex/desktop tauri:package --bundles app
```

Require all exits 0, no failed/ignored tests or Clippy warnings. Report actual
per-binary Rust counts: Windows was 89 total, but OS-specific tests differ.
Desktop Mac should remain 10 and lifecycle helper 14. Newly merged storage
benchmark has seven unit tests. Current `.app` must contain target arm64
Desktop/daemon. Preserve the source checkout for the compile-time pilot boundary.

```sh
pilot_repo="$(pwd)"
pilot_base="$HOME/Library/Application Support/Cryptex-Dev/p0-2"
pilot_parent="$pilot_base/mac-native-interruption-20261005-$(uuidgen)"
mkdir -p "$pilot_base"
mkdir "$pilot_parent"
mkdir "$pilot_parent/input"
ditto "$pilot_repo/target/release/bundle/macos/Cryptex.app" "$pilot_parent/input/Cryptex.app"
export CRYPTEX_PILOT_DESKTOP_TEST_BINARY="$pilot_parent/input/Cryptex.app/Contents/MacOS/cryptex-desktop"
node --test tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs tools/m0-lifecycle-compare/scripts/package-daemon-health.test.mjs tools/m0-lifecycle-compare/scripts/package-native-recovery.test.mjs tools/m0-lifecycle-compare/scripts/package-native-tree.test.mjs tools/m0-lifecycle-compare/scripts/package-handover-gate.test.mjs tools/m0-lifecycle-compare/scripts/package-desktop-session.test.mjs tools/m0-lifecycle-compare/scripts/package-desktop-pilot-cli.test.mjs tools/m0-lifecycle-compare/scripts/package-native-selection.test.mjs tools/m0-lifecycle-compare/scripts/package-native-interruption.test.mjs tools/m0-lifecycle-compare/scripts/m0-foundation.test.mjs
unset CRYPTEX_PILOT_DESKTOP_TEST_BINARY
node --test tools/m0-storage-bench/scripts/capture-syncthing-p0-1-raw-result.test.mjs
node --check tools/m0-lifecycle-compare/scripts/package-native-selection.mjs
node --check tools/m0-lifecycle-compare/scripts/package-native-interruption.test.mjs
node --check tools/m0-lifecycle-compare/scripts/native-selection-interruption-worker.mjs
node --check tools/m0-lifecycle-compare/scripts/run-native-recovery-proof.mjs
node --check tools/m0-lifecycle-compare/scripts/m0-foundation.mjs
node --check tools/m0-lifecycle-compare/scripts/m0-foundation.test.mjs
git diff --check
```

Require **123/123 + 5/5 Node**, no failures/skips, all exits 0. Native owner-kill
test uses an actual subprocess/native gate but injected sessions. Golden test
generates deterministic sealed synthetic packs with the accepted benchmark codec;
Doctor tests perform actual absent/failing IPC observations. No fixture left.
These suites require no unrelated Cryptex endpoint to be active; do not stop
an unrelated process automatically. Don't run real session suites concurrently.

## Actual updater kill, healthy baseline restart and fixture preservation

With an available GUI session, run the following once in the fresh root. No UI
clicks are needed; report visible flash/focus changes if observed.

```sh
node tools/m0-lifecycle-compare/scripts/run-native-recovery-proof.mjs --source "$pilot_parent/input/Cryptex.app" --root "$pilot_parent/proof" --helper "$pilot_repo/target/debug/m0-lifecycle-compare" --desktop Contents/MacOS/cryptex-desktop --daemon Contents/MacOS/cryptexd
shasum -a 256 "$pilot_parent/proof/native-recovery-report.json"
```

Require exit 0, actual updater kill at `selected-before-candidate-launch`, gate
release/reacquisition, `recovered-baseline`, new correctly owned Desktop/daemon
under baseline slot, protocol-1 real health with held lock and Doctor `healthy`.
The old baseline must already have cleanly stopped before updater kill.
Final exit 0, PIDs gone, daemon lock/native gate free, IPC absent, intent and
recovery claim absent. `synthetic_sealed_fixture_preserved=true` requires exact
membership and hashes for both packs and the fixture manifest before/after.
Retain proof tree/report outside Git; do not treat it as leftover temporary data.

Report SHA, tool versions, all counts/exits, updater/old/new PID pairs and parent/
executable checks, health/Doctor/cleanup/fixture fields, report path and SHA-256,
any POSIX flush failure and physical window observation limits.
This is one pre-launch kill boundary, not live-child killing, power loss,
production canonical schema or version-changing upgrade. ADR-0012 stays PROPOSED.

## Read-only P0-4 preparation inventory

Also report whether Syncthing **2.1.5** (the accepted stack baseline) is available
on Mac, with resolved executable path and version. Do not upgrade it, start it,
read its API key or alter its global/retained config. The old P0-1 Mac 2.1.2
binary is not proof of the pinned P0-4 baseline. This inventory helps prepare the
separate managed-Syncthing spike; it does not count as P0-4 PASS.
