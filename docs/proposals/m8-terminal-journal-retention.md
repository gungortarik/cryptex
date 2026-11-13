# M8 terminal journal retention decision

Status: ACCEPTED FOR IMPLEMENTATION on 2026-11-13; migration not yet implemented.
Source reviewed: `06b0c29b4333942a10f54db216fc5843795eaaa9`.

The owner delegated the choice ("hangisi iyiyse sen onu sec") after receiving
the concrete proposal below. The selected option is compact terminal receipts
in one atomically replaced journal, preserving cancelled-ID refusal and uncertain
evidence under the existing one-MiB bound. This authorizes that implementation
and migration, not the complete ADR-0019 addendum or ordinary product activation.
The owner separately selected macOS 26+ as sufficient for the application.

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
  return a distinct typed terminal-history-capacity error before any new signed
  message/capability is released or canonical event published. Do not erase old
  cancellation/replay protection or call the operation
  successful. This is finite storage, not an infinite-retention guarantee.

Migration must validate the entire old journal first, preserve all uncertain
evidence, and atomically install the new representation. Unsupported/corrupt
versions are preserved and refused. Every lookup and mutation route must consult
both active entries and terminal receipts; no route may silently treat a compact
terminal ID as new. A failed write/replace preserves the previous representation.

## Acceptance before implementation is complete

### Selected implementation details

Journal wire schema3 separates active transactions/exchanges from terminal
receipts. Decoding moves them into one bounded in-memory transaction view, so
existing cancellation, collision, startup and exact-retry routes continue to
observe every terminal ID. Encoding borrows data instead of duplicating strings.
Cancelled receipts retain the generated-but-unreleased flag but no reusable
prefix or completeness hash. Anchored admission receipts retain four exact public
messages for existing resume checks; only the matched attempt and explicit
canonical-position/operation-digest witness remain. The witness is data, not
authority: startup/retry rechecks the unique current canonical record under the
existing lock order. Legacy anchors remain full until that fresh proof exists.

Legacy1/2 files are first validated with their original sixteen-total cap. Load
does not rewrite them or claim durable migration: the next successful atomic
update writes schema3. Unsupported/corrupt input and failed migration are held
with evidence preserved. No canonical locks are acquired by loading/encoding.

The raw read is bounded to one MiB plus one overflow-detection byte. Persisted
payload remains one MiB; the expanded unified serialized view is bounded to two
MiB before signature verification. A4096-record pre-hydration resource bound is
conservative because a valid terminal receipt's mandatory IDs/field names alone
exceed256bytes; it cannot reduce valid one-MiB capacity. There is no compression
or unbounded expansion. JSON parser/tree overhead is additional, bounded by raw
input and closed typed collection limits; these are serialized-resource bounds,
not a platform-exact heap-allocation measurement.

The earlier pre-signing wording is refined explicitly under the owner's
delegation: ephemeral in-memory signature computation may precede persistence,
as in existing R3. Capacity or persistence failure releases no signature or
capability, restores no consumed confirmation token and publishes no canonical
event. A new reservation protocol is not introduced merely to avoid computation.
Canonical publication still requires its durable AppendUnknown evidence first.

Injected pre-replacement write/flush/replace failures must preserve old bytes;
post-replacement reopen/sync/directory-sync failures report typed unconfirmed
durability and preserve the visible candidate rather than falsely claim rollback.
Windows replacement/reopen semantics do not establish Unix parent-directory sync.

Native verification/source review of the implementation is still pending.

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

## Decision authority

Repository [AGENTS.md](../../AGENTS.md) says: “If a requested change in tool
roles, workflow, architecture, branch strategy, milestone scope, or integration
strategy is ambiguous, ask the user before changing direction.” This proposal
introduces a persisted terminal-receipt format and retention/capacity behavior;
the existing implementation basis did not select those semantics. Independent
review recommended this option; subsequent owner delegation authorized the choice
above. Review alone did not authorize an amendment or product activation. No
further permission question is needed for the selected implementation.
