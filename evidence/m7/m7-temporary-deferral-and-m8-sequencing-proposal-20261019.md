# Accepted temporary M7 deferral and M8 sequencing exception

**Date:** 2026-10-19  
**Status:** ACCEPTED by the owner on 2026-10-19; implementation-order exception only.  
**Product baseline:** `74bc3b14127962579ae03838bafea9336612a033`

## Problem and proposed outcome

M7's managed-note write-back prerequisite remains unresolved: no supported
Vault API design has established physical containment on both target OSes.
The owner selected the existing M6 generated-Markdown view while awaiting the
Obsidian API/forum response. M6 is complete within its accepted scope.

The current M7 contract ends with "Only then may M8 begin." The M8 contract's
explicit technical prerequisites are M0 managed-Syncthing proof, the accepted
ADR-0004 implementation addendum, M1 identity/sequence foundations, and M2
event validation/integrity foundations. It does not list the Obsidian plugin
as a technical prerequisite. That does not itself override the sequencing rule.

The owner accepted one explicit scheduling exception: leave M7 OPEN / DEFERRED with
managed-note write-back disabled, and allow M8 preparation and implementation
under its existing contract. Do not
record M7 as complete or permanently remove controlled editing from V1.

## Preserved requirements

- The Master Spec's canonical/derived boundary and eventual controlled
  editing requirement remain in force. Markdown remains a rebuildable view.
- Only `cryptexd` writes canonical data on a device.
- M7's Vault API, physical-containment, authorization, loop-suppression,
  cross-platform, verification, and completion requirements remain in force.
- No Obsidian proposal capability is enabled by this scheduling exception.
- M8 must satisfy its own trust, validation, offline-convergence, revocation,
  platform, CI, and Definition of Done gates; nothing is waived.
- The no-Actions instruction remains active. Any required CI gate that cannot
  be met under that instruction must be reported, not silently waived.
- M7's unresolved status remains visible through later reviews. This exception
  does not authorize a V1/beta completion claim while required M7 work is open.

## Approved documentation changes

1. In `docs/milestones/M7_OBSIDIAN_CONTROLLED_WRITEBACK.md`, preserve its
   completion criteria and replace only its final ordering sentence with:

   > Owner-approved scheduling exception (2026-10-19): M8 may begin while M7
   > remains OPEN / DEFERRED. Managed-note write-back stays disabled until
   > M7's prerequisite spike is resolved. This exception changes implementation
   > order only; M7's feature and completion requirements remain in force.

2. Add an interim scheduling note to `docs/20_MILESTONE_ROADMAP.md` linking
   this decision and recording M7 as deferred, M6 visualization available,
   and M8 as the next active implementation milestone. Keep milestone scopes.
3. Add a prerequisite note in the M8 contract referencing the accepted
   scheduling exception; retain all existing M8 prerequisites and gates.
4. Update `docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md` with the owner's actual
   acceptance date, active milestone, deferred M7 gates, and next M8 slice.

The scheduling exception is reflected in the milestone contracts, roadmap,
and operational handoff. Existing architectural and completion requirements
remain in force.

## Next work

Begin with a bounded M8 readiness and implementation map, using the accepted
sync/trust contracts and existing code:

| Foundation | Existing source or evidence to inspect |
|---|---|
| Managed transport | Accepted ADR-0004 addendum and M0 P0-4 Windows/Mac evidence; the measured controller is a test tool, not production |
| Pairing data and identity | `crates/core/src/pairing.rs`, `services/cryptexd/src/pairing_store.rs`, `identity_runtime.rs`, `local_device_identity.rs` |
| Sequence foundation | `crates/core/src/sequence.rs`, daemon `sequence_store.rs` and `sequence_runtime.rs` |
| Incoming event validation | Existing M2 validation/integrity/quarantine code; determine the guarded import entrypoint from source |
| Accepted trust semantics | `docs/07_SYNC_AND_CONFLICT_PROTOCOL.md`, `docs/08_SECURITY_DEVICE_TRUST_AND_RECOVERY_ROOT.md`, accepted ADRs |

Map existing behavior versus M8 requirements before choosing the first product
change. Treat transport pairing and Cryptex application trust as separate
requirements. Do not infer trust from file visibility or common device ownership.
Do not select new trust-event formats or cryptographic mechanisms in this record.

Keep Windows and Mac dirty checkouts preserved. Use an isolated clean worktree
for implementation; the designated Mac verification checkout remains read-only
unless its role is explicitly changed. Runtime data stays outside Git.

## Return to M7

An API/forum response is evidence to evaluate, not automatic permission to
enable write-back. Resume managed-note operations only after a supported design
meets the accepted cross-platform containment gate, or a separately accepted
specification decision changes that requirement. Record the remaining proposal
authorization and real-daemon in-flight cleanup gaps as open in either case.

## Acceptance

**Owner decision:** accepted on 2026-10-19 in chat ("tamam devam edebilirsin").
This explicit acceptance follows review of the proposed M7 deferral and M8
sequencing exception. It authorizes M8 preparation and implementation within
the existing M8 scope while M7 remains open with write-back disabled.
