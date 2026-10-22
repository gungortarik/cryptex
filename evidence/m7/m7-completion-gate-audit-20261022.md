# M7 completion-gate audit

**Date:** 2026-10-22\
**Status:** **OPEN / DEFERRED — NOT COMPLETE**\
**Architecture:** Accepted Obsidian controlled write-back is preserved. Managed-
note reads/writes stay disabled until the P1 gate passes. No Actions were run.

## Requirement-by-requirement state

| Requirement | Current evidence | State |
|---|---|---|
| Vault-API physical containment, Windows reparse points and macOS symlinks, including replacement races | Native synthetic probes observed Windows junction and macOS symlink escapes. Public Vault docs document content concurrency behavior but no physical-root guarantee. `lstat` is not race-safe. | **FAIL / BLOCKS M7** |
| Safe edit detection and event lifecycle | Windows synthetic edit/create/rename/delete and hard-unload observations. Mac synthetic edit was observed; hard quit/reopen was needed to unload. Startup counters are metadata-only and not complete parity. | **PARTIAL** |
| `Vault.process` conflict behavior | Windows synthetic manual-edit conflict preserved the manual edit. Owner-reported Mac probe did likewise. | **SCOPED PASS** |
| Projection-loop suppression | Windows synthetic probe observed own-write suppression. Mac loop-suppression behavior was not established. | **PARTIAL** |
| Plugin-to-daemon IPC | Windows native synthetic health IPC and Mac exact-source native health IPC returned `health.pong`. This is status-only; it does not prove proposal authorization or peer identity for a write capability. | **SCOPED PASS** |
| Real-daemon pending-request cleanup on plugin unload | Local held-open socket fixtures pass on both OS reports; actual real-daemon in-flight unload behavior has not been demonstrated. | **NOT VERIFIED** |
| Proposal/review/apply/reject authorization lifecycle | Product proposal operation is not implemented; authenticated proposal authorization was not run. | **NOT IMPLEMENTED / NOT RUN** |
| Cross-platform API/runtime baseline | Windows runtime/API inventory reports 1.14.4. Mac app/API reports 1.14.4 while installer/plist reports 1.13.7; supported baseline is not selected. | **OPEN** |
| Observation fixture safety/regression | Windows rerun on 2026-10-22: syntax check and 18/18 local Node tests pass. Owner-reported Mac run: 18/18 local Node tests pass. These are not containment or native proposal tests. | **PASS, FIXTURES ONLY** |
| Required CI and final review | No GitHub Actions were run. M7 human review and completion audit remain outstanding. | **OPEN** |

## Current-turn verification

- `node --check scripts/spikes/m7-obsidian-observation/main.js`: PASS.
- `node --test scripts/spikes/m7-obsidian-observation/guard.test.cjs scripts/spikes/m7-obsidian-observation/real-socket.test.cjs`: **18/18 PASS** on Windows.
- The focused Rust IPC test was attempted with both the existing and a fresh
  external Cargo target. The existing target failed to load `yoke_derive`; the
  fresh target was blocked by Windows Application Control 4551 when Cargo tried
  to execute the `quote` build script. No security policy was changed and no
  repository build artifacts were removed.
- The tested observation plugin has no `Vault.read`, `cachedRead`, `create`,
  `modify`, `process`, `rename`, or `delete` operation; it remains observation-
  only.

## Evidence references

- `m7-p1-windows-writeback-spike-partial-20261014.md`
- `m7-p1-macos-writeback-spike-partial-20261015.md`
- `m7-p1-observation-portability-windows-20261016.md`
- `m7-p1-observation-portability-macos-20261016.md`
- `m7-p1-macos-health-ipc-20261018.md`
- `m7-p1-macos-observation-lifecycle-20261018.md`
- `m7-p1-vault-api-containment-review-20261016.md`
- `m7-p1-current-obsidian-api-docs-review-20261022.md`
- `m7-p1-windows-observation-fixtures-rerun-20261022.md`

## Next allowed step

Do not productize proposal IPC or enable managed-note I/O yet. Resume the
containment spike only when the API request receives a supported Obsidian
contract or the owner accepts a separately reviewed physical-boundary design
that preserves the selected Obsidian workflow. Then prove that design on both
OSes against the acceptance matrix, finish the real-daemon/proposal tests, and
complete the required verification and review gates.
