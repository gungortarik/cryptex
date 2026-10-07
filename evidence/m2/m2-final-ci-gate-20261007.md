# M2 final dual-OS CI gate — 2026-10-07

**Status:** CI gate PASS (overall success). M2 remains **IN PROGRESS** — owner review pending. Do **not** claim M2 complete or merge from this record alone.

**Branch:** `m2/immutable-event-store`  
**Tested SHA:** `16f09a3830e743694d8dba52c16a816c93d46a16`  
**Workflow:** CI (`ci.yml`)  
**Run:** [37681630801](https://github.com/gungortarik/cryptex/actions/runs/37681630801)  
**Overall conclusion:** `success`  
**Completed:** 2026-10-07T20:47:07Z (created 2026-10-07T20:23:29Z)

## Job outcomes (exact)

| Job | Conclusion | Notes |
|---|---|---|
| Readiness (macos-latest) | **success** | Dual-OS readiness matrix |
| Readiness (windows-latest) | **success** | Dual-OS readiness matrix |
| IPC cross-account ACL (Windows) | **success** | Windows ACL denial path |
| Windows Desktop package (unsigned, manual only) | **skipped** | Skipped by workflow design (manual `workflow_dispatch` package job; not required for this gate) |

## Scope of this record

This evidence records the **final dual-OS Actions gate** on the post-hardening tip that includes:

- Windows quarantine extended-path adapter + regressions (slices 8g/8h)
- Pre-Actions security/DoD review + in-scope fixes (`evidence/m2/m2-security-dod-review-20261007.md`)
- cryptexd lib exclusive test-temp hardening (slices 8i/8j) at tip `16f09a3`

It does **not** by itself close M2. Per `docs/21_DEFINITION_OF_DONE.md` and the M2 milestone contract, owner / human review remains required before M2 may be called complete.

## Preserved caveats

- **`LongPathsEnabled=0` is not claimed.** Extended-path adapter verification observed `LongPathsEnabled=1` on the Windows implementer host only; portable success under `LongPathsEnabled=0` was not measured and must not be inferred from this CI run.
- Unsigned Windows Desktop package job remains out of this gate by design (skipped).
- M2 must **not** be merged and M3 must **not** start until owner review accepts this tip (and any remaining DoD items the owner requires).

## Authority cross-check

- `AGENTS.md` / `docs/00_MASTER_SPEC.md` — documentation-only update; no product code change in the commit that adds this file.
- `docs/milestones/M2_IMMUTABLE_EVENT_STORE.md` — cross-platform CI evidence satisfied for the readiness jobs above; completion still needs human review.
- `docs/21_DEFINITION_OF_DONE.md` — required CI green recorded; owner review still open.
