# ADR-0017 — Structured Event Evolution and Knowledge Materialization

**Status:** ACCEPTED — 2026-10-08
**Milestone:** M4 — Knowledge Engine
**Depends on:** ADR-0013 (ACCEPTED), ADR-0015 (ACCEPTED), ADR-0016 (ACCEPTED), M3 Local Projection + Search (completed)
**Authority:** Accepted M4 decision. This ADR narrows and extends ADR-0016 only as explicitly stated in §4.6; all v1 behavior remains unchanged.

## 1. Purpose

This ADR defines a backward-compatible path from the immutable M2 event
contract to explicit structured knowledge assertions and a deterministic M4
materializer. It preserves all canonical v1 `memory.note` records and their
integrity semantics. It does not authorize production of v2 events. M4 product
implementation may begin only after the coordinated documentation acceptance
change is verified and the owner explicitly authorizes the implementation
phase.

## 2. Repository decision-number reconciliation

The repository's `docs/30_ADR_INDEX.md` and source file
`docs/adr/ADR-0016-immutable-event-store-productization.md` identify ADR-0016 as
the **accepted M2 Immutable Event Store Productization** decision. Its Windows
quarantine addendum is also accepted. The repository contains no ADR-0016
source record titled “Cortex Fresh Re-ingestion, Not Direct Migration.” Cortex
fresh re-ingestion is represented separately by `docs/16_CORTEX_FRESH_REINGESTION.md`
and `docs/milestones/M15_CORTEX_FRESH_REINGESTION.md`.

Accordingly this ADR uses new number **ADR-0017** and does not renumber,
replace, edit, or reinterpret ADR-0016. It is a dependent decision that
refines event evolution and M4 behavior. It refines only those ADR-0016 clauses
explicitly listed in §4.6. All other ADR-0016 choices remain unchanged.

### 2.1 Authority cross-check

| Authority | Existing accepted rule | ADR-0017 treatment |
|---|---|---|
| `docs/00_MASTER_SPEC.md` | Immutable event union; deterministic consolidation after reconnection; no primary device; privacy/classification boundaries | Preserved; capability gating applies to producing new structured event types, not ordinary offline v1 notes |
| ADR-0016 / `docs/04_EVENT_SCHEMA_AND_INVARIANTS.md` | Exact strict v1 schema; JCS + BLAKE3 + Ed25519; unsupported/incompatible data is not silently interpreted; durable quarantine before truncating known-corrupt `.open` suffix | V1 remains exact. ADR-0017 accepts verified-unsupported and unverifiable states as narrow evolution rules; implementation and verification remain required |
| ADR-0013 / `docs/06_STORAGE_ARCHITECTURE.md` | Immutable sealed per-device packs; SQLite and indexes are derived | Preserved; no canonical storage layout or M3 database replacement |
| ADR-0015 | Device identity and durable per-device sequence allocation | Preserved; typed events use the same device principal/sequence chain |
| M3 contract and implementation | `projection.sqlite`, M3 projection schema v1, disposable rebuild; M3 replace-on-known-unusable behavior applies to the derived database | Preserve M3 tables and version. Add M4-owned metadata and tables; test old-reader/additive-table behavior and full rebuild after derived DB replacement |
| `docs/milestones/M4_KNOWLEDGE_ENGINE.md` | Deterministic entity/fact/relation consolidation, correction, provenance, classification/confidence, rebuild | Supplies M4 scope, but does not define exact event fields, compatibility rollout, reducer ordering, or SQLite migration |
| Existing M4 phase-0 notes | Record free-text interpretation and derived-store boundary as open alternatives | Their free-text-to-knowledge alternative is superseded; the records remain as historical audit material with explicit supersession notices |

No production implementation or accepted specification was changed during
this cross-check.

## 3. Accepted foundations this ADR preserves

The following are already accepted and are not reopened here:

1. Canonical durable memory is immutable event data; accepted bytes are never
   edited in place. Corrections add history.
2. `cryptexd` is the only supported canonical writer on a device.
3. ADR-0016 v1 production schema contains `memory.note` with `{text}` and
   strict v1 validation. The same v1 records, JCS bytes, hashes, signatures,
   domain-separation labels, and chain links remain valid byte-for-byte.
4. Integrity uses the accepted JCS + BLAKE3-256 + Ed25519 design and
   per-device `prev_content_hash` chain. V1 verification is not changed.
5. M3 `projection.sqlite` is derived and rebuildable; it is not canonical.
6. M4 knowledge consolidation is deterministic for a fixed canonical event
   set and fixed rules; conflict resolution is not Last-Write-Wins.
7. Free-text and AI output are untrusted. No arbitrary `memory.note` text is
   silently promoted to a confirmed entity, fact, relation, or security rule.

## 4. Accepted schema evolution contract

### 4.1 Version boundaries

Keep three version domains independent:

| Version | Owner | Meaning |
|---|---|---|
| `schema_version` | Canonical event protocol | `1` remains the exact ADR-0016 v1 contract; `2` is the structured event envelope contract defined here |
| `payload_schema_version` | Event type | Exact typed payload schema within a recognized envelope; initial value `1` for each v2 event type |
| `knowledge_meta.schema_version` | M4 derived SQLite schema | M4 table/migration version, independent of both event versions and M3 `projection_meta.schema_version` |
| `cardinality_policy_version` | M4 reducer policy | Version of the exact predicate-cardinality map in §5.2; independent of all schema versions |

No v1 validator is widened to accept v2 fields. The v1 parser and verifier
remain strict and unchanged. A v2 parser is a separately selected, strict
parser. Unknown fields in a known v2 envelope or known payload are rejected;
unknown *event type / payload schema* can be preserved as unsupported only
after the common v2 envelope and cryptographic integrity are verified.

### 4.2 V2 envelope contract

V2 retains the ADR-0016 signable envelope fields and adds a typed payload-schema
discriminator. Exact fields:

```json
{
  "schema_version": 2,
  "event_id": "<UUID>",
  "brain_id": "<UUID>",
  "device_id": "<ADR-0015 DeviceId>",
  "device_seq": "<canonical decimal u64, 1..18446744073709551615>",
  "created_at": "<RFC 3339 display timestamp; not ordering authority>",
  "event_type": "knowledge.assertion",
  "payload_schema_version": 1,
  "classification": "public|internal|private|restricted",
  "signing_key_id": "<ADR-0016 key id>",
  "payload": { "kind": "entity.declare", "...": "typed fields below" },
  "prev_content_hash": "<64 lowercase hex chars>",
  "content_hash": "<64 lowercase hex chars>",
  "signature": "<128 lowercase hex chars>"
}
```

V2 canonicalization remains RFC 8785 JCS. Hash and signature algorithms remain
BLAKE3-256 and Ed25519. V1 preimages and labels are frozen. Exact v2
preimages are:

```text
J2 = RFC8785_JCS(the exact v2 signable fields above, excluding content_hash/signature)
preimage_hash_v2 = UTF8("cryptex-event-content-v2") || 0x00 || J2
content_hash_raw_v2 = BLAKE3-256(preimage_hash_v2)
preimage_sig_v2 = UTF8("cryptex-event-sig-v2") || 0x00 || content_hash_raw_v2
signature_v2 = Ed25519.Sign(device_seed, preimage_sig_v2)
```

A v2 record is never verified using v1 labels. `prev_content_hash` is the
lowercase-hex encoding of the immediately preceding accepted event's raw
content hash, regardless of that predecessor's recognized schema version.
Thus the first v2 event links to the final verified v1 hash; subsequent v2
events link to v2 hashes. The same per-device `device_seq` continues strictly
increasing across the version boundary; there is no reset or parallel chain.
If the predecessor cannot be verified, the chain cannot advance. Signer-key
binding, classification, resource limits, duplicate-key rejection, and
fail-closed validation remain mandatory.

The accepted version-specific domain labels preserve the existing primitives
and v1 verification exactly while avoiding cross-version preimage ambiguity.

### 4.3 Initial typed payload contract

To keep the first contract bounded, v2 uses one event type,
`knowledge.assertion`, with a strict discriminated payload union. Every string
is UTF-8, bounded, and validated; identifiers are UUIDs; unknown fields are
rejected for known payload kinds. The first payload schema version is `1`.
The envelope retains ADR-0016's 1 MiB body, depth, and parser limits. Decimal
values are canonical decimal **strings**, never JSON floating-point numbers:
`-?(0|[1-9][0-9]*)(\.[0-9]*[1-9])?`; exponent, leading zero, trailing
fractional zero, and negative zero are rejected (zero is `"0"`). Text values
are strings, booleans are JSON booleans, dates are valid Gregorian
`YYYY-MM-DD` strings, and entity references are UUID strings.

| `payload.kind` | Exact fields | Meaning |
|---|---|---|
| `entity.declare` | `entity_id`, `entity_kind`, `label`, `publication_authorization` | Explicit declaration. Kinds follow Master Spec vocabulary: `person`, `project`, `course`, `device`, `organization`, `concept`, or `other`. |
| `entity.alias` | `entity_id`, `alias`, `publication_authorization` | Explicit alias claim for an existing entity. |
| `fact.assert` | `subject_entity_id`, `predicate`, `value`, `confidence`, `publication_authorization` | Typed fact. Envelope `event_id` is its assertion identity. `value` is a strict tagged union: `text`, `decimal`, `boolean`, `date`, or `entity_ref`; no untyped object. |
| `relation.assert` | `subject_entity_id`, `predicate`, `object_entity_id`, `confidence`, `publication_authorization` | Typed directed relation; envelope `event_id` is its assertion identity. |
| `assertion.correct` | `target_event_id`, `replacement`, `publication_authorization` | Corrects exactly one fact/relation assertion event with a typed replacement or `null` withdrawal; target remains immutable. |
| `entity.merge` | `entity_ids`, `publication_authorization` | Explicit authorized equivalence of two or more entity IDs; never inferred from names or fuzzy similarity. |
| `entity.merge_revoke` | `target_merge_event_id`, `publication_authorization` | Withdraws one previously accepted merge edge; other merge edges and entity history remain. |
| `assertion.resolve_conflict` | `target_event_id`, `decision_event_ids`, `outcome`, `publication_authorization` | Resolves the complete set of competing correction/resolution events for one assertion to one referenced decision, a typed replacement, or withdrawal. |

### 4.3.1 V2 payload validation constraints

The top-level v2 envelope has exactly the fields shown in §4.2 and rejects
unknown fields. `schema_version` is JSON integer `2`; `event_type` is exactly
`knowledge.assertion`; `payload_schema_version` is an integer in
`[1, 2147483647]`; `event_id` and `brain_id` use the accepted ADR-0015 UUID
validation; `device_id` uses ADR-0015 `DeviceId` validation; `device_seq` is the
canonical decimal string for `1..=u64::MAX`; `created_at` is a valid RFC 3339
string and display metadata only; classification is exactly one of
`public|internal|private|restricted`; `signing_key_id` is exactly 32 lowercase
hex characters; `prev_content_hash` and `content_hash` are exactly 64
lowercase hex characters (the first per-device event uses 64 zeroes for
`prev_content_hash`); signature is exactly 128 lowercase hex characters. The
canonical JCS body is at most 1 MiB and retains ADR-0016 duplicate-key, depth,
element-count, and Unicode validity checks.

Every known payload kind has an exact required-field set and rejects unknown
fields. Strings are valid UTF-8, NFC, non-empty where identifiers/labels are
required, and bounded by the 1 MiB envelope body limit; entity labels and
aliases are each limited to 256 UTF-8 bytes, and structured text fact values to
512 KiB. Labels and aliases must remain non-empty after the §5.2 normalization
step. No schema uses floating-point values. UUID fields use the accepted
ADR-0015 UUID validation. `entity_kind` is exactly one of the seven values in
the table above. `predicate` uses ASCII lowercase
`[a-z][a-z0-9._-]{0,63}`. A `publication_authorization` object has exactly
`basis` and `receipt_id`; basis is `direct_user` or
`user_approved_ai_capture`, and receipt ID is a UUID.

`value` is exactly one of: `{kind:"text", value:<NFC string>}`;
`{kind:"decimal", value:<canonical decimal string>}`;
`{kind:"boolean", value:<JSON boolean>}`;
`{kind:"date", value:<valid YYYY-MM-DD>}`; or
`{kind:"entity_ref", value:<UUID>}`. No extra keys are allowed. Correction
replacement contains only a fact/relation assertion's semantic fields and
confidence; it inherits the enclosing correction's authorization receipt and
canonical provenance. `entity_ids` contains 2–256 sorted, unique UUIDs;
`decision_event_ids` contains 2–4096 sorted, unique UUIDs. Counts are further
bounded by the envelope body limit. References must be same-Brain and
correct-kind. Malformed known payloads are invalid, not unsupported.

Confidence is exactly the Master Spec vocabulary: `confirmed`, `inferred`, or
`uncertain`. It describes epistemic confidence, not publication authorization
or lifecycle. `confirmed` requires a direct user statement/confirmation;
`user_approved_ai_capture` may emit only `inferred` or `uncertain`. Lifecycle
is derived and is not a payload field. The exact JSON Schema and generated
TypeScript must encode these rules; runtime validation remains authoritative
for constraints JSON Schema cannot express.

`publication_authorization` is a separate required object with `basis`
(`direct_user` or `user_approved_ai_capture`) and `receipt_id` (UUID of the
daemon-validated authorized request). In V1, the trusted `cryptexd` writer is
the authorization boundary: it validates the explicit user action before
emitting the signed canonical event. A receiving peer verifies the trusted
device/daemon attestation and preserves the receipt reference, but the event
signature and receipt UUID do **not** independently prove to that peer that a
human performed or approved the UI action, nor that the assertion is true.
This trust boundary records authorized publication under the trusted daemon
model; it is not remote non-repudiation of human intent.

`proposed` assertions remain in local staging and are not canonical events.
This staging is local app-data state, is not synced or searchable as confirmed
knowledge, and can be discarded only by explicit user action without changing
canonical history. Publishing requires an explicit authorized daemon action. A user-authorized
AI capture may publish with confidence `inferred` or `uncertain`; authorization
does not promote it to `confirmed`. AI extraction remains separate and
optional. Knowledge lifecycle (`ACTIVE`, `SUPERSEDED`, `FORGOTTEN`, `PURGED`)
is distinct from both confidence and publication authorization. M4 derives
only lifecycle semantics supported by structured events; Forget/Purge remains
governed by the applicable architecture and M10.

`confirmed` is permitted only when the user directly states or explicitly
confirms the fact; an AI-produced proposal, device trust, signature, or
capture authorization cannot set that confidence by itself. For
`assertion.resolve_conflict`, `outcome` is a strict tagged union:
`{"kind":"select","decision_event_id":"<UUID>"}`,
`{"kind":"replace","value":<typed assertion>}`, or
`{"kind":"withdraw"}`. The `decision_event_id` must be a member of the
complete sorted `decision_event_ids` set; otherwise the resolution is
unresolved and has no effect.

All assertion and correction payloads include provenance through their
canonical `event_id`, device, sequence, classification, signing key ID,
signature, confidence where applicable, authorization basis, and receipt
reference. The full canonical event remains the authoritative provenance
record. Materialized provenance rows copy these fields from the verified event
so they survive a complete canonical-to-SQLite rebuild; no derived row is the
only copy of provenance. A peer's interpretation of authorization remains
bounded by the trusted-daemon limitation above.

For the initial v2 payload schema, the exact event kinds, fields, enums,
primitive value types, limits, and validation rules in §§4.3–4.3.1 are the
accepted contract.
Predicate names use the stated open ASCII namespace; cardinality behavior is
fixed by `cardinality_policy_version=1` in §5.2. Unicode normalization is
versioned as Unicode 16.0.0 by §5.2. Entity/assertion references may point to
events authored on another device; if absent from the current verified event
set, they remain pending as specified in §§5.2 and 6. Production writing
remains disabled by the separate activation gate in §8.2.

### 4.4 Verified unsupported versus unverifiable

Readers classify each complete frame into exactly one of these states:

| State | Required checks | Preserve canonical bytes | Apply to M4 | Stream effect |
|---|---|---:|---:|---|
| `verified_supported` | Recognized envelope + payload schema; canonicalization, hash, signature, device binding, sequence and chain checks pass | Yes | Yes, subject to classification/status/rules | Continue |
| `verified_unsupported` | Recognized envelope version and cryptographic rules; canonical common envelope, hash, signature, device binding, sequence and chain pass; event type or payload schema is unknown | Yes, byte-for-byte | No | Continue integrity verification; update only the per-device integrity watermark |
| `unverifiable_envelope` | Envelope version/algorithm/required integrity semantics are unknown, so verification cannot be completed | Yes, byte-for-byte; no truncate, rewrite, discard, or quarantine-as-corrupt | No | Stop trusted interpretation at this device-chain boundary; preserve later bytes untouched and block appends/seal decisions that would claim a verified continuation |
| `invalid_or_corrupt` | Known-format record fails framing, canonicalization, hash, signature, chain, or strict known-schema validation | Yes until the existing accepted quarantine procedure has durably captured the exact bytes; never treat as unsupported | No | Existing ADR-0016 fail-closed quarantine/recovery rules apply; no silent truncation |

An unsupported but verified event is not corruption. It may advance the
integrity scan watermark only after all integrity checks pass; it must not
advance the **safely materialized knowledge watermark**. These are distinct
progress values.

An unverifiable boundary is not labelled verified, corrupt, or safe. Preserve
the original frame and suffix exactly. A newer reader may re-evaluate from the
last verified offset. Until then, no append, seal, projection, or policy action
may cross that boundary as if the chain were complete.

### 4.5 Reader-first compatibility rollout

Structured-event production is disabled by default. **This is separate from
M4 completion.** M4 may complete its compatible reader, deterministic reducer,
derived schema, rebuild, and tests while the production v2 writer remains
unavailable. The independent production-activation gate remains closed until
all currently trusted participating devices have demonstrated compatible
reader capability and downgrade protection is implemented and tested. Devices
need not be online simultaneously. Evidence may be collected during separate
verified sessions; the trusted-device set and capability records must be
current at activation.

| Reader / writer combination | V1 `memory.note` | V2 supported payload | V2 unknown payload / envelope | Policy |
|---|---|---|---|---|
| Legacy M2/M3 reader + legacy v1 writer | Supported | Not supported | May quarantine/truncate; not safe | May operate only on a store proven to contain no v2/unknown-version data |
| Reader-first M4-capable binary + v1 writer | Supported, byte-identical | Reader supports v2 | Verified unsupported is preserved; unknown envelope is preserved and chain-blocking | Required rollout stage; v2 writer stays off |
| Reader-first M4-capable binary + v2 writer | Supported | Supported and materialized by approved rules | Unsupported is preserved/not materialized; unverifiable boundary blocks | Allowed only after separate production activation gate passes |
| Any old/unknown binary after v2 data exists | V1 reads are not sufficient | Unsafe | Could truncate or misinterpret | Must be prevented from opening/writing the shared store; otherwise v2 production remains disabled |

“Reader-first” is an actual reader release and verification, not a capability
flag that legacy binaries might ignore. It must be deployed before enabling
any v2 producer.

| Device condition | Structured v2 writer | Existing v1 note capture | Required handling |
|---|---:|---:|---|
| Compatible reader demonstrated, active | Eligible only after global gate opens | Available | Reader capability and build identity recorded |
| Compatible reader demonstrated previously, temporarily offline | Eligible only if the current trusted-device roster still proves its capability; otherwise activation remains blocked | Available | No simultaneous connection requirement; roster freshness and changes since last observation must be verifiable |
| Trusted, incompatible or capability unknown | Blocked globally | Available | Upgrade and demonstrate capability, or explicitly retire the device |
| Explicitly retired | Not eligible | Existing local v1 operation policy remains separate | Retirement is an auditable trusted-device operation; do not infer retirement from age/offline status |
| Restored from old backup | Blocked pending revalidation | v1 read/capture only where safe | Quarantine the restored copy from shared canonical writes; upgrade reader and reconcile its chain before rejoining |
| New installation/device | Blocked until reader capability and trust enrollment are verified | v1 note capture may remain local | Do not let an unverified device introduce v2 into shared history |
| Software downgrade | Must be prevented from opening a v2-bearing store | v1-only stores remain usable | Old binaries cannot be made forward-compatible retroactively; enforce a minimum-reader gate before downgrade, or keep v2 production disabled |

**Activation blocker:** current M2 recovery may quarantine/truncate unsupported
schema data. A reader-first release must land and be verified on every trusted
participating device before v2 writer activation. A protocol marker alone is
not sufficient if an old executable ignores it. The repository currently
does not establish an M8-grade trusted-device capability registry or a
verified downgrade-prevention mechanism. Until one is accepted and implemented,
the safe default is to ship M4 readers/materialization with v2 production
disabled and preserve v1 note capture.

Retirement must be explicit, authenticated, auditable, and represented in the
trusted-device authority appropriate to its milestone. This ADR does not
productize pairing/revocation (M8); if no accepted mechanism can prove the
current trusted-device set, activation remains blocked rather than guessing.

Old backup restore must never overwrite newer canonical files. Restore into an
isolated staging copy; inspect version/capability and chain boundary; then
rejoin only through an approved reconciliation flow. Do not run an old binary
against shared canonical files after v2 production begins. A marker an old
binary does not read is not downgrade protection. If install/runtime
enforcement cannot guarantee that an old binary cannot open or write the
v2-bearing store, v2 writing remains disabled. M4 completion does not claim
this mechanism exists.

### 4.6 Relationship to ADR-0016

ADR-0016 remains accepted and unchanged. ADR-0017 accepts the following narrow
refinements:

1. ADR-0016 §3.2's “`memory.note` only in M2” remains true for v1; v2 structured
   types are introduced only by ADR-0017 and a separate strict validator.
2. ADR-0016 §7.2's known-corrupt/truncated-frame quarantine ordering remains
   unchanged. Its “unsupported schema” handling is refined for a complete,
   cryptographically verifiable v2 envelope with an unknown payload: classify
   as `verified_unsupported`, preserve exact bytes, do not truncate/quarantine,
   and do not materialize.
3. A truly unknown envelope version/algorithm is handled as
   `unverifiable_envelope`, preserved exactly and blocking trusted stream
   continuation. It must not be mislabeled as corrupt or verified.
4. All v1 JCS/hash/signature/frame semantics remain frozen; v2 uses explicit
   version-specific verification rules.

ADR-0017 is accepted as the narrowly scoped evolution decision; it does not
rewrite or weaken the established ADR-0016 v1 behavior. The accepted v1
validator, canonical bytes, hashes, signatures, framing, and quarantine rules
remain unchanged. The reader-side refinements above require implementation and
verification before any v2-bearing data is admitted. Production v2 writing
remains disabled until the separate activation gate in §8.2 passes.

## 5. Deterministic knowledge materialization contract

### 5.1 Input and trust

The reducer consumes only `verified_supported` structured events with valid
daemon-recorded publication authorization and permitted classification.
`memory.note` remains durable independently and is never parsed into canonical
knowledge by this reducer. Its availability through any retrieval path is
conditional on a current `RetrievalSafetyState` that allows access; a
Brain-wide hold overrides normal search availability. AI extraction remains
optional and separate; unapproved proposals remain local staging data. A user-authorized AI
capture may become a canonical `inferred` or `uncertain` assertion after the
daemon validates the explicit user action; it does not become `confirmed` by
virtue of the device signature.

### 5.2 Stable dependency and correction order

Replay order is independent of arrival order and wall clock:

1. Verify each device chain in `device_seq` order. The reducer input is a set
   of complete verified device histories, not an arrival stream.
2. Build a global map keyed by `event_id`. Byte-identical duplicates are
   idempotent; an event ID with different verified bytes is a typed integrity
   conflict and blocks affected materialization.
3. Resolve all entity/assertion references after event-set collection. Missing
   references caused by incomplete replication remain pending and are retried
   after input-set change. Cross-Brain, wrong-kind, self, or otherwise invalid
   references are rejected with diagnostics and have no effect.
4. Canonical semantic keys are typed tuples JCS-serialized after these exact
   rules: predicates and relation names must match ASCII lowercase
   `[a-z][a-z0-9._-]{0,63}`; decimal values are base-10 sign/digits without
   exponent or leading zeros and normalize negative zero to zero; dates are
   valid Gregorian `YYYY-MM-DD`; text values are Unicode NFC and otherwise
   byte-exact (no trim, case-fold, or locale behavior); entity labels/aliases
    use NFC, trim leading/trailing Unicode whitespace, then default Unicode
    case-fold using Unicode 16.0.0 data. Windows and macOS must use identical
    versioned test vectors. No fuzzy similarity creates an equivalence. The
    predicate key is an open but syntax-constrained ASCII namespace; new keys
    do not change reducer mechanics.
5. Entity equivalence is created only by an authorized `entity.merge` event
   naming at least two distinct, known IDs. Independently created IDs remain
   separate and ambiguous until that event is accepted. Active merge edges
    form undirected equivalence classes; representative is the lexicographically
    smallest UUID byte sequence in a class, independent of arrival order.
    `entity.merge_revoke` withdraws only its referenced merge edge and
    recomputation splits classes as appropriate. Missing or ambiguous IDs keep
    the merge unresolved.
 6. `assertion.correct` targets exactly one assertion event in the same Brain;
    it cannot target correction/resolution events or itself. Its replacement
    is an inline typed assertion or `null`; replacement derived ID is
    deterministic from the correction event ID and canonical replacement
    bytes. Null withdraws the target from active state while retaining
    target/provenance history. A missing target remains pending until the event
    union changes; wrong-kind references are invalid. These target edges cannot
    form cycles because correction targets are assertion events only.
 7. Two authorized corrections of the same target with incompatible
    replacements/withdrawals form an unresolved conflict. Neither wins by
    wall-clock time, cross-device sequence, arrival order, or event-ID order.
    `assertion.resolve_conflict` must include the complete sorted set of
    competing correction/resolution event IDs and choose one listed decision,
    an inline typed replacement, or withdrawal. Conflicting resolution events
    remain unresolved and require a later resolution over their complete set.
    If replay later discovers a competing decision omitted from an earlier
    resolution's set, that resolution is no longer effective and the conflict
    reopens. Decision-reference cycles or incomplete sets are invalid/unresolved;
    no tie-break chooses a winner.
 8. Resolve references and decision dependencies as a DAG after collecting the
    complete event set. Any strongly connected component caused by malformed
    decision references remains unresolved and none of its decisions applies.
    Among otherwise-ready nodes, use
   `(device_id UTF-8 bytes, device_seq numeric, event_id UUID bytes)` only as a
   stable processing tie-break, never as truth or conflict precedence.
   `created_at` is display metadata only.
9. Consolidate identical assertions to one derived item while preserving the
   complete sorted source-event set and each source's confidence,
   publication-authorization provenance, and classification. Conflicting
   assertions remain separate and reviewable. Stable IDs derive from the
   canonical semantic key or a deterministic source ID; random/replay-order
   IDs are prohibited.

#### Versioned predicate cardinality policy

`cardinality_policy_version=1` is stored in `knowledge_meta` and is part of
the reducer identity. Its exact map is:

| Predicate | Entity-kind constraint | Cardinality |
|---|---|---|
| `person.birth_date` | subject must be `person` | Single-valued |
| Every other predicate, including unknown future predicates | No extra kind constraint unless a later policy version adds one | Multi-valued |

For a single-valued key, byte/semantic-identical values consolidate; distinct
active normalized values form an unresolved conflict, with no chosen winner.
Resolution requires explicit authorized correction/withdrawal events that
address the competing assertion events; no assertion is selected merely by
being newer, arriving later, or having a different device sequence.
For multi-valued keys, distinct values coexist and exact semantic duplicates
consolidate. The single-valued fact key is `(brain_id, canonical subject
entity_id, predicate)`; `value` is deliberately excluded from that key so
different active birth dates are reviewable conflicts. For a multi-valued fact,
the semantic key additionally includes its normalized typed value. Relations
are multi-valued by default: distinct object IDs coexist, and only the identical
`(brain_id, subject, predicate, object)` tuple consolidates. Different values
or objects under a multi-valued predicate are not conflicts merely because they
differ. Changing this map requires a new explicit policy version and a full
deterministic rebuild; old events are never rewritten. No fuzzy, ontology, or
inverse-relation inference changes cardinality.

Identical semantic keys consolidate while preserving each source confidence;
the reducer does not invent one aggregate confidence value. Alias lookup uses
the exact normalized alias key: one target resolves, zero is missing, and
multiple targets are ambiguous and remain unresolved without merging entities.

### 5.3 Forget, purge, classification, and security

Unknown or unsupported events are never interpreted as permission to expose,
retain, forget, or purge content. They cannot override policy. If an unknown
event may affect forget/purge or classification and affected records cannot
be identified precisely, withhold the entire Brain from content retrieval.
This gate applies uniformly to M3 FTS hits and snippets, Desktop results,
daemon-controlled Obsidian projection generation/export, and future AI/MCP
context through a daemon-owned safety state; there is no supported direct-client
bypass. Integrity uncertainty blocks use of the
affected chain and triggers the broader Brain hold whenever policy impact is
not bounded. M4 must not claim purge completion; purge implementation remains
governed by M10 and its accepted policy.

Restricted/private classification propagates conservatively: a consolidated
item cannot become less restricted than any contributing source. If sources
have incompatible visibility, preserve separate variants or use the most
restrictive classification; do not widen exposure through deduplication.

### 5.4 Retrieval safety across surfaces

The daemon owns one `RetrievalSafetyState` derived from verified event
boundaries and applicable privacy/Forget/Purge policy. Every content-returning
path consults it before returning or exporting content: M3 FTS results and
snippets, Desktop retrieval, Obsidian materialization/export, and future AI/MCP
adapters. Clients do not read canonical files or SQLite directly to bypass
this gate.

Every statement in this ADR that M3 search remains available means available
only when the current `RetrievalSafetyState` is valid and allows the requested
content. A Brain-wide hold always overrides ordinary M3 search availability,
including while M4 is blocked, migrating, or rebuilding.

If unsupported semantics may affect privacy, classification, Forget, or Purge
and the affected records cannot be precisely identified, the fail-closed scope
is the entire Brain: withhold all content from normal search, Desktop,
Obsidian, and AI/MCP context until a capable reader resolves the boundary.
Diagnostics may expose non-sensitive IDs, offsets, versions, and reason codes.
If the safety state is missing, corrupt, stale for the canonical tip, or cannot
be read, retrieval also defaults to the Brain-wide hold until verified state is
rebuilt. A failed guard must never be interpreted as “no restrictions.”
V1 note capture remains available while v2 production is disabled; appends to a
device chain blocked by an unverifiable boundary are refused. A capture on an
unaffected stream may continue, but cannot reopen Brain-wide retrieval.

M4 implementation must wire this guard into the daemon-owned M3 search path
and test hits/snippets. Desktop inherits the daemon result. The contract for
any daemon-controlled Obsidian generation/refresh/export is to consult the same
guard and create or refresh no managed plaintext while a Brain-wide hold is
active; defining this contract does not implement M6/M7 Obsidian behavior. This
guard cannot revoke or erase plaintext files, exports, backups, or copies
already written to locations accessible to the operating system or user; the
daemon cannot technically guarantee those are inaccessible. AI/MCP adapters
must use the same policy gate when implemented; they must not add weaker
independent rules. M4 completion tests current M3/Desktop retrieval and the
shared daemon safety decision that future Obsidian/AI adapters must consume;
later surface implementation must include the same guard before exposure.

## 6. M3 SQLite contract

Use the existing `<app-data>/derived/projection/projection.sqlite`. Do not
create a second canonical or projection database. Preserve all M3 tables,
columns, indexes, FTS behavior, and `projection_meta.schema_version=1`.

Add M4-owned tables transactionally:

| Table | Purpose / key columns |
|---|---|
| `knowledge_meta` | Singleton `id=1`; `schema_version`; `reducer_version`; `cardinality_policy_version`; `normalization_version`; `build_id`; rebuild status |
| `knowledge_entities` | Stable `entity_id`; `entity_kind`; display label; lifecycle/materialization status |
| `knowledge_aliases` | `entity_id`, normalized alias key, display alias; unique key scoped by Brain and normalization version |
| `knowledge_assertions` | Stable assertion key, kind, subject/predicate/value/object, lifecycle state, classification |
| `knowledge_provenance` | Materialized row key, source `event_id`, device/seq, signing key ID, per-source confidence, authorization basis/receipt reference, classification, contribution role |
| `knowledge_conflicts` | Stable conflict key, competing assertion IDs, unresolved reason code; no winner field based on timestamp |
| `knowledge_corrections` | Correction event ID, target event ID, replacement ID/null, resolution status |
| `knowledge_materialization_state` | Sole authoritative per-device integrity watermark, contiguous safe materialization position, unresolved boundary/offset, and blocking reason |
| `knowledge_pending_dependencies` | One row per unresolved source event/reference: source device/pack/frame offset/event ID, dependency event ID if known, typed reason code; rebuilt from canonical input |

The listed table boundaries and key columns are the accepted M4 storage
contract. Implementation may select concrete SQL constraints and indexes only
where they preserve these semantics and M3's schema. All rows are derived,
disposable, and rebuildable. M3 event search tables remain independent
while M4 is blocked or rebuilding; M3 search is available only when the current
`RetrievalSafetyState` allows it, and a Brain-wide hold takes precedence.

### 6.1 Migration and down-version behavior

1. Acquire the existing canonical device lock(s) in stable device-ID order,
   then SQLite transaction ownership; preserve M3's device→SQLite lock order.
2. Use `BEGIN IMMEDIATE`; inspect M3 schema without changing it; create/validate
   all M4 tables and `knowledge_meta` in the same transaction.
3. Commit only after all constraints and version checks pass. Any failure rolls
   back the entire M4 migration and leaves M3 tables/data unchanged.
4. Repeating migration is idempotent. Unknown newer M4 `knowledge_meta` version
   fails closed for M4 use; never auto-drop M4 tables to make an older version
   open. M3 event search remains available only if its schema is valid and the
   current `RetrievalSafetyState` allows access; a Brain-wide hold overrides it.
5. A downgrade may ignore additive M4 tables only if its M3 validator is proven
   to tolerate them. It must not rewrite `projection_meta` or delete M4 tables.
   Current M3 `open_replacing_unusable` can replace the complete derived DB on
   corruption/unsupported M3 schema, so that path can erase M4 tables too; this
   is recoverable only by a full canonical rebuild. Test and document this
   explicitly; do not treat old M3 binaries as safe downgrade readers without
   evidence.
6. Full rebuild uses one SQLite transaction over M4 tables only: clear/rebuild
   M4 rows and state, then commit only after supported events, conflicts,
   provenance, and both watermarks are consistent. On failure, rollback leaves
   the last committed M4 generation and all M3 rows unchanged. A rebuild is
   idempotent for the same canonical event set and reducer version. Canonical
   event files are never touched by SQLite migration/rebuild.

`knowledge_materialization_state` is the only authoritative owner of both
per-device progress values. `knowledge_meta` contains no duplicate watermarks.
Integrity progress records the verified canonical frame-chain position.
Materialization progress is the greatest **contiguous event position** in that
device chain for which every preceding event is verified, supported or
semantically resolved, policy-eligible, and successfully applied. Position is
tracked by pack/frame offset plus content hash (not by assuming numeric
`device_seq` has no gaps). A verified-unsupported event can advance integrity
progress but blocks contiguous materialization progress at that frame. An
unverifiable boundary advances neither past the last verified position.

Every supported event held for a missing cross-device reference has a durable
derived row in `knowledge_pending_dependencies`; it is not counted as applied
and cannot advance the contiguous materialization position past that event.
An unresolved reference is never reported as successfully materialized or
permanently skipped. When the referenced event arrives, replay re-evaluates
the pending event and its dependent suffix, applies it only after validation,
removes the pending row, and advances progress. If complete history proves a
reference invalid rather than merely absent, retain a typed invalid/conflict
diagnostic and keep safe progress blocked at that event until an explicit
resolution or corrected canonical event set makes it applicable. Full rebuild
reconstructs both pending rows and watermarks from canonical packs.

Required late-arrival case: device A has a verified assertion referring to an
entity declared by device B, but B's event has not arrived. A's integrity
watermark may advance through A's verified frame; A's safe materialization
watermark stops immediately before that event, and the missing reference is
recorded as pending. Once B's declaration arrives, replay processes the
dependency closure, clears the pending row only after applying A's assertion,
advances A's contiguous safe position, and yields the same semantic state as a
full rebuild over the combined event set. There is no permanent skip based on
arrival order, and legal `device_seq` gaps do not themselves imply missing
events.

## 7. Failure modes and required behavior

| Failure | Required behavior |
|---|---|
| Invalid v1 event | Existing strict v1 validation and ADR-0016 failure/quarantine behavior; never reinterpret as v2 |
| Known v2 envelope, malformed known payload | Typed invalid-schema failure; preserve exact bytes through durable quarantine; no M4 apply |
| Known v2 envelope, unknown event type/payload version, valid signature/chain | `verified_unsupported`; preserve bytes, continue integrity scan, do not materialize |
| Unknown envelope version or crypto semantics | `unverifiable_envelope`; preserve bytes and remaining suffix exactly; block chain continuation and appends until a capable reader resolves it |
| Bad signature/hash/chain | Integrity failure; existing durable quarantine rules; no watermark past failure |
| Duplicate event ID, identical bytes | Idempotent replay, retain one logical assertion and all relevant provenance references |
| Duplicate event ID, differing content | Typed conflict/integrity failure; stop affected materialization; no winner |
| Missing/cyclic correction dependency | Keep unresolved diagnostic; do not apply correction; do not order by time |
| SQLite migration/rebuild failure | Roll back M4 transaction; preserve M3 and canonical packs; report typed failure |
| Unknown event may change privacy/forget/purge policy and affected records cannot be identified | Brain-wide retrieval hold across M3 FTS, Desktop, daemon-controlled Obsidian generation/export, and AI/MCP; preserve bytes and require capable reader. Previously generated plaintext remains outside daemon revocation control. |

Diagnostics expose event IDs, device IDs, offsets, version numbers, and reason
codes only; never payload text, secrets, or key material.

## 8. Separate acceptance gates

### 8.1 M4 implementation completion — independent of v2 activation

M4 can complete its compatible reader, deterministic reducer, additive derived
schema, retrieval guard, and verification without an enabled v2 writer, M8
device-management implementation, or M14 work. Required tests:

- Golden byte/hash/signature fixtures prove v1 `memory.note` is unchanged.
- Reader classifies supported v1/v2, verified-unsupported, unverifiable, and
  invalid records distinctly; v1 remains strict.
- Verified-unsupported bytes remain exact, verified, non-quarantined, and
  non-materialized; only integrity progress advances.
- Unknown envelope bytes/suffix remain exact across scan/restart; no truncation,
  rewrite, append, seal, or false verified label occurs. A capable reader can
  resume at the preserved boundary; invalid crypto remains invalid.
- Replay permutations across devices and arrival orders yield identical
  semantic state and digest on Windows/macOS.
- Cardinality policy v1: identical single-valued assertions consolidate;
  distinct `person.birth_date` values remain a conflict without a winner;
  unknown predicates and distinct relations remain multi-valued.
- Duplicate assertions preserve complete provenance; conflicts remain
  explicit with no LWW; separate cross-device entity IDs remain separate until
  an explicit authorized `entity.merge`.
- Corrections cover replacement, withdrawal, competing corrections, explicit
  resolution, missing/wrong-kind/self references, and malformed cycles.
  Valid correction cycles are structurally impossible under §5.2.
- Full SQLite rebuild equals incremental materialization and is idempotent;
  canonical packs remain byte-for-byte unchanged.
- M3→M4 transactional migration, interrupted rollback, refusal of newer
  `knowledge_meta`, M3 data preservation, and rebuild after disposable DB
  replacement are tested.
- Unknown policy-affecting semantics suppress M3 FTS hits/snippets and current
  Desktop retrieval Brain-wide when scope cannot be identified precisely;
  missing/corrupt/stale safety state and Brain-wide hold override normal M3
  search availability.
- A verified event plus receipt reference preserves authorization provenance
  through full rebuild; tests do not claim that a remote peer can independently
  prove a human performed or approved the action.
- A late cross-device dependency remains pending and blocks contiguous safe
  materialization until its referenced event arrives; incremental resolution
  equals full rebuild and does not permanently skip the event.
- The shared daemon safety decision reports Brain-wide hold for the future
  Obsidian adapter contract; prior plaintext files/copies are documented as
  outside daemon revocation control. M6/M7 product behavior is not implemented.
- Offline/no-LLM behavior, explicit publication authorization, classification
  propagation, and restrictions on notes/AI proposals are tested.

Required cross-platform, security, CI, owner review, and Definition of Done
gates follow `docs/21_DEFINITION_OF_DONE.md` and
`docs/milestones/M4_KNOWLEDGE_ENGINE.md`.

### 8.2 Production v2 activation — separate gate, not M4 completion

Production structured writing remains disabled until separately implemented
and verified:

- A trusted-device capability authority has a complete/current roster, and
  every currently trusted participating device has demonstrated compatible
  reader behavior. Devices need not be simultaneously online.
- Unknown/incompatible devices block activation until upgraded or explicitly
  retired through an authenticated, auditable mechanism. Offline status alone
  never retires a device.
- Old-backup restoration stages separately, cannot overwrite newer canonical
  bytes, and requires capability/reader/chain revalidation before joining.
- Actual downgrade protection prevents an old binary opening or writing any
  v2-bearing store. A marker ignored by the old binary is insufficient. If
  enforcement cannot be proven, the writer remains disabled.
- V1 note capture remains usable while v2 production is blocked. A blocked
  device chain refuses append without losing the user's captured input; local
  staging may retain it until safe publication.
- Rollback/activation procedures prove that an incompatible reader cannot be
  silently reintroduced after activation.

This activation gate may depend on later device-management/distribution work,
but none of it is required to complete M4's reader/reducer implementation.
Do not implement M8/M14 opportunistically to satisfy M4. Until every activation
condition passes, no production v2 event is emitted.

## 9. Explicitly out of scope

- Changing, rewriting, or weakening canonical v1 event records/validator.
- Enabling v2 structured event production before compatibility activation gate.
- Parsing `memory.note` into confirmed facts/entities/relations.
- Autonomous AI/LLM extraction or promotion.
- M5 capture UX, M6/M7 Obsidian behavior, M8 pairing/revocation productization,
  M10 Forget/Purge product workflows, or M11 recovery container.
- A second canonical database, cloud dependency, or custom cryptography.

## 10. Accepted decisions and remaining implementation gates

The owner has approved this design, including the v2 domain-separated
preimages in §4.2 and the narrow ADR-0016 relationship in §4.6. The accepted
contract fixes payload fields and validation limits (§§4.3–4.3.1), confidence vs
authorization vs lifecycle, trusted-daemon publication boundary and its
remote-proof limitation (§4.3.1), versioned cardinality (§5.2), pending
dependency/watermark behavior (§6), RetrievalSafetyState precedence and
Obsidian plaintext limits (§§5.3–5.4), and separate M4/v2 activation gates
(§8). This ADR is the scoped dependent decision for those refinements; the
accepted ADR-0016 v1 record is left intact and is not rewritten.

Production v2 activation remains a separate, later gate: its trusted-device
capability authority, roster freshness, retirement, old-backup rejoin, and
enforceable downgrade mechanism must be implemented and tested before any v2
writer is enabled. Those mechanisms are not claimed to work and are not
prerequisites to M4 implementation completion. M4 implementation itself must
wait until this coordinated documentation acceptance change is verified and
the owner explicitly authorizes the implementation phase.

The following are fixed proposals, not unresolved alternatives: Master Spec
confidence labels (`confirmed`/`inferred`/`uncertain`); separate authorization
and lifecycle dimensions; local-only staging for `proposed`; explicit
authorized entity merge with no fuzzy merge; Brain-wide retrieval hold when
policy impact cannot be bounded; single-valued `person.birth_date` and
multi-valued unknown predicates; separate M4 completion and v2 activation
gates; and one authoritative per-device watermark table.

## 11. Acceptance gate

This record is **ACCEPTED** on 2026-10-08. Acceptance authorizes M4 reader,
reducer, derived-schema, retrieval-safety, and verification implementation only
after the coordinated documentation acceptance change is verified and the
owner explicitly authorizes the implementation phase. It does not authorize a
production v2 writer: §8.2 remains a separate, closed gate. No M8/M10/M14 or
other out-of-scope implementation is authorized here.
