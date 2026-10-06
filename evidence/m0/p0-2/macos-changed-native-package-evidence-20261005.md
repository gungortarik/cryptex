# macOS changed native package handover — 2026-10-05

> **Historical status note (2026-10-05):** Status statements below reflect the state when this dated plan/result was recorded, before M0 closeout. Current M0 status is COMPLETE for the synthetic foundation scope; scoped ADR-0012/0004 are ACCEPTED. This note does not expand this artifact's evidence, satisfy production residuals, or authorize M1. See `evidence/m0/m0-closure-audit-20261005.md`.

**Result: scoped PASS**, reported by the Mac validation executor via the project
owner. Tested SHA: `9a74b40414fad7b34d6e038fbe95eff334820262`, containing `309bb66`.
Windows implementer checked that SHA against repository history and records the
supplied Mac report here; the Mac-local artifact was not opened on Windows.

Environment: macOS **27.0.1 / arm64**, Node **v24.19.0**, Rust/Cargo **1.98.1**,
pnpm **11.28.0**. Locked workspace build, two `.app` builds, new input tests
**6/6**, three syntax checks, format, strict workspace Clippy and diff PASS.
All command exits **0**, no failures/skips.

## Actual input observations

Info.plist versions **0.0.0 → 0.0.1**, both Desktop/daemon binaries arm64.
`changed_roles`: **desktop=true, daemon=false**. Daemon bytes did not change;
this result must not be represented as two changed native components.

| Role | Baseline SHA-256 | Candidate SHA-256 |
| --- | --- | --- |
| Desktop | `093afb6aad9d5d4af1485cac0fc29a539ef6cfd56bc59abeaf21ca07e20c1cda` | `63ee753252c54d8132da3d9887b9947954fd10b479c39146fc248cd3d0254d3f` |
| Daemon | `bfbfa26c6df1934312ba0d483f5351ac759c8f3f96458f8753d5b4a70157feb4` | Same |

## Native sessions

| Session | Desktop PID | Daemon PID | Outcome |
| --- | ---: | ---: | --- |
| Compatible baseline | 80870 | 80881 | Healthy then clean stop |
| Changed candidate | 80892 | 80899 | `committed`, candidate selected |
| Protocol-reject baseline | 80911 | 80918 | Healthy then clean stop |
| Rejected candidate | 80927 | 80937 | Expected helper exit 2, clean stop |
| Restored baseline | 80948 | 80955 | `rolled-back`, baseline selected, healthy |

Desktop parent matched runner **80865**; daemon parents matched their Desktop.
Correct complete-slot executable paths were confirmed. Healthy sessions returned
protocol **1**, `health.pong`, matching PID and `lock_held=true`.
All five sessions clean exit **0**, PIDs absent, daemon locks/native gates free,
IPC absent and no native intent. No flush/cleanup error. Synthetic sealed pack
and manifest inventory matched after **each scenario**.

Report path on Mac:
`/Users/tarikgungor/Library/Application Support/Cryptex-Dev/p0-2/mac-changed-package-20261005-876B270B-BFC2-442E-A9C4-B8059B314CD6/proof/native-selection-report.json`.
Supplied SHA-256:
`ed570f26272f886389b96c7b636956eb7a82af2b96ba6850c8910eb386dce9da`.

`same_source_bytes=false`, `changed_native_package_verified=true`,
`synthetic_sealed_fixture_preserved=true`. Acceptance flags stayed false.
Snapshots showed hidden windows; physical flash/focus was not separately observed.
Mac repo was clean; no source/evidence edits, commit/push, Actions, installation
or startup changes were reported.

## Scope and continuation

Changed native package selection/rollback now has scoped proof on both OSes.
This is not installed updater, coordinated daemon semantic-version migration,
signing/notarization, power-loss or production canonical-schema acceptance.
ADR-0012 remains **PROPOSED**. Separate Mac native interruption/Golden/Doctor and
managed Syncthing parity results have not been supplied by this report.
