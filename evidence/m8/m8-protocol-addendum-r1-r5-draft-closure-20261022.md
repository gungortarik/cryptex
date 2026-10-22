# M8 protocol addendum — scoped findings draft closure

**Date:** 2026-10-22\
**Candidate:** `docs/adr/ADR-0019-m8-protocol-addendum-proposal.md`\
**Disposition:** M8-R1 through M8-R5 now have concrete draft resolutions in the
proposal. The protocol remains **PROPOSED / NOT ACCEPTED**. This is not an
independent security review or implementation authorization.

## Draft resolutions

| Finding | Candidate resolution |
|---|---|
| M8-R1 immutable owner approval | BLAKE3/JCS digest binds the frozen operation, Brain/transaction, exact local and peer identities, transport binding, agreement hash and exact prior-message hash; a single-use daemon token is bound to the authenticated Desktop session. Five-minute monotonic session expiry; restart requires re-display and fresh confirmation. |
| M8-R2 bounded parsing | 64-KiB raw UTF-8 limit before parse; streaming bounds of depth 4, 24 object members, 16 array items, and 16-KiB decoded strings; duplicate decoded keys and non-ASCII/unknown keys rejected; only exact numeric token `1` is allowed for protocol version; bounds precede JCS. Bootstrap-only limits do not change v1/v2 readers. |
| M8-R3 pending/idempotency | Public-only daemon journal, 16 transaction / 1-MiB cap, role-specific allowed state transitions, terminal owner cancel/expiry, canonical-stream reconciliation and exact transcript/key comparison before retry. Uncertain append is reread under canonical locks; no blind second append. The proposal states that cancellation after releasing a signed approval cannot revoke that signature remotely. |
| M8-R4 v3 chain/reader | v3 shares the existing device sequence allocator and cross-version `prev_content_hash`; each link uses the verified prior record's content hash and version-specific verification. Local key comes from ADR-0018; remote key from this installation's direct local approval anchor. Older readers hold guarded Brain content on v3. Vectors include v1→v2→v3→v1. |
| M8-R5 identity equality | New protocol boundary uses lowercase hyphenated UUIDs; DeviceId stays exact, case-sensitive ASCII bytes; key IDs, fingerprints, public keys and transport IDs compare as specified bytes/strings, with no silent normalization. Existing v1/v2 acceptance is unchanged. |

## Idempotency schema correction

A follow-up consistency pass found that the generic retry rule searched by
`transaction_id`, but the restriction and self-retirement payloads did not
carry that field. The candidate event schema now includes it in every
`security.*` payload and requires uniqueness across those event types in the
local author's stream. The proposal also defines a domain-separated BLAKE3
operation digest over the stable event identity and payload fields while
excluding sequence, predecessor, timestamp, event ID, and signature fields
that can differ or be derived on retry. This permits an uncertain restriction
or retirement append to be recognized without treating a different operation
as a duplicate. Idempotent success requires exactly one matching record;
duplicate canonical records for one transaction ID cause a Brain hold even if
the digests match. This is a proposal-level schema change only; it has not been
independently reviewed or implemented.

The pending-journal record now names the Brain/operation, frozen owner-visible
snapshot and digest, plus whether a local approval signature was generated and
released. This supports re-presenting the same candidate after restart and
distinguishing an unreleased signature from one that may already exist on the
peer. The proposal now specifies candidate platform-specific atomic journal replacement
and requires write/flush/replace/readback fault injection plus torn/corrupt
journal recovery on both platforms. Journal rollback protection and whether
these durability steps meet the product's guarantees remain review gates.

## Additional sequence-gap consistency correction

The repository's sequence allocator may leave gaps after a reservation whose
event append did not complete. The first candidate wording required a
self-retirement at sequence `n` to set its previous boundary to `n - 1`, which
would point to a nonexistent record in that valid case. The proposal now binds
retirement to the immediately preceding verified chain record's actual
sequence and content hash (or zero/zero-hash when the stream is empty). A
restriction cutoff must likewise name an existing event sequence/hash in the
verified prefix, not an unused reserved number. This is a proposal correction;
no implementation or test was run.

The reducer draft also now includes a valid self-retirement boundary in that
device's effective authority cutoff. The cap is applied before evaluating its
outgoing restriction events in the issuer-to-target DAG, so post-retirement
actions cannot issue restrictions. This relationship still requires
independent review for cycles, cutoff ordering, and convergence.

The proposal now detects cycles in the set of structurally/cryptographically
valid restrictions from locally directly approved issuers before pruning
actions outside each issuer's effective prefix. This chooses a conservative
Brain hold for any such cycle, even where an edge could later be ineligible;
the availability/denial-of-service cost is explicitly flagged for independent
review rather than described as settled product behavior. The reducer wording
now makes the later hold rule refer to the same pre-filter cycle check, avoiding
an apparent second/post-filter cycle pass.

## Source cross-check follow-up

Static inspection of `canonical_writer.rs` and `sequence_store.rs` confirmed
that the per-device canonical lock spans reconcile, sequence allocation,
append, and tip update, while the sequence-store process/counter locks are
released immediately after the durable reservation and before event append.
The candidate now records those distinct lock lifetimes. It also says a valid
strictly-increasing event sequence may skip values while the hash link must
remain valid; the previous phrase “chain gaps” was narrowed to missing or broken
predecessor links. These are text corrections against existing implementation
facts, not proof that a future importer or the proposed journal is safe.

## Local reducer sanity pass (not independent)

The candidate graph is internally ordered as follows: an edge `issuer ->
target` means restrictions incoming to an issuer are resolved before that
issuer's outgoing actions. In an acyclic graph, a source-to-target topological
pass can compute each issuer's effective cap before deciding whether its
restriction event is eligible. Multiple eligible incoming restrictions reduce
to the minimum cutoff; a valid self-retirement adds a terminal cap; an issuer
action above either cap cannot tighten another device. All accepted inputs are
verified first and the final result is recomputed from the complete validated
union, so file arrival order and timestamps do not select a winner. The chosen
pre-filter cycle hold is consistent with fail-closed intent but can allow an
otherwise ineligible signed edge to cause Brain-wide unavailability.

The independent review must still challenge at least these boundary scenarios:
competing restrictions arriving in opposite orders; a restriction issuer that
is itself restricted before/after its action; self-retirement after another
issuer's cutoff; a self-edge and multi-device cycle; a valid skipped sequence
before the existing event named by a restriction cutoff; missing or conflicting
target-key history; and a directly approved issuer whose key is compromised.
This pass found no additional
internal ordering contradiction, but it is not an independent liveness,
cryptographic, or cross-platform security review.

## Additional local clarity pass (not independent review)

The R1 token rule now states that validation and consumption are atomic within
the authenticated session: concurrent confirm/cancel/replay attempts serialize,
cancellation invalidates a token if it wins, and only one request can consume
it. Consumption precedes signing or append; any subsequent error requires a
fresh displayed confirmation and token. R2 now defines depth as container
levels including the root object and defines the 16-KiB string bound as decoded
UTF-8 bytes for keys and values, enforced during escape decoding; invalid
surrogate sequences are rejected. These are proposal clarifications only and
have not received independent security review or protocol acceptance.

## Remaining gates

- Independent security review must challenge the five resolutions, especially
  owner-session binding, durable journal semantics, replay/cancel behavior, and
  v3 key/chain integration. The earlier scoped review does not satisfy this.
- Complete the separate issuer/reducer review for mutual restrictions, cycles,
  forks, cutoff boundary hashes, compromised issuers, and convergence.
- Static lock-order reconciliation inspected `canonical_writer.rs`,
  `sequence_store.rs`, and `knowledge_materialization.rs`. A correction now
  makes the lifetime explicit: inventory/device locks stay held while a local
  append obtains and durably reserves a sequence under the sequence-store
  process/counter locks; those sequence locks are released before event append
  and sync, while canonical locks remain held. The code warns that
  out-of-protocol event-file mutation is not race-free; Syncthing or any
  importer must join the writer lock protocol before trust activation.
- Still define and verify platform durability, journal persistence, importer
  lock integration, restore/downgrade and crash behavior against the actual
  event store before any writer exists.
- Generate exact schemas/bindings and byte-level vectors, then run the required
  Windows batch and consolidated macOS stage only after protocol acceptance.
- The generated/vector gate must include the security-operation digest cases:
  same operation with new sequence/predecessor, changed payload, transaction-ID
  reuse across event types, and multiple canonical matches.
- M7 remains deferred; managed-note I/O stays disabled pending its containment
  prerequisite.

No product code, generated schema, tests, trust state, Actions, commit, push or
merge changed in this documentation-only draft step.
