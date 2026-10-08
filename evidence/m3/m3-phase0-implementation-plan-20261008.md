# M3 Phase 0 Implementation Plan - Local Projection + Search

**Status:** Phase 0 complete (planning only; no product code, no dependency pins)  
**Date:** 2026-10-08  
**Branch:** `m3/local-projection-search` @ `038204f` (kickoff) -> this plan commit  
**Base:** `m0/foundation` M2 merge `817b6a126cf6e9cd3e8a27b1348ec977bdc9dcdd` (PR #8)  
**Implementer:** Cursor  
**Authority:** `AGENTS.md`, `docs/00_MASTER_SPEC.md`, `docs/03_MEMORY_MODEL.md`, `docs/04_EVENT_SCHEMA_AND_INVARIANTS.md`, `docs/06_STORAGE_ARCHITECTURE.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/21_DEFINITION_OF_DONE.md`, `docs/milestones/M3_LOCAL_PROJECTION_SEARCH.md`, accepted ADRs (0013-0016), `evidence/m3/m3-kickoff-plan-20261008.md`

---

## 1. Verdict - ADR gate

**No new ADR is required to begin M3 product implementation**, provided the work stays inside the accepted M3 contract and the boundaries below.

| Choice | ADR needed? | Why |
|---|---|---|
| SQLite + FTS5 as the local search projection | **No** | Already mandated by Master Spec section 9 and Storage Architecture section 10; M3 contract restates it. |
| Disposable DB under platform app-data roots | **No** | Placement roots are fixed (`Cryptex` / `Cryptex-Dev`). ADR-0015 layout note and Storage Architecture section 7 allow choosing exact subdirs under that root at implementation time, as long as the DB stays derived/disposable and outside the Git repo / visible home/Desktop/Documents. |
| Projection columns derived only from M2 `AcceptedEvent` / `memory.note` | **No** | Exact projection schemas belong to later milestones (`docs/03` section 18); M3 is that milestone. No new canonical event fields. |
| Bundled SQLite amalgamation with FTS5 enabled (vs system lib) | **No** (implementation pin) | Needed for deterministic Win/mac FTS5 availability. Treat as a workspace dependency pin at first product slice - same class as M2 crate pins after ADR-0016 - **not** a new architecture decision. |
| Inventing entity/fact/relation columns, vector search, or Syncthing of the DB | **Yes - stop** | Conflicts with M3 non-goals / derived-state boundary. |
| Making SQLite required for recovery or a second canonical write path | **Yes - stop** | Violates Master Spec / Storage Architecture recovery contract. |
| Changing M2 `.open`/`.seg` formats, seal policy, or event schema | **Yes - stop** | Out of M3 scope; ADR-0016 owns those. |

**Phase 0 does not pin `rusqlite`/SQLite versions.** First product slice may add an exact workspace pin once this plan is reviewed; if review rejects bundled FTS5 or the proposed path, revise the plan (or open an ADR) before coding.

---

## 2. Repository audit (merged M2)

### 2.1 Reusable today

| Area | Location | M3 use |
|---|---|---|
| Production event types | `crates/events` - `AcceptedEvent`, `EventType::MemoryNote`, `MemoryNotePayload { text }`, `Classification`, integrity helpers | Sole projection input shape |
| Record validation | `validate_accepted_record_bytes` | Trust boundary before any INSERT |
| Pack I/O | `cryptex-storage`: `fold_verified_frames`, `recover_active_open`, layout helpers | Streaming verified frame bodies |
| Verified reader | `cryptexd::canonical_writer::load_verified_events` | Existing fail-closed sealed-pack + active-open recovery path |
| Tip precedent | `writer_tip.json` under device canonical dir | Pattern: rebuildable derived cache; packs remain authoritative |
| App-data roots | Master Spec / `paths.rs` (`Cryptex` / `Cryptex-Dev`; tests under `Cryptex-Dev/...`) | Projection DB must live under the same selected root |
| Schema artifact | `packages/schemas/cryptex-event-v1.schema.json` | Documents M2 fields; **do not** invent a parallel canonical schema for projection |

### 2.2 Absent (expected)

- No SQLite / FTS / `rusqlite` / `libsqlite3` entries in workspace `Cargo.toml` or `Cargo.lock`
- No projection crate, path helper, checkpoint table, or search API
- No Desktop/MCP search UX (M13 / AI milestones)

### 2.3 Reader gap to close in product slices (not Phase 0)

`load_verified_events` is correct but loads all events into memory and lives beside the writer. M3 should:

1. **Extract or wrap** a daemon-owned **verified event iterator/scanner** (sealed fail-closed + open recover-then-prefix) that yields `AcceptedEvent` without requiring callers to reimplement quarantine rules.
2. Prefer **streaming project** over full `Vec` for large histories; tests may still use the full loader for small fixtures.
3. Keep **canonical mutation** exclusively on `append_memory_note` / seal paths - projection never appends frames.

---

## 3. Projection ownership and API boundary

```
verified canonical packs (.seg / recovered .open)
        |  read-only scan (M2 validation + chain rules)
        v
 cryptexd projection owner  --writes-->  disposable SQLite+FTS5
        |
        +-- project / rebuild / checkpoint APIs (daemon-internal + library)
        +-- search / metadata-filter queries (library first; IPC later if needed)
```

**Ownership**

- **`cryptexd` owns** projection open/rebuild/incremental update and is the only supported writer to the projection DB file(s).
- **Desktop / CLI / MCP / Obsidian / AI** must not treat direct SQLite file edits as authoritative; any future client query goes through daemon/library APIs.
- **`cryptex-storage` stays canonical-pack focused.** Prefer a new workspace crate (working name `cryptex-projection`) for SQL schema, FTS, checkpoint, and query helpers, depended on by `cryptexd`. Do not fold SQLite into `cryptex-events`.

**Narrow API surface (product slices)**

| Layer | Responsibility | Must not |
|---|---|---|
| Verified read | Enumerate trusted `AcceptedEvent`s for local device dirs | Project invalid/partial sealed packs; skip M2 quarantine rules |
| Projection write | Upsert rows + FTS; advance checkpoint; full rebuild | Write `.open`/`.seg`; mint events; "heal" canonical corruption |
| Search query | FTS5 match + metadata filters; typed errors | Require LLM; expose restricted-to-AI policy as M3 "done" (store classification; enforce AI defaults later) |

---

## 4. Minimal projection fields (from exact M2 schema)

Production M2 event coverage is **only** `memory.note`. Project only fields already on `AcceptedEvent` / payload.

### 4.1 Row identity and metadata (content table)

| Projection column | Source | Notes |
|---|---|---|
| `event_id` | `AcceptedEvent.event_id` | Primary key (UUID string) |
| `brain_id` | `brain_id` | Filter / sanity |
| `device_id` | `device_id` | Multi-device local union |
| `device_seq` | `device_seq` decimal string -> store as TEXT (+ optional INTEGER only if lossless for filters) | Preserve exact decimal |
| `created_at` | `created_at` RFC3339 | Metadata filter / sort |
| `event_type` | `event_type` (`memory.note`) | Ready for future types without schema invention now |
| `classification` | `classification` | Metadata filter; enables later AI exclusion |
| `content_hash` | `content_hash` | Provenance / rebuild fingerprint |
| `signing_key_id` | `signing_key_id` | Optional but cheap; aids diagnostics |
| `text` | `MemoryNotePayload.text` | FTS source; respect `MAX_MEMORY_NOTE_TEXT_BYTES` |

**Do not project in M3:** `prev_content_hash`, `signature`, entity/fact/relation fields, confidence, memory-state machines, Obsidian paths, vectors.

**Do not add** synthetic semantic columns "for convenience."

### 4.2 FTS5

- Content-sync FTS5 table over `text` (default: **text only**).
- `event_id` as FTS rowid mapping / content=`events` pattern so DELETE/rebuild stay consistent.
- Tokenizer: start with SQLite default unicode61 unless tests prove need for a custom tokenizer (custom tokenizer would re-open ADR review).

### 4.3 Metadata filters (M3)

Support conjunction of: full-text query (optional), `classification`, `event_type`, `device_id`, `created_at` range, optional `brain_id`.  
Relationships from Master Spec search section wait for M4 materialized graph - **out of M3**.

### 4.4 Projection meta / checkpoint tables

| Store | Purpose |
|---|---|
| `projection_meta` | `schema_version` (projection schema, **not** event schema_version), `built_at`, integrity/build id |
| `device_checkpoint` | Per `device_id`: last projected `device_seq`, last `content_hash`, last sealed index fully consumed, open-pack verified prefix length or equivalent monotonic cursor |

Projection `schema_version` is a **derived-schema** integer owned by the projection crate; bumping it forces rebuild. It must not be confused with `AcceptedEvent.schema_version == 1`.

---

## 5. App-data placement

**Roots (unchanged):**

- Production: macOS `~/Library/Application Support/Cryptex/`; Windows `%LOCALAPPDATA%\Cryptex\`
- Development: corresponding `Cryptex-Dev\`
- Tests: exclusive temp dirs under existing `Cryptex-Dev/...` patterns (`assert_outside_repository`)

**Proposed disposable layout (implementation choice, not ADR):**

```text
<app-data-root>/
  canonical/events/<device_id_hex>/   # M2 - authoritative
  quarantine/canonical-events/        # M2
  identity/...                        # M1/M2 - not disposable
  derived/
    projection/
      projection.sqlite               # primary DB (+ WAL/SHM as SQLite creates)
```

Rationale: keep derived state clearly separated from `canonical/` and `identity/`; single brain-local DB for all local device packs (simpler search); path stays private under app-data.

**Rules**

- Never create the DB inside the Git repo, Desktop, Documents, or visible home by default.
- Loss of `derived/projection/` must be healable by rebuild; must not delete or rewrite canonical packs.
- Do not replicate the SQLite file via Syncthing as Cryptex semantic state.

---

## 6. SQLite / FTS5 on Windows and macOS

| Requirement | Plan |
|---|---|
| First-class OS | Windows + macOS must both run the same projection tests |
| FTS5 availability | Use a **bundled** SQLite build with FTS5 enabled at compile time so hosts are not dependent on system SQLite compile flags |
| Likely crate family | `rusqlite` (or equivalent) with bundled feature set - **pin only after plan review**, in workspace.dependencies style consistent with M2 |
| Linking | Prefer static/bundled amalgamation for CI reproducibility; avoid silent fallback to a system lib lacking FTS5 |
| Verification | Startup/self-check: `PRAGMA compile_options` / `CREATE VIRTUAL TABLE ... USING fts5` smoke test fails closed with typed error if FTS5 missing |
| Concurrency | Single writer (daemon); readers via SQLite WAL; document that external processes must not write |

No custom cryptography; SQLite is not a secrecy boundary for restricted text (OS file ACLs on app-data apply as today).

---

## 7. Transaction, checkpoint, corruption, rebuild semantics

Align with `docs/17` section 9 and M3 verification list.

### 7.1 Trust input

- Project **only** events that pass the same verification path as writer rebuild: signature/schema validation, device_id binding, hash-chain / strictly-increasing `device_seq`, sealed packs fully consumed or quarantined+error.
- Never build trusted rows from a partial/invalid sealed pack.
- Active `.open`: recover (quarantine-before-truncate) then project verified prefix only; truncated tail is not projected.

### 7.2 Incremental vs rebuild

| Condition | Action |
|---|---|
| DB missing | Full rebuild |
| DB not SQLite / page corruption / `quick_check` fail | Report typed error; delete/replace disposable DB; full rebuild |
| `projection_meta.schema_version` unsupported | Full rebuild (or explicit migrate later - M3 default: rebuild) |
| Checkpoint behind canonical tip | Incremental project in device_seq order |
| Checkpoint references unknown hash/seq / ahead of packs | Fail closed -> full rebuild |
| Interrupted rebuild (crash mid-transaction) | Use SQLite transactions; incomplete txn rolls back; retry safe |
| Canonical event loss (purge later) | Out of M3; design rows keyed by `event_id` so a future purge can delete+rebuild |

### 7.3 Checkpoint semantics

- Persist checkpoint **in the same SQLite transaction** as the projected rows for that batch (atomic visibility).
- After successful full rebuild, checkpoint matches tip of all successfully verified local devices.
- Daemon restart: open DB -> integrity/schema check -> incremental catch-up -> serve queries.
- Projection lag is allowed; search results must not imply canonical acceptance of unverified data.

### 7.4 Failure transparency

- Typed errors for I/O, SQL, FTS missing, schema mismatch, canonical verification failure.
- Logs: no memory bodies, secrets, or recovery material by default (AGENTS.md).
- Do not silently serve a half-built index as complete without surfacing rebuild/attention state to callers (library-level status flag is enough for M3; Desktop health is later).

---

## 8. Tests and platform verification

### 8.1 Required behavior tests (local first)

1. **Clean build** - append N `memory.note` via canonical writer -> project -> row count, text, metadata match.
2. **FTS5** - query matches substrings; non-matches empty; unicode smoke on Win+mac.
3. **Metadata filters** - classification / device_id / created_at range / event_type alone and with FTS.
4. **Missing DB rebuild** - delete `projection.sqlite*` -> rebuild -> same search results; canonical packs unchanged.
5. **Corrupt DB rebuild** - truncate/overwrite DB bytes -> detect -> rebuild -> results restored; canonical unchanged.
6. **Checkpoint / restart** - project subset, restart catch-up with new appends, no duplicate `event_id`s.
7. **Sealed vs open** - events in `.seg` and recovered `.open` both appear; truncated open tail excluded.
8. **Fail-closed sealed corruption** - corrupted `.seg` does not populate trusted projection (mirrors M2 quarantine behavior).
9. **Multi-device local** - two device dirs under one app-data root -> union search with `device_id` filter.
10. **Idempotent rebuild** - two full rebuilds -> stable counts; disposable path only under app-data.

### 8.2 Cross-platform / CI

- Windows host: crate + `cryptexd` lib tests for projection (fmt, clippy `-D warnings`, test).
- macOS parity before final gate (same four-check discipline as M2).
- **Do not run GitHub Actions in Phase 0.** One required final M3 workflow only after a reviewed candidate (wire `cryptex-projection` into `.github/workflows/ci.yml` in a later slice, analogous to M2 events/storage).

### 8.3 Explicit non-tests for M3

- Desktop Cmd/Ctrl+K UX, MCP tool surface, semantic/vector search, M4 consolidation, Syncthing of DB, purge workflows.

---

## 9. Proposed implementation slices (post Phase 0 review)

Ordered; each: code + tests + evidence note + handoff update; **no Actions until final candidate**.

1. **Scaffold** - `cryptex-projection` crate + workspace member; path helpers under `derived/projection/`; no SQL pin until this plan's ADR verdict stands.
2. **SQLite pin + FTS5 smoke** - exact dependency pin; Win/mac compile-options/FTS self-check tests.
3. **Schema + empty open/create** - content + FTS + meta/checkpoint tables; schema_version = 1.
4. **Verified feed integration** - stream/iterate M2-verified events into projection transactions.
5. **Rebuild + incremental + corruption** - docs/17 section 9 matrix.
6. **Search API** - FTS + metadata filters; typed errors; classification stored for later AI policy.
7. **Cross-platform + CI wire** - local both-OS; then single final Actions run after review.
8. **DoD / security / owner review** - complete M3 gate; only then M4.

---

## 10. Non-goals (hard)

- M4 entity/fact/relation productization
- M8 replication / pairing product behavior
- M10 forget/purge product workflows
- M13 broad Desktop search UX
- Vector/semantic search
- Mutating canonical `.open`/`.seg` from projection code
- Treating projection DB as recovery-required state
- Dependency pins or frozen SQL DDL in this Phase 0 commit

---

## 11. Continuation point

**Now:** Phase 0 plan ready for owner/reviewer acceptance.

**Next (after review, still no Actions):** Slice 1 scaffolding only if the ADR verdict above remains "no ADR required." If review demands a different app-data layout, non-bundled SQLite, or schema beyond M2 fields, stop and revise this plan or file an ADR before product code.
