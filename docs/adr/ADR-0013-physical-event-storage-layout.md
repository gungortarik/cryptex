# ADR-0013 — Physical Immutable Event Storage Layout

**Status:** ACCEPTED  
**Date drafted:** 2026-10-01  
**Date accepted:** 2026-10-02  
**Spike:** M0 P0-1  
**Authority:** `docs/00_MASTER_SPEC.md`, `docs/23_REQUIRED_TECHNICAL_SPIKES.md`, `docs/18_IMPLEMENTATION_READINESS_PLAN.md`, `docs/20_MILESTONE_ROADMAP.md`, `docs/06_STORAGE_ARCHITECTURE.md`, `docs/milestones/M0_FOUNDATION.md`, `docs/30_ADR_INDEX.md`, `evidence/m0/p0-1/`

This record accepts the **physical packaging architecture** for immutable canonical events. It does not complete P0-4, does not define Cryptex replication semantics, and does not freeze benchmark profile numbers as production constants.

## 1. Context

Cryptex must choose physical packaging for immutable canonical events before M2. Candidates compared by M0 P0-1:

1. **A — one immutable file per event**
2. **B — immutable sealed per-device segment/pack files**
3. **C — hybrid** only if evidence from A/B justified it

Required scale matrix: 10k / 100k / 500k events on both macOS and Windows, including local filesystem and Syncthing transport measurements listed in `docs/23_REQUIRED_TECHNICAL_SPIKES.md`.

P0-1 measured a benchmark profile with `payload_bytes=1024` and `segment_events=1000`. That profile is evidence context, not a production constant.

## 2. Decision

**Accepted packaging architecture:** canonical durable events are stored as **immutable sealed per-device segment/pack files** (candidate B).

**Rejected:** one immutable file per event (candidate A), because it failed required top-scale Syncthing practicality and showed worse local enumeration/rebuild/allocation cost at 500k events.

**Not selected:** hybrid thresholded segments (candidate C). A/B evidence is conclusive; hybrid would add complexity without restoring a viable files layout at required scale.

**Tunable implementation parameters (not frozen by this ADR):**

- events per sealed segment / pack (`segment_events=1000` was the verified P0-1 benchmark profile only)
- on-disk directory layout and container filenames
- active-tail vs sealed-object naming
- exact seal/roll policy used by M2 productization

M2 must implement the accepted **architecture** (sealed per-device segments as the canonical physical packaging). M2 may choose production seal size and filenames from later product constraints, provided they remain sealed per-device immutable packs and remain practical on macOS and Windows.

## 3. Alternatives considered

| Alternative | Decision | Reason |
|---|---|---|
| A — one file per event | **Rejected** | Required 500k Syncthing initial sync did not complete within 21600s on macOS→Windows (`p0-1-500k-files-macos-to-windows-r2`, ~21.2% peer completion). That is controlled timeout / impracticality evidence, not a Syncthing crash. Local 500k enumeration/rebuild and Windows file-allocation overhead were also worse than segments. |
| B — sealed per-device segments | **Accepted** | Verified local FS + Syncthing at 10k/100k/500k on both platforms, both directions for segments, including approved +100 incremental. Meets scale; crash probes recover complete prior records in the active pack. |
| C — hybrid thresholded segments | **Not selected** | A/B are conclusive. Hybrid is not the simplest option that meets scale/recovery. |

## 4. Evidence summary (verified)

### 4.1 Local filesystem (macOS + Windows)

Verified for both layouts at 10k / 100k / 500k:

- append latency, directory enumeration, rebuild
- disk overhead (including Windows allocation evidence at 500k)
- crash probes

Sources: dated `evidence/m0/p0-1/macos-*` and `windows-*` files.

At 500k, segments used 500 physical objects vs 500000 for files, with substantially lower enumeration/rebuild cost. Windows 500k files file-allocation was ≈3.95× logical bytes; segments packed the same logical payload into far fewer objects.

### 4.2 Syncthing transport — files

| Cell | Classification |
|---|---|
| 10k files both directions (initial + incremental) | VERIFIED |
| 100k files both directions (initial + incremental) | VERIFIED |
| 500k files macOS→Windows original | FAILED / INVALID (sleep/hibernate) |
| 500k files macOS→Windows r2 | COMPLETED CONTROLLED TIMEOUT / IMPRACTICALITY (21600s; ~21.2% peer completion; not a crash; not a timing result) |
| 500k files reverse direction | Not required / not scheduled after r2 impracticality |

Illustrative verified timings:

- 100k files macOS→Windows initial ≈ 2.50e6 ms (~42 min)
- 100k files Windows→macOS initial ≈ 5.01e6 ms (~83 min)

### 4.3 Syncthing transport — segments

| Cell | Classification |
|---|---|
| 10k segments both directions (initial + incremental) | VERIFIED |
| 100k segments both directions (initial + incremental) | VERIFIED |
| 500k segments macOS→Windows initial + incremental | VERIFIED |
| 500k segments Windows→macOS initial + incremental | VERIFIED |

Illustrative verified timings:

- 500k segments macOS→Windows initial ≈ 1.84e4 ms (~18 s)
- 500k segments Windows→macOS initial ≈ 1.56e4 ms (~16 s)
- 500k segments incremental ≈ 1.2–1.3 s timed window (append setup excluded)

These are isolated P0-1 transport measurements (`transport_scope=explicit_direct_tcp_only`). They do **not** prove Cryptex pairing, trust, validation, conflict semantics, or managed-Syncthing product integration. P0-4 remains separate and incomplete.

### 4.4 Recovery / purge / corruption tradeoffs

| Criterion | Evidence | Accepted implication |
|---|---|---|
| Crash / truncated active tail | VERIFIED both layouts, both platforms | Segments recover complete prior records in the active pack; incomplete suffix is detected. Sufficient for recovery goals. |
| Purge complexity | VERIFIED local probes both platforms | Files: unlink one object per purged event. Segments: may rewrite/seal packs (higher packaging complexity). Acceptable: purge remains a later product milestone; packaging must not prevent pack-scoped rewrite. |
| Corruption blast radius | VERIFIED local probes both platforms | Files: per-object blast radius. Segments: pack-scoped blast radius for sealed objects. Acceptable against files’ scale failure. |

Spike rule: simplest option that meets **scale and recovery** on both platforms. Files fail required 500k Syncthing scale. Segments meet scale and remain rebuildable.

## 5. Platform implications

- macOS and Windows remain first-class; packaging is not OS-specific.
- Canonical text remains UTF-8 with LF; no symlink dependence for canonical data.
- Syncthing remains transport only. This ADR does not invent Cryptex replication semantics and does not complete P0-4.

## 6. Recovery / security implications

- Canonical events remain immutable; corrections are additional history, not in-place edits.
- Derived SQLite, search, and Obsidian projection remain rebuildable from canonical packs.
- Purge/forget product semantics remain later milestones; this ADR only fixes physical packaging complexity implications.
- Restricted memory / AI exposure rules are unchanged.
- Integrity, signatures, and quarantine remain M2 productization.

## 7. Acceptance status

| Field | Value |
|---|---|
| ADR-0013 | **ACCEPTED** |
| Layout selected | **immutable sealed per-device segment/pack files** |
| One-file-per-event | **rejected** |
| Hybrid evaluated | **no** (not justified; not selected) |
| `segment_events=1000` | **P0-1 benchmark profile only; not a production constant** |
| P0-1 spike | **complete** |
| P0-4 / M0 | **not complete at the 2026-10-02 ADR snapshot; superseded for current status by `evidence/m0/m0-closure-audit-20261005.md` and `checklists/M0_FOUNDATION_GATE.md` (M0 complete for synthetic foundation scope)** |

## 8. References

- `evidence/m0/p0-1/README.md`
- `evidence/m0/p0-1/syncthing-500k-files-macos-to-windows-r2-TIMED-FAILED.md`
- `evidence/m0/p0-1/syncthing-500k-segments-macos-to-windows.md`
- `evidence/m0/p0-1/syncthing-500k-segments-macos-to-windows-incremental.md`
- `evidence/m0/p0-1/syncthing-500k-segments-windows-to-macos.md`
- `evidence/m0/p0-1/syncthing-500k-segments-windows-to-macos-incremental.md`
- Verified 10k/100k Syncthing evidence and local `macos-*` / `windows-*` runs under `evidence/m0/p0-1/`
- `evidence/m0/p0-1/purge-complexity.md`
- `evidence/m0/p0-1/corruption-blast-radius.md`
