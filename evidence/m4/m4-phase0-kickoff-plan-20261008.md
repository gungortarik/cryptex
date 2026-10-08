# M4 Phase 0 — Knowledge Engine preparation

**Status:** HISTORICAL — Phase 0 discovery superseded by accepted ADR-0017 (2026-10-08)
**Branch:** `m4/knowledge-engine`  
**Base:** `m0/foundation` at M3 merge commit `35538c144434e1dbfb97b47f89d2536b7646046a` (PR #9)  
**Scope of this change:** documentation and planning only; no product code, pins, runtime data, or Actions.

> **Superseded planning record.** Its open choices about free-text
> interpretation, event schema, reducer rules, storage boundary, and owner
> decisions are resolved by accepted ADR-0017. Do not use its former
> recommendation that M4 could interpret `memory.note` text as current
> authority. ADR-0017 governs. Product implementation remains blocked until
> the documentation acceptance change is verified and the owner explicitly
> authorizes the M4 implementation phase; production v2 writing remains behind
> ADR-0017 §8.2.

## 1. Authority reviewed

- `AGENTS.md`
- `docs/00_MASTER_SPEC.md`
- `docs/03_MEMORY_MODEL.md`
- `docs/05_ENTITY_FACT_RELATION_MODEL.md`
- `docs/17_TEST_AND_FAILURE_MATRIX.md`
- `docs/21_DEFINITION_OF_DONE.md`
- `docs/milestones/M4_KNOWLEDGE_ENGINE.md`
- M3 projection/search implementation and closeout evidence

The accepted M4 contract permits deterministic derived consolidation, but the
entity/fact/relation document intentionally leaves concrete schemas, matching,
confidence, and tie-breaking unresolved. Planning may map those gaps; product
code must not silently choose them.

## 2. M3 closeout / M4 starting point

M3 is complete within scope and integrated through normal merge PR #9 at
`35538c1`. Its SQLite/FTS state remains a disposable search projection. M4
knowledge materialization must remain a distinct, rebuildable derived result
over accepted canonical event history; M3 search tables must not become
canonical knowledge or be extended with unreviewed M4 semantics.

Canonical events remain immutable and authoritative. `cryptexd` remains the
only supported canonical writer. Untrusted event text or AI output cannot
silently establish protected/security policy. Forget/Purge behavior remains
owned by its later milestone and must not be invented here.

## 3. M4 scope mapped to work areas

1. **Input and deterministic replay:** consume only accepted, integrity-verified
   canonical history; define stable ordering and deterministic rebuild behavior.
2. **Entity and alias model:** identify entities, preserve alias provenance, and
   define normalization, collision, ambiguity, and merge behavior.
3. **Facts and relations:** define typed fact keys/values, relation taxonomy,
   direction/cardinality, and how new assertions supersede older materialized
   state without rewriting events.
4. **Confidence and lifecycle:** preserve `confirmed` / `inferred` / `uncertain`
   and `ACTIVE` / `SUPERSEDED` semantics; avoid authority derived from wall
   clock or untrusted content.
5. **Correction and provenance:** reconstruct correction/supersede state from
   additive canonical history and retain links to the source events.
6. **Derived storage and recovery:** select an isolated rebuildable layout,
   transactional update strategy, schema/version behavior, and failure handling.
7. **Validation:** deterministic replay, aliases and ambiguity, fact/relation
   reconstruction, correction/supersede, confidence/classification, corrupt or
   missing derived state, and cross-platform parity.

These are planning headings, not decisions about exact product schemas or
algorithms.

## 4. Decisions that must be resolved before dependent product code

The following are intentionally open in accepted specifications:

- stable entity ID generation and collision behavior;
- alias normalization/matching and ambiguous-resolution policy;
- fact key/value taxonomy and equality semantics;
- relation type taxonomy, direction, and cardinality;
- confidence assignment/update rules and deterministic tie-breaks;
- correction/supersede precedence when events conflict or are concurrent;
- exact provenance fields and representation;
- materialized schema, location, and migration/rebuild policy;
- trusted versus untrusted authority tiers for any protected namespaces.

For each decision, prepare a concise option/tradeoff record and identify
whether an accepted ADR or a reviewed implementation-evidence decision is
required. Do not pin libraries or add dependencies until the storage/API plan
is reviewed.

## 5. Proposed preparation sequence

1. Audit M2 event shapes and M3 verified-reader interfaces against the M4
   contract; list exactly what M4 can consume without changing canonical schema.
2. Produce decision tables for the open semantics above, including deterministic
   replay examples and conflict cases.
3. Propose a narrow daemon-owned read/materialization API and a separate derived
   store boundary; identify how M3 search can later consume M4 state without
   making it canonical.
4. Draft acceptance matrix and failure/rebuild tests from the M4 contract and
   `docs/17_TEST_AND_FAILURE_MATRIX.md`.
5. Review the Phase 0 packet and resolve any ADR gates before Slice 1.

Discovery update: source-envelope and verified-reader audit plus the initial
owner decision table are recorded in
`evidence/m4/m4-phase0-input-and-semantic-decision-review-20261008.md`.
The accepted source is canonical event history; current schema v1 contains
free-form `memory.note` text, not typed knowledge assertions. The open decision
is its deterministic interpretation policy. New event types are not required
unless a reviewed design establishes that need and the ADR/schema process
accepts it.

Do not start product slices, change accepted ADRs/specs, or run Actions as part
of Phase 0. A final workflow should be reserved for a reviewed implementation
candidate when required by the gate.

## 6. M4 completion gate reminder

M4 is not complete until deterministic rebuild, entity/alias consolidation,
fact/relation state, correction/supersede reconstruction, confidence/state,
failure cases, relevant tests, required CI, DoD, and human review all pass.
Only then may M5 begin.

## 7. Supersession record — 2026-10-08

The owner accepted ADR-0017 after this Phase 0 record was prepared. Its
proposed semantics, payload, M3 database boundary, compatibility behavior and
acceptance sequence are historical alternatives, not current authority.
Current scope and implementation sequencing are in
`docs/milestones/M4_KNOWLEDGE_ENGINE.md`. Implementation still waits for
verification of the coordinated documentation acceptance change and explicit
owner authorization; production v2 writing remains behind ADR-0017 §8.2.
