# M3 Final Security and DoD Review

**Date:** 2026-10-08  
**Branch:** `m3/local-projection-search`  
**Candidate HEAD reviewed:** `ecf58fb` (documentation closeout; behavior code `6b61882`)  
**Review result:** PASS for M3 implementation scope; pending human owner review

## Authority reviewed

- `docs/00_MASTER_SPEC.md`
- `docs/milestones/M3_LOCAL_PROJECTION_SEARCH.md`
- `docs/21_DEFINITION_OF_DONE.md`
- `docs/04_EVENT_SCHEMA_AND_INVARIANTS.md` and `docs/06_STORAGE_ARCHITECTURE.md`
- Accepted ADR-0013 and ADR-0016
- M3 Slice 8 security findings and current projection/daemon implementation

## Disposition

| Requirement | Result | Evidence / review note |
|---|---|---|
| SQLite/FTS is derived, disposable state | PASS | Projection is stored under `derived/projection`; replace/rebuild paths are scoped to the projection database and its SQLite sidecars. |
| Canonical event data remains authoritative and immutable | PASS | Daemon feed uses the existing verified canonical reader; projection does not parse or mutate `.open`/`.seg` packs. |
| Single-writer and read consistency boundaries | PASS | `cryptexd` owns projection feed; the shared per-device canonical lock spans verified feed discovery, recovery, scan, and callback. Lock order is canonical device lock then SQLite. |
| Search behavior and query safety | PASS | Text and metadata predicates use bound SQL parameters; FTS/query and time-bound errors are typed `InvalidQuery`; results use exact UTC instant ordering. |
| Rebuild, checkpoint, and corruption behavior | PASS | Missing/corrupt/unusable derived state, FTS external-content mismatch, checkpoint recovery, sealed/open feed behavior, and canonical preservation have regression coverage. Transient I/O and SQLite failures are non-replaceable. |
| SQLite corruption classification | PASS | Both open and FTS mappers use SQLite result/extended codes only. Misleading `SystemIoFailure` messages remain non-replaceable and preserve DB/sidecars. The fixed-codec regressions and genuine-corruption rebuild cases pass. |
| Cross-platform verification | PASS | Windows local checks and owner-reported macOS four-check pass at exact branch HEAD `4a2ea56`; final Windows/macOS readiness and Windows IPC ACL pass in [Actions run 37780668640](https://github.com/gungortarik/cryptex/actions/runs/37780668640). |
| UI/privacy review | N/A for M3 | No Desktop/MCP rendering surface was added; broad Desktop search UI is outside M3 scope. Returned note text must still be escaped/sanitized by any future renderer per the security rules. |
| Required documentation | PASS | Mac verification, final Actions result, corruption-finding closure, and handoff are recorded in M3 evidence. |

## Remaining bounded limitation

`SearchQuery.limit = None` collects every matching row in memory to preserve
exact UTC ordering. The API documents this behavior and supports `limit`, which
retains O(limit) results. Callers handling large stores should set a limit.
This is not a blocker under the M3 contract, which requires correct local search
and does not mandate a bounded default result count.

## Review boundary and status

No remaining blocking security or M3 DoD finding was identified in this
scoped review. This is an implementation/security review against repository
requirements, not a penetration test or a claim of production readiness.

**Human owner review: PENDING.** M3 remains IN PROGRESS and is not merged.
Do not begin M4 until the owner review is recorded and the M3 completion gate
is explicitly closed.
