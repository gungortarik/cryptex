# Windows changed native package handover — 2026-10-05

**Result: scoped PASS.** Source runner: `309bb66`; Windows x64, Node `v24.19.0`.
M0 P0-2 native pilot, not installed/signed updater acceptance or an ADR decision.

## Inputs and build

Baseline retained input:
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\desktop-session-input-20261005-3efd2eb3a25c42179ddcb6d878e1cdc8`.
Candidate fresh input:
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\changed-package-input-20261005-402618fae836403e980958f46338a291\candidate`.

The current native sidecar was built with locked Cargo. Tauri performed its
TypeScript/Vite frontend build and native release build, all exits 0. A temporary
configuration **outside Git** supplied trial version `0.0.1`; repository version
remains `0.0.0`. Windows PE FileVersion/ProductVersion observation was baseline
`0.0.0` and candidate `0.0.1`. No installer or existing installation was changed.

| Role | Baseline SHA-256 | Candidate SHA-256 |
| --- | --- | --- |
| Desktop | `26556b6c9255911a81d0faf2481a83138efdc05c18698b07a86eecd700609edd` | `79e3e18f7a648ebce23b7b1a8c6ce5052df0dd533a237657fdd8a1677083eec1` |
| Daemon | `0abb57a0e88a3659e181df9988027cf4c7fe040e399da316c7bbd8a62f44c72c` | `a958e52813d5f59561e20e1f80a1d261f3028b9fefc5fa3df093fa1589493658` |

Both actual role payloads differ; the daemon semantic version was not changed.
The trial Desktop version is test metadata, not a declared product release.
An optional candidate input requires at least one actual native-role digest to
change. Identical binaries or changed resources alone cannot claim this proof.

## Actual final run

Root: `%LOCALAPPDATA%\Cryptex-Dev\p0-2\changed-package-proof-20261005-15a74befaa064976b4f14bbb89f1d7f0`.
Report: `native-selection-report.json`, SHA-256:
`2d6e19d8e6e2519a7568347824cc28f20813b11df6688d18365d88152d62cdb5`.

| Session | Desktop PID | Daemon PID | Result |
| --- | ---: | ---: | --- |
| Compatible baseline | 30104 | 11272 | Healthy then clean stop |
| Compatible changed candidate | 33232 | 14804 | `committed`, selection candidate |
| Protocol-reject baseline | 29200 | 20168 | Healthy then clean stop |
| Protocol-rejected changed candidate | 13344 | 9764 | Helper exit 2, expected protocol rejection; clean stop |
| Restored baseline | 32000 | 17336 | `rolled-back`, selection baseline, healthy |

Runner parent **28356** matched reported Desktop parents; daemon parents matched
their owning Desktop. Executable paths resolved to the correct complete slot.
Healthy sessions returned protocol **1**, `health.pong`, matching daemon PID and
`lock_held=true`. Protocol-2 metadata deliberately disagreed with actual protocol
1; it was not made compatible by weakening negotiation.

All five sessions cleanly exited **0**, daemon locks released and actual IPC
endpoint absent. Native gates were released by the scoped owner. No pending
native intent remained. Two deterministic synthetic sealed packs and the Golden
manifest were inventoried before work and after **each** scenario: membership
and all hashes stayed unchanged. No production canonical data was supplied.
An earlier independent run of these changed inputs also passed both scenarios.

## Repository verification

- Combined Node **143/143**, no failures/skips, exit 0, using the retained trial
  Desktop for actual executable CLI boundary tests.
- Storage capture **5/5**, exit 0.
- Release Desktop Rust **17/17**, exit 0.
- Workspace format, strict Clippy, changed Node syntax and diff checks PASS.
- New proof-input tests **6/6** cover unchanged, changed-role, resource-only,
  missing/incomplete/duplicate inputs and overlapping/relative roots.

## Limits

This establishes native staged **changed-byte pair** compatibility and actual
rollback, replacing the earlier same-byte-only limit on Windows. It is not an
installed updater, coordinated semantic-version migration, authenticated/signed
release, power-loss or interruption proof. Existing interruption evidence is
separate. `version_upgrade_verified`, `publisher_authenticated` and
`canonical_data_acceptance` remain false. Windows directory power-loss durability
remains unverified. Hidden-window snapshots passed; physical flash/focus was not
separately observed. Mac parity and ADR-0012 acceptance remain open. No Actions.
