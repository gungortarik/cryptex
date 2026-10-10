# M4 Phase 0 — Input audit and semantic decision review

**Status:** HISTORICAL — discovery and decision table superseded by accepted ADR-0017 (2026-10-08)
**Branch:** `m4/knowledge-engine`
**Base:** M3 merge `35538c1`
**Scope:** Read-only source audit and proposed decision framing; no product code, schema/pin change, runtime data, or Actions.

> **Superseded decision framing.** The alternatives below are retained as an
> audit trail only. In particular, deterministic interpretation of free-form
> `memory.note` text and the former claim that new event kinds were not
> required are no longer current decisions. Accepted ADR-0017 preserves v1
> `memory.note` unchanged and defines structured v2 assertions, deterministic
> materialization, the existing M3 database boundary, and the separate
> production-v2 activation gate. Product implementation remains unauthorized
> until the documentation acceptance change is verified and the owner
> explicitly authorizes the implementation phase.

## 1. What the current canonical input can support

The current production M2 envelope (`cryptex-events::AcceptedEvent`) is schema
version 1 and currently has only one event type: `memory.note`. Its payload is
only `text`; the envelope also supplies event/brain/device IDs, device sequence,
creation timestamp, classification, signing-key ID, previous/content hashes,
and signature. M3's daemon-owned verified reader yields accepted events after
the existing integrity checks and holds the per-device canonical lock through
the callback path.

This gives M4 trustworthy provenance and a deterministic byte-level source,
but does **not** provide typed entity, alias, fact, relation, confidence, or
correction assertions. The accepted M4 contract already identifies the input
source as accepted canonical event history; the unresolved design question is
how the current free-form `memory.note` text is interpreted deterministically.
AI output is untrusted under the accepted model and cannot silently become
accepted knowledge or protected policy.

The specs do **not** require adding new canonical event kinds before M4 can
start. If a proposed design expands the accepted M2 event/schema boundary, that
expansion must go through the ADR/specification process. Otherwise, document
and review the extraction semantics as M4 implementation evidence before
coding them.

## 2. Options requiring owner selection

| Decision | Option A | Option B | Option C / consequence |
|---|---|---|---|
| Interpretation of canonical input | Keep M2 `memory.note` unchanged and define a deterministic, conservative interpretation policy over its text as M4 evidence | Restrict extraction to explicitly structured declarations in note text | Propose typed canonical assertion/correction events only if the M4 design demonstrates that current events cannot meet scope; this would require ADR/schema review |
| AI-assisted extraction | AI may only create untrusted proposals, with a separate explicit acceptance path | No AI extraction in M4 | Making AI output directly authoritative is disallowed |
| Derived store boundary | New isolated rebuildable M4 store | Separate M4 tables/schema in the existing M3 projection DB, with strict ownership/versioning | M3 search state must never become canonical; any shared DB choice needs explicit failure/rebuild and locking analysis |
| Entity identity | Deterministic ID derived from stable accepted provenance | Deterministic IDs assigned by replay order | Either must define collisions and cross-device union behavior; random IDs are not replay-deterministic |
| Alias resolution | Exact normalized alias match only; ambiguous matches remain unresolved | Reviewed/scored matching with explicit ambiguity thresholds | No fuzzy auto-merge without a deterministic, auditable policy |
| Fact/relation conflicts | Partial-order/concurrent assertions remain multi-valued until resolved | Explicit deterministic precedence/tie-break policy | Wall-clock timestamps alone cannot be security authority |
| Confidence | Preserve only confidence explicitly present in accepted input | Deterministic evidence-based derivation, with complete rules in M4 evidence | AI-inferred confidence needs explicit proposal/acceptance semantics |

These are alternatives for review, **not accepted decisions**. The table does
not select entity IDs, normalization, fact/relation taxonomies, thresholds,
confidence scoring, precedence, or schema.

## 3. Proposed first implementation slice after decisions

Once the owner resolves the input contract and storage boundary, Slice 1 should
be limited to the selected domain types and validators plus schema/evidence
tests. It should not yet perform text extraction or canonical writes. Later
slices can then implement deterministic replay, entity/alias resolution,
fact/relation materialization, lifecycle/correction reconstruction, and
disposable-store rebuild with the corresponding failure tests.

Every output row must retain source-event provenance and classification.
Materialized state must be reproducible from the same accepted event history
and accepted rules; canonical events remain immutable; all writes remain
daemon-owned; loss of M4 derived state must be recoverable from canonical data.

## 4. Acceptance tests to require in the reviewed plan

- Same accepted history + same rules yields byte/logically identical derived state.
- Reordering event arrival across devices does not create nondeterministic output.
- Alias collisions and ambiguous references fail closed or remain explicitly
  unresolved; no silent entity merge.
- Corrections/supersession add derived state without editing canonical history.
- Confidence and classification survive rebuild and do not gain authority from
  untrusted text or wall-clock ordering.
- Missing/corrupt derived state rebuilds; canonical event packs remain byte-
  unchanged.
- Windows/macOS behavior agrees for schema, locking, filesystem, and recovery.

## 5. Gate

M4 preparation has begun, but product implementation remains **NOT STARTED**.
Owner review must resolve the note interpretation policy and derived-store
boundary before dependent product code; an ADR is needed only if the reviewed
design changes an accepted architecture/schema decision. No dependencies are
pinned and no Actions were run for this discovery pass.

## 6. Supersession record — 2026-10-08

The owner accepted ADR-0017 after this read-only discovery review. The options
and gate text above are retained only as historical audit context. They are
superseded by ADR-0017 and the updated
`docs/milestones/M4_KNOWLEDGE_ENGINE.md`: preserve v1 `memory.note` unchanged,
use explicitly structured v2 assertions, and materialize additively in M3's
existing `projection.sqlite`. The documentation acceptance change must be
verified before the owner separately authorizes implementation. Production v2
writing remains disabled until ADR-0017 §8.2 passes.
