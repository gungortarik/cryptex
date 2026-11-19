# Delivering a device's final signed retirement

The owner approved one portable handoff followed by automatic forwarding.
A exports its original signed retirement in a `.cxret` file. In Device Trust,
use **Save retirement notice** on A and **Open retirement notice** on B.
Once B applies the notice, C can obtain the original sealed segment from B
through normal sync. A can remain offline throughout forwarding.

## Behavior before this change

1. The owner confirms retirement on A. The daemon records the signed event
   once in A's canonical history. Its boundary identifies the preceding
   verified record by sequence and hash. Sequence allocation can have gaps.
2. A's reduced trust state immediately excludes it from normal exchange.
   Retrying an old exchange plan also requires fresh authority. Queuing the
   event for the next ordinary sync therefore cannot guarantee first delivery.
3. On any exporter, an ordinary physical pack is eligible only if every
   record is within its device's effective cutoff. The retirement event's own
   sequence exceeds its declared content boundary. Even a peer that obtained
   the event cannot rely on ordinary export to repopulate its sealed segment.
4. When an eligible receiver already has the signed bytes and sufficient
   verified history, its daemon can apply the retirement boundary, including
   during recovery of a prepared import. That proves application at that
   receiver. It says nothing about how the bytes first reached it.

Implementation references: [owner append](../../services/cryptexd/src/knowledge_materialization.rs),
[exchange admission](../../services/cryptexd/src/managed_profile/exchange_policy.rs),
[whole-pack eligibility](../../services/cryptexd/src/knowledge_materialization/export_plan.rs),
[sealed checkpoint](../milestones/M8_SEALED_EXPORT_CHECKPOINT.md) and
[prepared import recovery](../../services/cryptexd/src/import_staging.rs).
The [physical cutoff receipts](../../evidence/m8/m8-physical-lan-cutoff-9a6dc973.json)
exercise a restriction imposed by another still-authorized device.

## Approved first delivery and forwarding

| Stage | Action and observable result |
| --- | --- |
| A retires, including while offline | On confirmed retirement, under the writer locks, finish the prior active segment if needed, then append and seal the single terminal event in its own segment. Show local retirement as soon as the canonical append is durable. Persist enough state to finish sealing or recreate the handoff from the same canonical bytes after a crash. Report handoff availability separately. |
| Owner delivers once to B | A narrow native export/import action carries the original terminal-only sealed segment to one remaining trusted device, including by removable media. The daemon chooses the source and validates its original device and index. The package carries no notes, signing keys or credentials. This works after ordinary sync closes and does not need a live B at confirmation time. |
| B verifies and stores | B uses its own verified Brain history, device/key binding, original signature, preceding sequence/hash and canonical import rules. It publishes through the daemon's existing no-replace path. A matching replay has no additional canonical effect. B can report local application after durable publication and policy reduction. |
| Other peers reconnect | B, or another eligible holder, automatically offers the exact verified terminal-only segment over its own eligible sessions. This requires a dedicated forwarding rule. Each receiving daemon verifies A's event independently and applies it when its verified history supports it. An offline peer can catch up while a holder remains available. No peer signs on A's behalf. |

The forwarding rule admits only a byte-identical sealed segment containing
one validated terminal event, from a locked verified snapshot. The ordinary
cutoff continues to apply to every content record and every other policy event.
A previously retired identity with its terminal event in a mixed segment needs
separate recovery review. Rewriting its canonical segment or bundling its notes
would invalidate the minimal handoff design.

If B lacks the preceding history, keep the portable file for a retry and
obtain the required history from an eligible replica. Apply the existing
guarded-content hold wherever ADR-0019 requires it. A package manifest cannot
establish authority. If the only prior copy is on A, an explicit recovery of
that history may be necessary. A verified fork or authenticated later event
follows the existing hold rules. Unauthenticated input cannot establish
retirement.

If A and the sole copy of its notice are lost before handoff, delivery of A's
signature is impossible. A remaining owner can use the existing peer restriction
flow as a separate decision. The UI should distinguish local retirement,
handoff availability and local application on each receiver. No stage here
produces an acknowledgement from all devices.

## Verification and acceptance

At source `2f5bc2b`, the following Windows automated checks passed using test
fixtures. They exercise today's event, authority and import behavior.

| Cargo test selection (all with `--locked`) | Passed | Coverage |
| --- | ---: | --- |
| `-p cryptex-events self_retirement` | 3 | Event validation, signing and boundary ordering. |
| `-p cryptexd --lib retirement` | 7 | Confirm-once, durable append recovery, stale plan refusal, local exchange restrictions and terminal trust rules. |
| `-p cryptexd --lib prepared_recovery_batch_verifies_existing_destination_against_preimport_policy` | 1 | Existing destination verified under pre-import policy and imported retirement cutoff reapplied on recovery. |

The three-profile interrupted-publication regression interrupts A after the
durable append and B after staging its import. It verifies a reproducible
package, idempotent recovery, refusal of ordinary exchange on A, and exact
terminal bytes and cutoff on B and C after offline reopen. The complementary
native worker test uses three disposable Windows owners. Only B opens the
file. B is shut down before C comes online. The test requires B and C to
converge through the product's network workers with A offline.

The owner file contains the original signed UTF-8 event and its original
segment index. The daemon reconstructs the canonical frame, checks its
signature and preceding history on import, and publishes using its normal
no-replace transaction. A mixed historical segment is ineligible for this
handoff. A peer that has never directly approved the source cannot grant it
authority by importing a file.

Trust rules: [ADR-0019 protocol addendum](../adr/ADR-0019-m8-protocol-addendum-proposal.md)
and [restriction authority addendum](../adr/ADR-0019-m8-restriction-authority-addendum.md).
