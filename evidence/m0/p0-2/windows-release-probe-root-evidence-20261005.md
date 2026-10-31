# Windows M0 release probe development root

**Result: scoped PASS.** Namespace source `ad44e82cb42464b0dbcbf3d4c852659a6e5ae49a`;
final tested code `fc79f25bb9b7b9a21f056a0e0ec813f738d33d58`.
Windows x64, Rust/Cargo 1.98.1. Corrects the concrete finding in
`../m0-final-gate-audit-20261005.md`.

Ordinary M0 release Desktop now resolves:

- Windows: `%LOCALAPPDATA%/Cryptex-Dev/runtime/p0-2`
- macOS: `~/Library/Application Support/Cryptex-Dev/runtime/p0-2` (Mac parity pending)

Debug retains its Cryptex-Dev comparison root. A validated pilot root continues
to take precedence; its validation, separate identity and startup prohibition
are unchanged. No product paths in the Master Spec were changed. Historical
product-root proof data and installed binaries/registrations were not migrated,
removed or replaced. Restarting an older installed build still uses its older
behavior; this is current-source proof, not installed-update acceptance.

## Verification

| Command | Final result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo test --locked -p cryptex-desktop` | 21/21 PASS |
| `cargo test --locked --release -p cryptex-desktop` | 21/21 PASS |
| Strict Desktop Clippy, debug and release, all targets | PASS |
| Locked release Desktop build | PASS |
| CLI Node with newly rebuilt release Desktop | 4/4 PASS |
| `git diff --check` | PASS |

Every final command exit 0; no failed, skipped or ignored tests. The new
`compiled_default_uses_development_namespace_in_debug_and_release` test calls
the actual compiled default resolver in both profiles, checks the full OS
app-data path and never creates a runtime or mutates environment variables.
Release helper/missing-variable tests were updated to the M0 development-root
requirement. Existing pilot/sidecar validation tests remain enabled.

Initial extra strict release Clippy exited 101 for a pre-existing
`collapsible_if` warning in release-only startup-error recording. `fc79f25`
uses a let-chain preserving the same error-recording condition/body; no lint
allow or test weakening. Both full test profiles, both strict lint profiles,
release build and CLI checks were rerun successfully afterward.

Final CI coverage now explicitly runs the release default regression and strict
release Desktop lint on both OS jobs, alongside existing debug/workspace and
Node coverage. No workflow was dispatched. No frontend behavior changed, so its
previous build/visual evidence is retained without a redundant build.

This is path resolver/build/negative CLI proof, not physical login, renderer
invocation or a visible runtime session. No visible app, daemon, startup/global
configuration change or unrelated process termination occurred. Mac task:
`macos-release-probe-root-test-plan-20261005.md`. Final CI and human ADR/M0 gates
remain open.
