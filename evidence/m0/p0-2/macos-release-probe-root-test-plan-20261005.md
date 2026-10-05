# Mac task — M0 release probe development namespace

Use clean current `m0/p0-2-cursor` containing `fc79f25` (and namespace fix
`ad44e82`). Report full tested SHA. Read AGENTS, current handoff, M0 contract
and `windows-release-probe-root-evidence-20261005.md`.

Only the new namespace correction and release-only lint fix are requested.
Ordinary release must resolve to
`~/Library/Application Support/Cryptex-Dev/runtime/p0-2`.
Debug keeps `Cryptex-Dev/p0-2/compare/mode-b-desktop`; validated pilot roots and
their isolation remain unchanged. No old product runtime migration is intended.

Run in order; stop at first failure and report diagnostics:

```sh
cargo fmt --all -- --check
cargo test --locked -p cryptex-desktop
cargo test --locked --release -p cryptex-desktop
cargo clippy --locked -p cryptex-desktop --all-targets -- -D warnings
cargo clippy --locked --release -p cryptex-desktop --all-targets -- -D warnings
cargo build --locked --release -p cryptex-desktop
CRYPTEX_PILOT_DESKTOP_TEST_BINARY="$PWD/target/release/cryptex-desktop" node --test tools/m0-lifecycle-compare/scripts/package-desktop-pilot-cli.test.mjs
git diff --check
```

Expected Mac: Desktop **14/14 debug and 14/14 release** (previous 13 plus the
actual compiled-default resolver regression); CLI **4/4**. No failures/skips/
ignored; every exit 0. The shared compiled-default test must execute in BOTH
profiles. Report release helper path and missing-HOME checks, retained sidecar
and pilot-root validation, fixture cleanup and repo state.

These tests resolve paths without creating normal runtime data and reject CLI
requests before Tauri startup. No physical renderer/login/tray session needed.
Do not repeat Syncthing, native handover/recovery, Golden/Doctor or physical
login/tray plans. Do not open the app/daemon, move/delete old runtime data,
modify startup/global config, run Actions or change installation/security policy.
No frontend change; no redundant frontend/.app packaging run requested.

On PASS report only: no source/evidence edits, commit or push. Include full SHA,
platform/architecture, Rust/Cargo/Node versions, all counts/exits and cleanup.
This does not prove installed update, console-free login, signed distribution,
production canonical safety or human ADR acceptance. Final CI is separate.
