# Mac task — changed native package handover and rollback

> **Historical status note (2026-10-05):** Status statements below reflect the state when this dated plan/result was recorded, before M0 closeout. Current M0 status is COMPLETE for the synthetic foundation scope; scoped ADR-0012/0004 are ACCEPTED. This note does not expand this artifact's evidence, satisfy production residuals, or authorize M1. See `evidence/m0/m0-closure-audit-20261005.md`.

Use clean current `m0/p0-2-cursor` containing `309bb66`; report full tested SHA.
Read handoff, M0/P0-2 contract and Windows changed-package evidence. This is an
additional scope to native interruption/Golden/Doctor and managed Syncthing plans;
it does not replace either. Reuse their current-source baseline build only if its
input copy is still intact; no need to repeat their completed tests.

No Actions, installer, startup/tray setting, personal data or old P0-1 corpus.
Never stop unrelated processes. On PASS report only, without edits/commit/push.
On failure stop/report exact command/exit and retain diagnostics; do not weaken
checks, flatten `.app` links or silently change policy. An unrelated active
Cryptex endpoint must be reported rather than stopped automatically.

## Builds and two retained inputs

Run locked workspace build if the current helper/accepted storage codec is not
already built. Need both `target/debug/m0-lifecycle-compare` and `m0-storage-bench`.
Create a fresh parent under application support, then baseline and candidate
input directories, separate from the future proof root.

```sh
trial_repo="$(pwd)"
trial_base="$HOME/Library/Application Support/Cryptex-Dev/p0-2"
mkdir -p "$trial_base"
trial_parent="$trial_base/mac-changed-package-20261005-$(uuidgen)"
mkdir "$trial_parent"
mkdir "$trial_parent/baseline" "$trial_parent/candidate"
cargo build --workspace --locked
pnpm --filter @cryptex/desktop tauri:package --bundles app
ditto "$trial_repo/target/release/bundle/macos/Cryptex.app" "$trial_parent/baseline/Cryptex.app"
printf '%s\n' '{"version":"0.0.1"}' > "$trial_parent/candidate-tauri-config.json"
pnpm --filter @cryptex/desktop tauri:package --bundles app --config "$trial_parent/candidate-tauri-config.json"
ditto "$trial_repo/target/release/bundle/macos/Cryptex.app" "$trial_parent/candidate/Cryptex.app"
```

The temporary version override is a **test label**, not a repository version
change or release. Do not install either `.app` or enable its startup registration.
Require both builds exit 0 and arm64 Desktop/daemon files. Record both app
Info.plist version observations and role SHA-256 hashes. A fresh same-source
daemon may have identical bytes; report actual `changed_roles`. The Desktop
version override must yield different Desktop bytes. Do not claim both changed
if the daemon remained identical, or treat a resource-only change as a native
component update.

## New checks and actual sessions

```sh
node --test tools/m0-lifecycle-compare/scripts/native-selection-proof-inputs.test.mjs
node --check tools/m0-lifecycle-compare/scripts/native-selection-proof-inputs.mjs
node --check tools/m0-lifecycle-compare/scripts/native-selection-proof-inputs.test.mjs
node --check tools/m0-lifecycle-compare/scripts/run-native-selection-proof.mjs
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
node tools/m0-lifecycle-compare/scripts/run-native-selection-proof.mjs --source "$trial_parent/baseline/Cryptex.app" --candidate-source "$trial_parent/candidate/Cryptex.app" --root "$trial_parent/proof" --helper "$trial_repo/target/debug/m0-lifecycle-compare" --desktop Contents/MacOS/cryptex-desktop --daemon Contents/MacOS/cryptexd
shasum -a 256 "$trial_parent/proof/native-selection-report.json"
```

Require **6/6**, no failures/skips, all check/runner exits 0. The runner must
report `same_source_bytes=false`, `changed_native_package_verified=true`, actual
input role digests and changed-role booleans. Compatible scenario commits the
changed candidate with actual protocol-1 health. Protocol-reject scenario
deliberately expects 2, observes actual 1, cleanly rejects the candidate (helper
exit 2 is expected), restores baseline and proves its actual health.

Require correct slot paths/parent chains for baseline, candidate and restored
sessions; healthy daemon PID, `health.pong`, protocol 1 and held lock. All five
owned Desktop/daemon sessions must cleanly exit 0; PIDs gone, locks/gates released,
IPC absent and no native intent. The accepted-codec synthetic sealed pack and
manifest inventory must match before/after **each** scenario;
`synthetic_sealed_fixture_preserved=true`. Any POSIX flush/cleanup error is failure.
Record source/platform/tool versions, counts/exits, five PID pairs, identity/health
observations, report path/hash and any observed physical flash/focus limits.

This proves a changed native package in test slots. It does not prove installed
updater migration, signing/notarization, canonical production schema, power loss
or version-changing daemon semantics. Keep ADR-0012 PROPOSED and those report
acceptance flags false.
