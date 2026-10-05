# Proposed M0 / production acceptance boundary decision

**Status: OWNER APPROVED FOR PREPARING ADR REVISIONS — 2026-10-05.**
The owner answered "kurallar dahilinde onayliyorum" to the concrete scope question
after Mac parity was recorded. This authorizes preparing the proposed revisions;
it does not accept ADRs, complete M0, authorize M1 or approve distribution.
M0 remains IN PROGRESS. The recommendation below is retained as the reviewed
proposal; revised ADR-0012/0004 remain PROPOSED pending review and required CI.
Read alongside `m0-security-notice-acceptance-review-20261005.md`.

## Why a decision is needed

Accepted M0 contract scopes executable foundation/P0 spikes and excludes production
identity/event semantics. Roadmap M1 includes OS secure-store integration; M14
includes packaging/updater coordination; M17 includes signed/notarized release
and final third-party notices. Proposed ADR-0012
currently additionally makes signed package/production canonical acceptance
prerequisites to its own acceptance. Handoff requires honoring those prerequisites.
We cannot silently accept the proposal, remove a prerequisite or build later
product milestones to get a green M0 checkbox.

## Recommended decision for review

Allow a reviewed ADR revision separating **M0 implementation-direction acceptance**
from **production distribution acceptance**, with the following exact boundaries:

| Area | M0 evidence and proposed acceptance | Production requirement retained |
| --- | --- | --- |
| Mode B supervision | Both-OS launch/close/login/crash/instance/IPC/tray/clean stop; backend-only sidecar selection after new Mac regression | Final installed/signed lifecycle, including console-free login on final package |
| Desktop/daemon update | Complete changed native package pair selection/protocol refusal/rollback; one actual updater kill before successor launch; immutable accepted-codec synthetic packs unchanged | Installed updater, real production event schema safety, signing/publisher validation, required crash/power-loss boundaries |
| Managed Syncthing | Both-OS owned isolated native 2.1.5 lifecycle/auth/pairing/history/conflict/staged notices; actual 2.1.2 -> 2.1.5 fixture transition and controlled cold restore | Shipped backend supervisor, production identity/secret permissions, installed distribution, full dependency notices and source-access delivery |
| Secrets | No production secrets in M0; generated native fixture files remain private test state outside Git and are never AI/client credentials | M1 secure-store integration and subsequent replication-specific secret boundary |
| Availability | Existing read-only IPC role/ACL/frame/correlation evidence; explicitly record missing whole-read deadline | Stalled-peer resilience before exposing a hardened production API |
| Legal/provenance | Exact upstream notice text/digests and links, recorded fixture archive correspondence and scoped staged inclusion | Complete dependency/source-access/distribution review, actual installer notice delivery and publisher authentication |

Under this proposal no production requirement is labeled PASS or deleted. Owner
approval would authorize preparing revised ADR-0012/0004 **proposals** with those
separate gates. It would not itself accept the ADRs, green the checklist, authorize
M1, authorize distribution or certify safety. New Mac check, remaining implementer
review, final required CI and human diff/ADR approval still precede M0 completion.

## Alternative

Keep the proposed production prerequisites as current M0 blockers. M0 then stays
open until corresponding signing/installed-update/production-schema evidence is
available. Scope for work belonging to later milestones must be explicitly
authorized before implementation; no automatic milestone expansion is inferred.

## Recorded owner answer and continuation

The owner approved preparing the scoped revisions. The Mac boundary regression
is now PASS at `388e46a7e5f750d88e7fc786e0c108dcea5329e4`; no rerun is required.
Review revised ADR-0012/0004, complete remaining scoped review, then execute
required final CI once on the stable candidate. Human diff/ADR acceptance and
the full evidence-backed M0 checklist remain separate required steps.
