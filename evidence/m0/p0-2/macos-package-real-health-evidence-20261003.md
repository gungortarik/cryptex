# macOS package candidate real daemon health evidence

**Result:** 3/3 real health scenarios PASS; Mac helper checks PASS.
**Plan source:** `1e07ca406db4bfb2f39029e8fda82e5d936d4dc8`.
**Host:** macOS 27.0.1 arm64; Node v24.19.0.
**Candidate proposal:** `docs/adr/ADR-0012-update-orchestration-candidate.md`,
which remains PROPOSED.

## Build and local checks

Executed against the plan source with the pinned existing toolchain:

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS, exit 0 |
| `cargo test -p m0-lifecycle-compare` | PASS, 10 passed, 0 failed |
| `cargo clippy -p m0-lifecycle-compare --all-targets -- -D warnings` | PASS, exit 0, no warnings |
| `cargo build -p m0-lifecycle-compare` | PASS, exit 0 |
| `node --test tools/m0-lifecycle-compare/scripts/package-daemon-health.test.mjs` | PASS, 4 passed, 0 failed/skipped |
| Node syntax checks for `package-daemon-health.mjs` and `run-package-real-health-proof.mjs` | PASS, exit 0 |
| `git diff --check` | PASS, exit 0 |

## Binary provenance and inputs

The existing target-matched macOS arm64 release pair was retained from
`c77479ffb82b2fc697a2f4d16fb3e3a6bd19f4d7`, the package-smoke/release source
identified in `macos-tauri-owned-supervisor-restart-evidence-20261003.md`.
The same pair was supplied as baseline and candidate, so this verifies real
daemon health and recovery, not a version-changing Mac package update.

| Input | Absolute path | SHA-256 |
| --- | --- | --- |
| Release Desktop | `/Users/tarikgungor/github_repos/cryptex/target/release/cryptex-desktop` | `0bfd52594a4a30bfcd6e37678835bb7d833cfe6be867f44996e45e18de36f1a5` |
| Release daemon | `/Users/tarikgungor/github_repos/cryptex/target/release/cryptexd` | `bfbfa26c6df1934312ba0d483f5351ac759c8f3f96458f8753d5b4a70157feb4` |
| Health helper | `/Users/tarikgungor/github_repos/cryptex/target/debug/m0-lifecycle-compare` | `a735921f54896d57c2748b5ce1284ca7baa692efd5cb764121803d160b65067c` |

Before starting a daemon, the helper proved the shared IPC endpoint absent.
The in-sandbox probe could not inspect the local socket (`Operation not
permitted`), so the exact read-only endpoint preflight was repeated with host
socket access and returned `{"endpoint_absent":true,"schema":1}`. The runner
also checks for an absent endpoint before each owned daemon launch and refuses
an active/ambiguous endpoint.

## Actual health and rollback scenarios

The runner was invoked with absolute paths for all six required flags. Its
fresh root and the complete report are outside Git:

`/Users/tarikgungor/Library/Application Support/Cryptex-Dev/p0-2/mac-package-real-health-20261003-b15a85f/`

Full JSON report:
`/Users/tarikgungor/Library/Application Support/Cryptex-Dev/p0-2/mac-package-real-health-20261003-b15a85f/real-health-report.json`

Report SHA-256: `2caf9f48e7be2eb99d8f63a5a5625f7a9c465fb38bd668f33b09a178eafe92bc`.

| Scenario | Outcome |
| --- | --- |
| Compatible protocol 1 baseline and candidate | Real shared Rust Desktop IPC returned `health.pong`, `client_role=desktop`, matching owned daemon PID, protocol 1 and held lock. Active endpoint preflight was refused. Both daemon runs exited 0 and released their locks; candidate became active. |
| Non-executable daemon candidate | Rejected as `DAEMON_START_FAILED`; macOS reported child exit `-13` (no signal). Baseline was restored and passed real IPC health; clean exit 0 and lock release verified. |
| Declared protocol 2/2 pair with actual protocol 1 daemon | Rejected with expected protocol 2 / observed protocol 1. Owned daemon exited 0 and released its lock. Baseline was restored and passed real IPC health; candidate was not selected. |

Four successful IPC observations reported the exact child PID, Desktop role,
protocol 1, held lock and `health.pong`; all their active-endpoint checks
refused as expected. The rejected protocol candidate also stopped cleanly and
released its lock. The synthetic fixture SHA-256 was unchanged:
`a21d61aabca6c81f17fff51cec680281df4b0ed45768710e2257793ccd5d52aa` before
and after.

The report records `desktop_gui_executed=false`,
`installed_updater_integrated=false` and `canonical_data_acceptance=false`.
The Desktop bytes were staged, never executed. Post-run lifecycle status showed
all five successfully started daemon runtimes as `instance=stale` with no live
PID, and the invalid executable runtime as `instance=absent`. A process query
found none of the five owned daemon PIDs. The shared endpoint was absent again
after the run.

## Scope and remaining gates

The helper and test suite ran on the source commit containing the real-health
candidate implementation; runtime binaries came from the earlier documented
Mac release pair and were byte-identical baseline/candidate inputs. No daemon
build was needed. The retained report contains exact PIDs, error text, protocol
observations and runtime paths.

This proves actual daemon lifecycle and shared Rust Desktop IPC health in the
headless candidate harness. It does not prove a Mac version-changing package
upgrade, GUI execution, installed native updater handover, signed package or
resource integrity, successor launch, concurrency, power-loss durability, or
production canonical-data preservation. ADR-0012 remains PROPOSED. No Desktop
rebuild, physical login/tray/Stop repetition or GitHub Actions run occurred.
