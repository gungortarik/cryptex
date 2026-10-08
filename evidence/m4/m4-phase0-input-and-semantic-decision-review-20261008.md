# M4 Phase 0 — Input audit and semantic decision review

**Status:** Discovery complete; owner decision review required before dependent product code  
**Branch:** `m4/knowledge-engine`  
**Base:** M3 merge `35538c1`  
**Scope:** Read-only source audit and proposed decision framing; no product code, schema/pin change, runtime data, or Actions.

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
correction assertions. Arbitrary natural-language `text` does not by itself
specify how those assertions are extracted. AI output is untrusted under the
accepted model and cannot silently become accepted knowledge or protected
policy.

**Blocking design question:** define what accepted source representation M4
consolidates before implementing an extractor/materializer. Adding canonical
event kinds or fields would change the accepted M2 event/schema boundary and
must go through the ADR/specification process. Silently treating heuristic or
AI extraction as authoritative would violate determinism/trust requirements.

## 2. Options requiring owner selection

| Decision | Option A | Option B | Option C / consequence |
|---|---|---|---|
| Knowledge assertions as input | Extend canonical events with explicit typed assertion/correction events (requires proposed ADR/schema compatibility decision) | Keep `memory.note` unchanged and define a strictly deterministic, conservative parser over its text as M4 evidence | Defer semantic assertions until a later accepted input contract; M4 scope may need revision because no meaningful general extraction is available |
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
Owner review must resolve the input contract and store boundary, or authorize a
scoped ADR proposal, before Slice 1. No dependencies are pinned and no Actions
were run for this discovery pass.
