# M8 independent security review follow-up (2026-10-25)

**Source supplied by owner:** Cursor Security Review `b6f68bf7-93d5-4674-9286-e2221cdf5e52`.
**Reviewed source pin:** `2fe8639` on `codex/m8-device-trust`.
**Reported workspace tip:** `2683917` (subsequent docs/test-only commits at time of report).
**Recommendation:** **NOT READY**.

The review states that its critical findings are latent until canonical
publication or trust activation. It confirmed the implementation is still
pre-publication: the `publish_staged_no_replace` helper only places files in
staging, no production caller wires `ImportStagingSession`, and prepared
recovery aborts only while all canonical destinations are absent. A visible
destination remains a typed conflict; no committed marker or partial replay
exists.

## Findings

| Severity | ID | Finding |
|---|---|---|
| Critical | C1 | No canonical import publication API under inventory and sorted device locks. |
| Critical | C2 | No committed marker or recovery/replay for a partially published batch. |
| Critical | C3 | Owner-approval IPC and bounded bootstrap parser are not implemented; `peer_review` remains in-memory. |
| Critical | C4 | Import validation does not apply trust roster, cutoff policy, or `RetrievalSafetyState`. |
| High | H1 | Windows parent-directory durability has no verified implementation. |
| High | H2 | Non-Windows `inspect_directory_chain` is a path-based stub; macOS handle-containment parity is absent. |
| High | H3 | Alias scanning is admission/recovery-time only and is not a publication-time check under canonical locks. |
| High | H4 | Managed transport is not wired to the staging pipeline. |
| High | H5 | Staging has no publication integration with live canonical appenders. |
| Medium | M1 | A crash leaving the prepared-manifest temporary file causes recovery to fail closed. |
| Medium | M2 | `verify_batch*` is test-only and not part of a production admission path. |
| Medium | M3 | There is no new-peer stream admission path. |

The review also confirmed: startup recovery runs before `hold`/`login-hold`,
identity enrollment, and `pilot-fixture` writer paths; Windows handle-walk and
junction tests exist; managed XML does not enable active folders or peers;
R1–R5 remain proposed; and the reported 13/13 staging plus 411/411 library
results do not establish full M8 completion. Native fault-injection, broader
owned-process/Tauri review, and future Syncthing folder-boundary runtime proof
remain missing.

## Follow-up implemented after the review

The prepared-manifest temp-file crash window was addressed on the current
branch: recovery now recognizes only the exact `.<canonical-uuid>.prepared.tmp`
name, requires its staging transaction to match, revalidates it as a bounded
single-link regular file, removes that incomplete unprepared transaction, and
leaves the transport source available for retry. Unknown temp names and a
coexisting temp/final manifest fail closed. Focused Windows tests cover cleanup
and preservation of unknown journal state. This does not implement publication
or change the review recommendation for C1–C4/H1–H5.

The independent review is pinned to `2fe8639`; it does not cover this later
temp-manifest recovery change. Before any trust activation, refresh the review
against a commit containing the change and resolve the remaining critical and
high findings. No trust/transport activation, Actions, or M8-complete claim.
