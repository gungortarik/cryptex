# Mac task — renderer sidecar launch boundary

Use clean current `m0/p0-2-cursor` containing `82c4d32`; report full tested SHA.
Read handoff, AGENTS, Master Spec and
`evidence/m0/m0-security-notice-acceptance-review-20261005.md`.
The corrected command now validates the renderer request against the backend
sidecar in every mode, before runtime creation/launch. The displayed path is
read-only. This addresses the normal-session arbitrary executable finding.

Only this new verification is requested. Do not repeat Syncthing, login/tray,
native selection/recovery or previous Golden/Doctor live plans. No Actions,
installer, startup, global config or personal data changes. On PASS report only;
no file edits/commit/push. Stop/report first failure and preserve diagnostics.

```sh
cargo fmt --all -- --check
cargo test --locked -p cryptex-desktop
cargo clippy --locked -p cryptex-desktop --all-targets -- -D warnings
pnpm --filter @cryptex/desktop build
cargo build --locked --release -p cryptex-desktop
CRYPTEX_PILOT_DESKTOP_TEST_BINARY="$PWD/target/release/cryptex-desktop" node --test tools/m0-lifecycle-compare/scripts/package-desktop-pilot-cli.test.mjs
git diff --check
```

Expected Mac Desktop **13/13** (previous 10 plus three boundary tests), CLI Node
**4/4**, no failure/skip, every exit 0. The alias test must create/refuse a real
POSIX symlink. Tests cover exact backend path, other executable, relative,
directory/missing paths, backend developer override authority and alias refusal.
The CLI tests reject invalid requests before Tauri startup; no visible app/daemon
or physical user interaction is required. Fixture cleanup must complete.

Report full source, platform/architecture, Rust/Cargo/Node/pnpm versions, counts,
command exits, fixture cleanup and clean repo state. Do not claim a physical
renderer invocation or repeat old runtime proofs to substitute for these tests.
ADR/M0 human acceptance, distribution/signing and final CI remain separate.
