# M8 terminal journal retention decision

Status: PROPOSED, not an accepted protocol amendment or implemented migration.
Source reviewed: `06b0c29b4333942a10f54db216fc5843795eaaa9`.

## Problem

`PendingBootstrapJournal` currently counts completed/cancelled transactions
against the sixteen-pending-operation bound. Sixteen terminal entries therefore
prevent another pairing/restriction/retirement even with no active operation.
R3 in the ADR-0019 addendum bounds pending operations and the whole journal to
one MiB. Raising that limit or deleting cancellation history is not a safe fix.

## Recommended decision

Keep one daemon-owned, atomically replaced, closed-schema journal. Introduce a
versioned compact terminal-receipt collection, distinct from the at-most-sixteen
active exchanges/transactions. Both collections share the existing one-MiB
serialized payload limit. There is no age-based eviction and no new canonical
cancellation event. Existing canonical event bytes remain immutable.

* A cancelled-before-release operation with no append evidence may become a
  compact cancellation receipt. Its transaction ID remains permanently reserved;
  admission/offer/resume paths must reject re-creation of that ID. Preserve its
  reserved stable event ID, operation and approved-snapshot digest binding.
* An anchored operation may compact only after its unique canonical record is
  independently verified under the existing writer/history guards. Retain its
  transaction ID, operation/type, operation digest, stable event ID, sequence,
  content hash and local signing identity. Exact retries resolve against that
  verified canonical record; changed operations or ambiguous records refuse.
* AppendUnknown, released-but-unanchored, CancelledAfterRelease and ambiguous
  evidence retain their full original journal representation. They are never
  discarded to make room. No raw full entry disappears before its replacement
  receipt is durably installed under the same lock/replace transaction.
* The overall byte bound still applies. If compact receipts eventually fill it,
  return a distinct typed terminal-history-capacity error before new signing or
  writes. Do not erase old cancellation/replay protection or call the operation
  successful. This is finite storage, not an infinite-retention guarantee.

Migration must validate the entire old journal first, preserve all uncertain
evidence, and atomically install the new representation. Unsupported/corrupt
versions are preserved and refused. Every lookup and mutation route must consult
both active entries and terminal receipts; no route may silently treat a compact
terminal ID as new. A failed write/replace preserves the previous representation.

## Acceptance before implementation is complete

Exercise sixteen cancelled and sixteen anchored operations followed by a new
operation, migration/reopen, exact retry and changed-request rejection, cancelled
offer re-creation refusal, unknown/released evidence retention, stable event-ID
collisions, canonical absence/ambiguity/corruption, byte-bound refusal, failed
write/replace recovery and cross-process serialization on both native platforms.
Use synthetic data and existing daemon-only writer paths. An independent source
review and exact-source Mac/Windows CI must pass before acceptance.

## Alternative

A separately segmented terminal archive needs additional locks, atomicity and
rebuild rules. It is a larger architecture change and is not selected here.
Keeping the present sixteen-total lifetime cap preserves current behavior but
leaves the availability defect unresolved and M8 open.

## Why an owner decision is pending

Repository [AGENTS.md](../../AGENTS.md) says: “If a requested change in tool
roles, workflow, architecture, branch strategy, milestone scope, or integration
strategy is ambiguous, ask the user before changing direction.” This proposal
introduces a persisted terminal-receipt format and retention/capacity behavior;
the existing implementation basis does not select those semantics. Read-only
independent review recommends this bounded option, but that review does not
authorize an amendment or product activation.
