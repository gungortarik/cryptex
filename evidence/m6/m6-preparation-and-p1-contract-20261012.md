# M6 preparation and P1 attachment projection contract

**Status:** Preparation started after owner-approved M5 closeout, 2026-10-12. M6 implementation and P1 results are not claimed complete.

## Authority and starting point

Follow `docs/00_MASTER_SPEC.md`, `docs/milestones/M6_OBSIDIAN_MATERIALIZER.md`, `docs/13_OBSIDIAN_INTEGRATION.md`, `docs/23_REQUIRED_TECHNICAL_SPIKES.md`, platform requirements, and DoD. M5 closeout is recorded in `evidence/m5/m5-final-dod-security-ui-review-20261012.md`.

Preparation remains on the existing `codex/m5-capture-timeline` checkout. M5 has not been merged; branch/integration strategy is not changed by this document. GitHub Actions must not run. Preserve unrelated local scratch files.

## First deliverable: reproducible P1 benchmark

Build a standalone local harness using only generated synthetic attachment bytes under an explicit unique runtime outside Git. Record OS, filesystem/volume, sizes, elapsed time, bytes copied, link/fallback outcomes, cleanup results, and source-byte checks before/after. No personal vault or canonical storage is used.

Required matrix on Windows and macOS:

| Scenario | Required observation |
|---|---|
| Same-volume hard link and copy | Measure both; verify projected bytes and source remains unchanged after unlinking derived output. |
| Cross-volume link attempt | Where a second volume exists, record actual failure and copy fallback. If unavailable, mark this case unavailable; injected failures do not establish native cross-volume PASS. |
| Unsupported/denied linking | Distinguish link failure from source read or destination write failure; never mask failed copies as success. |
| Empty, small, and large fixtures | Report sizes and separate timings; no guessed universal threshold. |
| Existing destination / interrupted copy | Preserve existing unrelated files; partial derived output must not become accepted output. |
| Rebuild and cleanup | Only remove harness-owned generated output; retain source fixtures and prove byte identity. |
| Mutation through projected hard link | Demonstrate shared-inode mutation using disposable fixtures; use the result when deciding whether hard links can preserve source integrity. |
| Path hazards | Exercise spaces, Unicode, case collisions, reserved Windows names, traversal, and long paths through safe generated destinations. |

The experiment compares hard links with copies; it does not preselect hard links for production. A writable derived hard link may mutate its source. Policy must preserve canonical immutability and be selected from evidence before attachment projection is finalized.

## Subsequent M6 implementation

After the spike, specify deterministic generated names, escaped frontmatter/links, managed-output ownership, safe rebuild publication, and a stable synthetic attachment input boundary. Daemon-controlled generation and refresh must honor current RetrievalSafetyState. Plan complete generated-vault coverage from supported accepted events, then Open Vault/Obsidian Graph integration. Unknown schema, unsafe paths, corruption, and unavailable safety state require explicit typed failures.

M7 plugin/write-back and M9 canonical blob storage, deduplication, reference tracking, and repair remain outside M6. Production v2 writing stays disabled.

## Verification policy

Add focused tests with behavior changes; run local format/lint/relevant tests before claiming completion. Consolidate platform runs to avoid repeated suites. No Actions dispatch, workflow changes, push, or merge is needed for this preparation. Final M6 CI policy must be resolved against the milestone gate and owner prohibition before declaring M6 complete; this document does not waive that gate.

**Next action:** implement the M6 synthetic attachment projection using the copy-only policy in `m6-p1-attachment-projection-decision-20261013.md`. Mac cross-volume behavior was not run; Windows cross-volume fallback passed. Keep the Mac limitation explicit in later evidence.

## 2026-10-13 continuation update

The preparation action above is complete: the renderer, safe rebuild, copy-only attachment boundary, daemon/Desktop vault IPC, and Open Vault/Graph path are implemented locally. Packaged Windows native verification now passes for generated-note opening and Graph. See `m6-slice1-v1-note-renderer-20261013.md`, `m6-slice2-safe-vault-rebuild-and-copy-projection-20261013.md`, `m6-slice3-desktop-vault-ipc-20261013.md`, and `m6-final-dod-audit-20261013.md`.

M6 remains open pending macOS implementation verification, Mac cross-volume P1 availability/evidence, required CI resolution against the owner's no-Actions instruction, and final human review. No claim of M6 completion is made.
