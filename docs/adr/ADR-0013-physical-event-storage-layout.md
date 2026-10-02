# ADR-0013 — Physical Immutable Event Storage Layout

**Status:** PROPOSED / PENDING 500k segment Syncthing evidence  
**Date drafted:** 2026-10-01  
**Spike:** M0 P0-1  
**Authority:** `docs/23_REQUIRED_TECHNICAL_SPIKES.md`, `docs/06_STORAGE_ARCHITECTURE.md`, `docs/milestones/M0_FOUNDATION.md`, `docs/30_ADR_INDEX.md`

> **DRAFT ONLY.** This record does **not** accept ADR-0013.  
> **No production layout is selected.**  
> Do not treat any candidate as frozen until remaining mandatory Syncthing evidence is recorded and this ADR is explicitly marked ACCEPTED.

## 1. Context

Cryptex must choose physical packaging for immutable canonical events before M2. Candidates under comparison:

1. **A — one immutable file per event**
2. **B — immutable sealed per-device segment/pack files** (`segment_events=1000` in the P0-1 profile)
3. **C — hybrid** only if evidence from A/B justifies it

Required scale matrix: 10k / 100k / 500k events on both macOS and Windows, including local filesystem and Syncthing transport measurements.

## 2. Decision (current)

**No final layout decision.**

Working hypothesis from verified evidence so far (subject to remaining 500k segments Syncthing measurements):

- Candidate **B (segments)** is currently the leading pragmatic option for Syncthing-scale transport and local enumeration/rebuild cost.
- Candidate **A (files)** remains practical at 10k and completed at 100k, but shows large Syncthing initial-sync cost that worsens sharply with scale. The 500k files macOS→Windows r2 cell is **completed controlled timeout / impracticality evidence** (`p0-1-500k-files-macos-to-windows-r2`, 21600s, ~21.2% peer completion). That is not a Syncthing crash, not a timing result, and does not by itself accept this ADR. No further 500k files retry is scheduled.
- Candidate **C (hybrid)** is **not** justified yet; do not evaluate unless A/B remain inconclusive after mandatory evidence closes.

## 3. Alternatives considered

| Alternative | Status in this draft |
|---|---|
| A — one file per event | Under evaluation; strong Syncthing cost signal against it at ≥100k; 500k macOS→Windows r2 closed as controlled timeout / impracticality |
| B — sealed segments | Leading on verified local + Syncthing evidence through 100k; **500k Syncthing still required** |
| C — hybrid thresholded segments | Deferred; only if A/B cannot satisfy scale/recovery together |

## 4. Evidence summary (verified only)

### 4.1 Local filesystem (macOS + Windows)

Verified for both layouts at 10k / 100k / 500k:

- append latency, directory enumeration, rebuild
- disk overhead (including Windows allocation evidence at 500k)
- crash probes

Sources: dated `evidence/m0/p0-1/macos-*` and `windows-*` files.

Trend recorded in those runs: segments generally lower append/enumeration/rebuild/allocation cost than files at the same scale. That trend is evidence, not acceptance.

### 4.2 Syncthing transport — files

| Cell | Classification |
|---|---|
| 10k files both directions (initial + incremental) | VERIFIED |
| 100k files both directions (initial + incremental) | VERIFIED |
| 500k files macOS→Windows original | FAILED / INVALID (sleep/hibernate) |
| 500k files macOS→Windows r2 | **COMPLETED CONTROLLED TIMEOUT / IMPRACTICALITY** (21600s; ~21.2% peer completion; not a crash; not a timing result; no further files retry) |

Illustrative verified timings (exact values live in evidence files):

- 100k files macOS→Windows initial ≈ 2.50e6 ms (~42 min)
- 100k files Windows→macOS initial ≈ 5.01e6 ms (~83 min)
- 100k files incremental remains seconds-to-tens-of-seconds after append setup

### 4.3 Syncthing transport — segments

| Cell | Classification |
|---|---|
| 10k segments both directions (initial + incremental) | VERIFIED |
| 100k segments both directions (initial + incremental) | VERIFIED |
| 500k segments any direction | **NOT YET EXECUTED** |

Illustrative verified timings:

- 100k segments macOS→Windows initial ≈ 1.64e4 ms (~16 s)
- 100k segments Windows→macOS initial ≈ 3.79e3 ms (~3.8 s)
- 100k segments incremental ≈ 1.2–1.3 s timed window

### 4.4 Recovery / purge / corruption tradeoffs

| Criterion | Evidence status | Draft tradeoff reading |
|---|---|---|
| Crash / truncated active tail | VERIFIED probes both layouts | Segments recover complete prior records in active pack; files isolate per-object failure |
| Purge complexity | VERIFIED local probes both platforms | Files: one object per purged event. Segments: may rewrite/seal packs → higher packaging complexity for selective purge |
| Corruption blast radius | VERIFIED local probes both platforms | Files: smaller per-object blast radius. Segments: larger pack-scoped blast radius for sealed objects |

These tradeoffs favor **files** for fine-grained purge/corruption isolation and **segments** for Syncthing/enumeration scale. ADR acceptance must weigh whether segments’ blast-radius/purge cost is acceptable against files’ Syncthing impracticality at required top scale.

## 5. Exact remaining evidence required for acceptance

Mandatory before this ADR may be marked ACCEPTED:

1. Record outcome of **`p0-1-500k-files-macos-to-windows-r2`** — **DONE** as completed controlled timeout / impracticality (`evidence/m0/p0-1/syncthing-500k-files-macos-to-windows-r2-TIMED-FAILED.md`). Do not convert the timeout wall window into a timing result. Do not schedule another 500k files retry.
2. **500k segments macOS→Windows** initial Syncthing measurement — VERIFIED
3. Approved **+100-event incremental** on that same segments cell — VERIFIED
4. **500k segments Windows→macOS** initial Syncthing measurement — VERIFIED  
   *(recommended by evidence-protocol parity with 10k/100k; treat as required for this closure plan unless a later audit explicitly narrows to one direction)*
5. Approved **+100-event incremental** on that reverse segments cell — VERIFIED
6. Re-read local FS + purge + corruption evidence against the chosen candidate
7. Update this ADR to ACCEPTED with selected layout, rejected alternatives, and platform/recovery implications

Not required by Master Spec given r2's controlled-timeout impracticality signal, and **not scheduled**:

- 500k files reverse-direction initial/incremental
- additional files retries beyond r2
- hybrid experiments

## 6. Platform implications (draft)

- Both platforms remain first-class; no OS-only packaging.
- Syncthing remains transport only; packaging choice must not invent Cryptex replication semantics (P0-4 separate).

## 7. Recovery / security implications (draft)

- Packaging must preserve immutable event history and rebuildability of derived state.
- Purge/forget semantics remain product milestones; this ADR only chooses physical packaging complexity implications.
- Restricted memory / AI exposure rules unchanged by packaging.

## 8. Acceptance status

| Field | Value |
|---|---|
| ADR-0013 | **PROPOSED / PENDING** |
| Layout selected | **none** |
| Hybrid evaluated | **no** |
| Ready for ACCEPTED | **no** |

## 9. References

- `evidence/m0/p0-1/README.md`
- `evidence/m0/p0-1/syncthing-initial-sync-protocol.md`
- `evidence/m0/p0-1/syncthing-500k-files-macos-to-windows-r2-TIMED-FAILED.md`
- `evidence/m0/p0-1/syncthing-500k-segments-runbook.md` (DRAFT)
- Verified 10k/100k Syncthing evidence files under `evidence/m0/p0-1/`
- `evidence/m0/p0-1/purge-complexity.md`
- `evidence/m0/p0-1/corruption-blast-radius.md`
