# Mac task — actual daemon health gate for the package candidate

**Status:** scoped PASS at `1e07ca406db4bfb2f39029e8fda82e5d936d4dc8`;
3/3 headless real-health scenarios and all listed checks passed. Evidence:
`macos-package-real-health-evidence-20261003.md`. Production acceptance limits
below remain open.
**Scope:** M0 P0-2 headless candidate health integration, not installed updater.

Mac Codex: read the current handoff and candidate proposal, then record the exact
source SHA of the commit containing this plan. Preserve prior Mac evidence.
Use the pinned existing toolchain; no dependency upgrades or GitHub Actions.

## Required checks

```sh
cargo fmt --all -- --check
cargo test -p m0-lifecycle-compare
cargo clippy -p m0-lifecycle-compare --all-targets -- -D warnings
cargo build -p m0-lifecycle-compare
node --test tools/m0-lifecycle-compare/scripts/package-daemon-health.test.mjs
node --check tools/m0-lifecycle-compare/scripts/package-daemon-health.mjs
node --check tools/m0-lifecycle-compare/scripts/run-package-real-health-proof.mjs
git diff --check
```

Expect **10 Rust tests**, **4 Node adapter tests**, zero failures/skips and no
Clippy warnings. Do not repeat the 29 already passed filesystem tests or GUI
login/tray/Stop suites; no Desktop rebuild is needed.

## Real-health run

1. Resolve absolute paths to an existing target-matched release Desktop and
   daemon from retained artifacts. Record both SHA-256 hashes and source identity.
   The runner stages Desktop bytes only, never opens the GUI. If only one pair
   exists, supply it as both baseline and candidate and label the identical hashes;
   this verifies health/recovery, not a Mac version-changing package upgrade.
   If no retained daemon exists, build only `cryptexd` locally and record that
   provenance; do not claim a signed packaged pair.
2. The shared daemon IPC endpoint must be unused. The helper will refuse an
   active/ambiguous endpoint. Do not kill unrelated/user processes or overwrite
   their runtime. If an installed Cryptex is active, report the blocker and ask
   the operator for ordinary clean Quit; it is not a new physical lifecycle test.
3. Run `node tools/m0-lifecycle-compare/scripts/run-package-real-health-proof.mjs`
   with absolute paths for all six flags:
   `--root`, `--health-helper`, `--baseline-desktop`, `--baseline-daemon`,
   `--candidate-desktop`, `--candidate-daemon`.
   Helper is the newly built `target/debug/m0-lifecycle-compare` executable.
   Root must be a fresh directory under
   `~/Library/Application Support/Cryptex-Dev/p0-2/`; its parent must exist.
   Do not use installed/product runtime or retained P0-1 corpora.
4. Expect **3 scenarios**: compatible actual daemon health, non-executable daemon
   failure with baseline restoration, declared `2/2` pair versus actual protocol-1
   daemon rejection with baseline restoration. The invalid executable error code
   may differ by OS; retain its actual code. No assertion should require Windows
   `EFTYPE`. The real protocol mismatch must show expected 2/observed 1.
5. All four successful IPC observations must show Desktop role, matching daemon
   PID/held lock, protocol 1, active endpoint refusal, clean exit 0 and released
   lock. The rejected protocol candidate must also cleanly exit and release its
   lock. Confirm unchanged synthetic fixture hash and no owned child left.
   Preserve the full `real-health-report.json` outside Git; record its path.
6. Record exact results/provenance in a new
   `macos-package-real-health-evidence-20261003.md`, update evidence index and
   current handoff, then commit/push. Report source and evidence commit SHAs.

## Acceptance limits

This is actual daemon lifecycle and the shared Rust Desktop IPC client, not GUI
execution or installed native updater handover. Each health probe uses a fresh
derived runtime. The protocol-negative control compares real response metadata;
it does not implement protocol 2. Restored startup is driven by the experiment.
Synthetic byte preservation is not production canonical-event acceptance.
Signing/resources, startup integration, native successor launch, concurrency,
power-loss durability and ADR-0012 acceptance remain open.
