# Mac task — actual isolated Desktop and bundled daemon session parity

Read handoff and approved native-package-handover pilot plan. Use clean current
`m0/p0-2-cursor` containing the test compilation fix
`49e4e5d2342a8714ea92554cbf3f5130235537f8`.
Report full tested SHA, macOS/architecture and pinned tool versions.

The initial attempt at `8152267bff436c3ec5ddeeb8066da7c32e3bffa3` stopped
before tests ran with duplicate `_EMBED_INFO_PLIST` symbols. The config test now
uses a plain two-window Config fixture; only main generates the Tauri context.
Preserve that failed attempt in `macos-owned-desktop-session-evidence-20261005.md`.
Restart this entire plan from the build checks on the updated clean source;
the Windows 17/17 unit result does not close Mac verification.

This plan builds and launches the current-source Desktop `.app` executable with
an initially hidden window in fresh pilot runtimes. Use a host with an available
interactive GUI session. No UI clicks, installer, existing login registration,
production canonical data or retained corpora. No Actions. Do not substitute a
legacy binary lacking the pilot flag, reuse a proof root or flatten package links.
No source/evidence edit, commit/push or dependency upgrade is authorized on PASS.
Stop and report any security barrier or failure without changing policy/tests.

## Build and Rust checks

```sh
cargo fmt --all -- --check
cargo test -p cryptex-desktop -p m0-lifecycle-compare --locked
cargo clippy -p cryptex-desktop -p m0-lifecycle-compare --all-targets --locked -- -D warnings
cargo build -p m0-lifecycle-compare --locked
pnpm --filter @cryptex/desktop tauri:package --bundles app
```

Expected on Mac: Desktop **10/10** (including six pilot-isolation tests), lifecycle
helper **14/14**, no failures/ignored tests and no Clippy warnings. Report actual
counts; any discrepancy must be explained, not silently accepted. The fresh `.app`
must include `Contents/MacOS/cryptex-desktop` and the target-matched bundled
`Contents/MacOS/cryptexd`. Verify paths before following the commands below.

## Fresh native package input and Node checks

Run from repository root; preserve the source checkout because the pilot validates
its compile-time repository boundary. These variables never overwrite HOME.

```sh
pilot_repo="$(pwd)"
pilot_base="$HOME/Library/Application Support/Cryptex-Dev/p0-2"
pilot_parent="$pilot_base/mac-owned-session-20261005-$(uuidgen)"
mkdir -p "$pilot_base"
mkdir "$pilot_parent"
mkdir "$pilot_parent/input"
ditto "$pilot_repo/target/release/bundle/macos/Cryptex.app" "$pilot_parent/input/Cryptex.app"
export CRYPTEX_PILOT_DESKTOP_TEST_BINARY="$pilot_parent/input/Cryptex.app/Contents/MacOS/cryptex-desktop"
node --test tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs tools/m0-lifecycle-compare/scripts/package-daemon-health.test.mjs tools/m0-lifecycle-compare/scripts/package-native-recovery.test.mjs tools/m0-lifecycle-compare/scripts/package-native-tree.test.mjs tools/m0-lifecycle-compare/scripts/package-handover-gate.test.mjs tools/m0-lifecycle-compare/scripts/package-desktop-session.test.mjs tools/m0-lifecycle-compare/scripts/package-desktop-pilot-cli.test.mjs
unset CRYPTEX_PILOT_DESKTOP_TEST_BINARY
node --check tools/m0-lifecycle-compare/scripts/package-desktop-session.mjs
node --check tools/m0-lifecycle-compare/scripts/package-process-identity.mjs
node --check tools/m0-lifecycle-compare/scripts/run-desktop-session-proof.mjs
node --check tools/m0-lifecycle-compare/scripts/package-desktop-session.test.mjs
node --check tools/m0-lifecycle-compare/scripts/package-desktop-pilot-cli.test.mjs
git diff --check
```

Expected Node **97/97**, zero failures/skips, exit 0. Seven new session tests cover
preflight refusal, read-only process-parent/executable identity and completed
Desktop snapshot readiness. Four actual Desktop CLI tests must reject invalid,
reused, linked, repository and relative roots before starting Tauri/daemon.
Failing tests must stop this plan; don't continue to live session proof.

## Actual `.app` session proof

Confirm the shared IPC endpoint is absent. The runner also refuses an active or
ambiguous endpoint before every launch; never stop an unrelated process to free it.

```sh
node tools/m0-lifecycle-compare/scripts/run-desktop-session-proof.mjs --source "$pilot_parent/input/Cryptex.app" --root "$pilot_parent/proof" --helper "$pilot_repo/target/debug/m0-lifecycle-compare" --desktop Contents/MacOS/cryptex-desktop --daemon Contents/MacOS/cryptexd
```

Require exit 0 and retained `proof/desktop-session-report.json`. Two sessions must
each show the owned Desktop PID, bundled daemon PID, actual OS parent/executable
paths, protocol-1 health.pong and held lock. Both must cleanly exit 0 with daemon
PID gone, released daemon lock and absent IPC endpoint. The native gate spans
both sessions. No owned child/native lock may remain; keep the input/proof/report
outside Git. Report any abnormal shutdown or cleanup error, never delete evidence
to hide it. If process identity adapter/path or resources fail, preserve the
report/diagnostic and stop rather than weakening checks.

Return exact SHA, all counts/exits, both process pairs, health/cleanup fields,
report absolute path and SHA-256, and any visible window/focus disturbance. Do
not repeat tray/login tests. This is **same-package baseline reopen**, not a
version upgrade, candidate rollback or handover transaction. Publisher signing,
canonical-data safety, power loss and ADR-0012 acceptance remain open.
