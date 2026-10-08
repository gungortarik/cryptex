# M4 — Knowledge Engine

**Status:** ACCEPTED — architecture by ADR-0017; implementation authorized and in progress
**Baseline:** 2026-09-28; scope updated 2026-10-08

## Purpose

M4 builds deterministic, rebuildable knowledge materialization from explicitly
structured canonical events. The governing architecture is
`docs/adr/ADR-0017-structured-event-evolution-and-knowledge-materialization.md`.

## Authority

Follow `AGENTS.md`, `docs/00_MASTER_SPEC.md`, `docs/03_MEMORY_MODEL.md`,
`docs/04_EVENT_SCHEMA_AND_INVARIANTS.md`,
`docs/05_ENTITY_FACT_RELATION_MODEL.md`, `docs/06_STORAGE_ARCHITECTURE.md`,
`docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/21_DEFINITION_OF_DONE.md`, and
accepted ADR-0013, ADR-0015, ADR-0016, and ADR-0017. If they conflict, stop and
report the conflict before implementation.

## Scope

- Strict v1/v2 reader classification, preserving v1 `memory.note` bytes and
  validation exactly as established by ADR-0016.
- Explicit structured assertion payloads from ADR-0017 only; arbitrary
  `memory.note` text and AI output are never silently promoted to canonical
  entities, facts, relations, or policy.
- Deterministic entity/alias/fact/relation consolidation, versioned
  cardinality, explicit corrections/withdrawals/merge, conflicts and
  provenance.
- Additive M4 tables in the existing M3 `projection.sqlite`; independent M4
  schema/reducer metadata, transactional migration and complete rebuild from
  canonical events.
- Per-device integrity and contiguous safe-materialization watermarks,
  unresolved dependency tracking, and deterministic late-dependency replay.
- RetrievalSafetyState enforcement for current M3 search and Desktop
  retrieval. A Brain-wide hold takes precedence over normal search. The shared
  daemon safety decision is the required future-adapter contract for
  Obsidian/AI; M6/M7/M14 product behavior remains out of scope. Previously
  generated plaintext accessible to the OS/user cannot be revoked by daemon
  policy.

## Required invariants

- Canonical history remains immutable, authoritative, and written only by
  `cryptexd`; M3 and M4 SQLite state remains derived and rebuildable.
- Existing v1 events, bytes, validation, cryptographic preimages, signatures,
  framing, and quarantine behavior do not change.
- Verified-unsupported records are preserved exactly and not materialized;
  unverifiable envelopes and suffixes remain untouched and block trusted
  interpretation/continuation. Neither is silently truncated or discarded.
- Same complete canonical event set and policy versions produce equivalent
  semantic state across event arrival order, Windows, and macOS.
- Provenance and classification survive deduplication and full rebuild.
- No timestamp/LWW winner; ambiguity, unresolved dependencies, and conflicts
  remain explicit. Unknown predicates are multi-valued under cardinality
  policy v1.
- Missing/corrupt/stale RetrievalSafetyState and Brain-wide holds fail closed
  across supported content-returning paths.
- Production v2 writing remains disabled until ADR-0017 §8.2's separate
  activation gate is implemented and verified.

## Implementation sequence (after explicit owner authorization)

1. **Reader boundary:** add the separately selected strict v2 envelope/payload
   validator and reader classifications while retaining the strict v1 path;
   prove byte/hash/signature compatibility and unsupported/unverifiable
   preservation before any writer work.
2. **Payload/schema:** implement only ADR-0017's initial
   `knowledge.assertion` payload kinds, exact limits, generated schema/bindings,
   and validation vectors. Keep `proposed` assertions local and production v2
   writing disabled.
3. **Reducer:** implement deterministic normalization, entity/alias handling,
   cardinality policy v1, duplicate provenance, conflicts, corrections,
   withdrawals, explicit entity merges, and dependency resolution.
4. **Derived SQLite:** add M4 tables to M3 `projection.sqlite`; implement
   transactional additive migration, one authoritative materialization-state
   table, pending dependencies, and full idempotent M4-only rebuild without
   modifying M3 schema v1 or canonical packs.
5. **Daemon integration and safety:** connect verified canonical input to the
   M4 reducer and enforce RetrievalSafetyState on M3 search/snippets and Desktop
   results. Expose the shared hold decision for future adapters; do not
   implement later milestone UI or product behavior.
6. **Verification/review:** complete the tests below, Windows/macOS parity,
   security review, required CI once on the reviewed candidate, Definition of
   Done, and human review. Record results before any merge/completion claim.

## Required verification

- Golden v1 `memory.note` bytes/hash/signature vectors and strict rejection of
  v2 fields by the v1 reader.
- V2 known payload valid/invalid cases and separate
  `verified_unsupported`, `unverifiable_envelope`, and corrupt classifications;
  exact byte/suffix preservation across restart with no unsafe append/seal.
- Deterministic replay under event-arrival permutations and Windows/macOS
  semantic equivalence.
- Duplicate fact consolidation with complete provenance; cardinality v1
  single-valued `person.birth_date` conflict/no winner; multi-valued unknown
  predicates and relations.
- Conflicting corrections/resolutions, withdrawal, entity merge/revoke,
  missing/wrong-kind references, cycles, and late cross-device dependency
  arrival. Pending events must not be reported applied or permanently skipped;
  resolved incremental output must equal full rebuild.
- Full M4 SQLite rebuild equals incremental materialization and is idempotent;
  M3 schema/data remain intact; migration failure rolls back; canonical packs
  remain byte-identical.
- Offline operation without an LLM; explicit publication authorization;
  confidence/authorization/lifecycle separation; signed receipt provenance
  survives rebuild without claiming remote proof of human action.
- RetrievalSafetyState allow/hold/missing/corrupt/stale cases for M3 FTS hits
  and snippets plus Desktop retrieval; Brain-wide hold overrides normal
  search. Verify the shared hold decision for future adapters and document the
  OS-accessible prior-plaintext limitation.
- Cross-platform format, tests, strict lint, security/failure/recovery checks,
  required final CI, and DoD evidence.

## Separate production-v2 activation gate

M4 implementation completion does not enable v2 writing. Production v2
activation remains closed until every condition in ADR-0017 §8.2 is actually
implemented and verified, including compatible reader capability for all
trusted participating devices (not necessarily online simultaneously),
explicit retirement, safe old-backup rejoin, and enforceable downgrade
protection. No marker-only claim is sufficient. V1 note capture remains usable
while v2 writing is blocked. M8/M10/M14 work is not authorized opportunistically
to satisfy M4.

## M4 completion gate

M4 is complete only after all M4 scope, required tests, cross-platform
verification, security review, required CI, Definition of Done, and human
review pass. M4 completion does not assert that production v2 activation has
passed. Do not begin M5 until the M4 gate is green.
