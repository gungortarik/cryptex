# Native Desktop/daemon handover pilot — reviewable continuation plan

**Status:** READY FOR SCOPED METHOD APPROVAL; implementation not started
**Source reviewed:** `bb058d7`
**Milestone:** M0 P0-2; ADR-0012 remains PROPOSED

## Decision requested

Authorize a test-only native handover pilot for the complete-package slot
candidate in `docs/adr/ADR-0012-update-orchestration-candidate.md`. This extends
the measured filesystem experiment to actual Desktop ownership and native
package resources. It does not select a shipped updater, change the installed
application, accept ADR-0012 or authorize M1.

The handoff requires explicit approval before selecting updater orchestration.
Mode B supervision was approved; the update mechanism was not. Broad permission
to continue work is not recorded here as that missing method decision.

## Evidence and recommendation

| Method | Recorded evidence | Remaining problem | Recommendation |
| --- | --- | --- | --- |
| Stock installer replacement | Windows unsigned 0.0.0 to 0.0.1 and operator rollback | Sequential component copy; interrupted pair and automatic rollback unproved | Keep as distribution evidence |
| Separate component replacement | No complete native handover proof | Mixed-pair states require additional recovery paths | Do not add this experiment now |
| Complete package slots plus selection record | Windows/Mac transaction, recovery, process races and native claim cleanup; real daemon/shared Desktop IPC | Desktop launch, native resources, process exclusion across handover and signed package integrity | Extend this candidate with a bounded pilot |

The current `desktop.bin`/`daemon.bin` fixture cannot represent a complete `.app`
or installed resource tree. Keep existing schema/tests intact. Define a separate
versioned pilot package inventory for native trees and explicitly reject unknown
schemas, missing entries, target mismatch, links/reparse points and extra files.
This inventory proves correspondence to supplied bytes; publisher authentication
must remain a separate result.

## First implementation slice after approval

1. Add a test-only helper/launcher behind Windows and macOS platform adapters.
   Reuse pinned dependencies. It operates only on fresh, marked Cryptex-Dev roots
   outside Git and owns only the children it starts.
2. Stage baseline and candidate complete target-matched package trees. Preserve
   executable permission information on macOS and all resource paths. Record
   source SHA, build mode, platform, versions and hashes. Never overwrite a
   running Desktop or edit the retained source packages.
3. Use a persistent OS lock for the entire pilot handover, not only stale-claim
   cleanup. Define how startup/recovery contenders acquire the same gate; an
   uncoordinated caller must not launch during a transition. Busy/ambiguous state
   causes typed refusal. Never unlink the lock file while it can be held.
4. Request clean shutdown of the owned Desktop, suppress its daemon restart,
   wait for both child exits, daemon lock release and IPC endpoint absence.
   Timeout/forced shutdown is a failed proof and must not advance selection.
5. Verify the candidate tree again, activate the complete selection, launch its
   Desktop with its bundled daemon, and measure actual shared IPC readiness and
   Desktop/daemon PID association. Metadata agreement alone is insufficient.
6. Failed readiness must quiesce the owned candidate, restore the complete
   baseline selection, restart baseline Desktop and prove its IPC readiness.
   Failure to stop or restore retains diagnostics/journal and reports a
   controlled error; it must not claim rollback success.

First deliverable is local helper/unit/integration evidence with an exact Mac
parity plan. Actual GUI launch is scheduled with the operator to preserve focus.
Do not expose the pilot through renderer shell/filesystem capabilities or an AI
recovery tool. Do not add a background startup registration in this slice.

## Required pilot measurements

| Scenario | Required outcome |
| --- | --- |
| Compatible complete pair | Old children exit, candidate Desktop owns candidate daemon, IPC succeeds |
| Real protocol mismatch or missing resource | Reject before activation where detectable; otherwise restore healthy baseline |
| Candidate fails to start/become healthy | Candidate stops; baseline restarts with IPC health, or explicit retained failure |
| Second handover/startup contender | Refuse without changing winner's selection or starting an extra writer |
| Helper killed before/after selection | A subsequent owned helper follows journal/lock recovery and verifies complete baseline |
| Desktop/daemon stop timeout | No selection advance; structured error and retained evidence |
| Final cleanup | No owned children, no held daemon/native lock and no IPC endpoint |

Record exit codes and observed process identities, not only logical callbacks.
Process SIGKILL is not power-loss evidence. Native signatures/resources require
their own checks; unsigned fixtures cannot close the signed delivery gate.

## Canonical-data acceptance prerequisite

The handoff records accepted P0-1 sealed per-device packs, but this branch lacks
the final ADR-0013 integration. Do not recreate that decision or mutate retained
benchmark corpora. Inspect the authoritative accepted layout through an approved
integration/reference step before claiming canonical safety. Later use a fresh
synthetic fixture in that layout; compare full path membership and byte digests
across success, rejection, rollback and interruption. Package helper receives
no canonical-write authority. Existing generic canary files are not this proof.

## Platform execution and Actions policy

Windows implements the first slice and records exact checks/provenance. Mac
receives a committed runnable plan with exact source, commands, expected results
and cleanup contract after that slice exists. Do not ask Mac to run this design
document as a test or reuse the same binary as proof of a version-changing update.

Prefer local format, strict Clippy, locked builds and relevant tests. Use one
targeted workflow only if a required artifact/check cannot be produced on the
physical hosts. No workflow is required to review this documentation. Stop and
report signing/toolchain barriers without bypassing host security policy.
