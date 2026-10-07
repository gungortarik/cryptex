# M2 Kickoff Plan — Immutable Event Store

**Date:** 2026-10-07

**Branch:** `m2/immutable-event-store`

**Base tip:** `0c84c5c6a2872480ac4bbc82ebb1541773ddfb75` (`origin/m0/foundation` M1 documentation closeout)

**M1 merge ancestor:** `c2168f3edbd4deb00d5e16f5f35643948028b0ca` (PR #6) — verified ancestor of HEAD

**Implementer:** Cursor (sole write-capable M2 agent)

**Phase:** 1 — ADR-0016 ACCEPTED; product slices authorized

## 1. Authority

- `AGENTS.md`
- `docs/00_MASTER_SPEC.md`
- `docs/milestones/M2_IMMUTABLE_EVENT_STORE.md`
- `docs/04_EVENT_SCHEMA_AND_INVARIANTS.md`
- `docs/06_STORAGE_ARCHITECTURE.md`
- `docs/08_SECURITY_DEVICE_TRUST_AND_RECOVERY_ROOT.md`
- `docs/17_TEST_AND_FAILURE_MATRIX.md`
- `docs/21_DEFINITION_OF_DONE.md`
- ADR-0013 ACCEPTED (sealed per-device segments)
- ADR-0014 ACCEPTED (P1 SecureStore)
- ADR-0015 ACCEPTED (Brain/device/sequence foundations)
- PROPOSED: `docs/adr/ADR-0016-immutable-event-store-productization.md`

## 2. Branch / base verification (2026-10-07)

| Check | Result |
|---|---|
| Active branch | `m2/immutable-event-store` |
| HEAD | `274b114` — M2 durability hardening (base `0c84c5c`) |
| Tracks remote | `origin/m2/immutable-event-store` @ same tip; clean tree at kickoff |
| `c2168f3` ancestor of HEAD | yes (M1 merge integrated) |
| Base tip `0c84c5c` (M1 docs closeout); branch tip advances with M2 slices | yes |
| Working tree at verification | clean |

## 3. Repository audit — reusable foundations

### 3.1 Present and reusable

| Area | Location | Reuse notes |
|---|---|---|
| Sealed-segment packaging architecture | ADR-0013 + `tools/m0-storage-bench` | Architecture accepted. Bench proves `.open`/`.seg`, length-prefixed records, crash truncated-tail recovery, seal rename. **Benchmark framing is not an accepted production wire format.** |
| Atomic durable public-metadata writes | `services/cryptexd/src/metadata_persistence.rs` | Synced temp + exclusive lock + atomic replace patterns for mutable metadata; adapt carefully for append-only segment tails (different durability shape). |
| Identity / sequence attribution | `crates/core` + cryptexd sequence/install/identity runtimes | `brain_id`, `device_id`, `install_id`, first usable sequence `1`, fail-closed missing/corrupt counters, path-safe device filenames. |
| Schema-version fail-closed helper | `crates/core/src/validate.rs` (`require_exact_schema`) | Pattern for exact schema refusal; M2 event versioning policy still unsettled. |
| Typed errors | `crates/core/src/error.rs`, cryptexd error modules | Extend; do not silently swallow integrity/schema failures. |
| SecureStore private material | cryptexd `secure_store` (ADR-0014 P1) | Holds opaque device private blobs; **does not select signing algorithm**. |
| Writer boundary invariant | Master Spec / AGENTS / M1 daemon ownership | Product writes remain `cryptexd`-only. |
| Planned package slots | `crates/events`, `crates/storage`, `crates/security`, `packages/schemas` | Placeholders (`.gitkeep` only). |

### 3.2 Absent / not productized

- No product event domain types
- No JSON Schema / TypeScript generation pipeline
- No canonical segment writer in `cryptexd`
- No hash / signature / integrity-chain implementation
- No quarantine product path
- No crypto library pins for events (no ed25519/blake/sha product deps today)
- No `evidence/m2/` prior to this kickoff

### 3.3 Explicitly unauthorized to invent (must not ship without ADR)

Per `docs/04` §22, `docs/06` §16, `docs/08` §21:

1. Exact event field names
2. Exact serialization format
3. Exact hash / signature library and algorithms
4. Exact event type enum
5. Exact schema-version encoding / compatibility window
6. Exact quarantine directory/layout and metadata format
7. Final physical event/segment naming
8. Production per-device segment thresholds / seal-roll policy

Also: do **not** freeze P0-1 `segment_events=1000` as a production constant (ADR-0013).

## 4. Decision gates

| Gate | Meaning |
|---|---|
| **ADR-0016 review** | Owner accepts / revises / rejects the productization choices. Status remains PROPOSED until accepted. |
| **Product implementation** | Only after ADR-0016 (or accepted narrower successors covering the blocking choices) is ACCEPTED. |
| **M2 completion** | Scope + applicable failure/security/cross-platform tests + required CI + DoD + security review + owner review. Then M3 may begin. |

## 5. Implementation plan (post-acceptance slices)

Ordered after ADR-0016 acceptance. Each slice: behavior + tests + local format/lint/tests + evidence/handoff update + commit + push. No routine Actions; required workflow once at final candidate.

1. **Workspace scaffolding** — productize `crates/events` / `crates/storage` (+ security surface as decided); keep runtime data outside the repo; wire workspace members without inventing wire policy beyond the accepted ADR.
2. **Rust event domain types + schema generation** — Rust source of truth; generate JSON Schema + TypeScript bindings into `packages/schemas` (or accepted path); add drift check scaffolding.
3. **Integrity primitives** — content hash, device signature over accepted bytes, integrity-chain link rules; typed failures; no custom crypto.
4. **Sealed per-device segment store** — active `.open` append with sync; seal rename to immutable pack; configurable seal threshold (default may reuse 1000 **only as tunable config**, never protocol constant); per-device directory layout from ADR.
5. **cryptexd canonical writer API** — sole supported write path; allocate device sequence via existing M1 allocator; bind `brain_id`/`device_id`; refuse non-daemon mutation paths.
6. **Validation + quarantine** — corrupt / incompatible / unverifiable input never trusted; explicit quarantine with visible failure; truncated active-tail recovery consistent with ADR-0013 evidence.
7. **Schema-version behavior** — exact / compatible / reject / quarantine per accepted ADR rules.
8. **Cross-platform + failure matrix** — Windows local first; Mac parity when required; atomic/crash/integrity/quarantine/schema tests from `docs/17`.
9. **Final gate** — one required CI run, security review, DoD checklist, owner review. Do not merge without that package.

### Independent work allowed while ADR-0016 is PROPOSED

- Handoff / evidence / ADR proposal maintenance
- Audit notes and test-plan drafting
- Non-product documentation that does not pin unauthorized formats
- **Not allowed:** product Rust/TS that selects serialization, crypto, quarantine layout, segment naming, or seal policy

## 6. Non-goals (hard)

- M3 SQLite / FTS / projection productization
- M4 knowledge consolidation
- M8 replication / pairing / revocation product behavior
- M10 Forget/Purge product workflows
- Treating Syncthing transport success as semantic acceptance
- Editing existing canonical events in place
- Claiming M2 complete without final evidence/CI/security/owner review

## 7. Continuation point

**Now:** ADR-0016 **ACCEPTED**. Slices 1–5 + durability hardening landed. Next: cryptexd canonical writer API. No GitHub Actions until the final required M2 CI gate.


**Historical note:** Operational handoff previously retained contradictory M1 “review open / merge pending” wording after PR #6 merged. That residue is labeled historical/superseded in `docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md`. M1 remains COMPLETE at `c2168f3` / docs closeout `0c84c5c`.
