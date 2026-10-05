# M0 P0-1 — Runbook: 500k Segments Syncthing Cells

**Status:** COMPLETE FOR TIMED CELLS — Cell A and Cell B VERIFIED; ADR-0013 ACCEPTED; folders not retired by this file  
**Architecture effect:** packaging architecture is recorded in ADR-0013; this runbook does not complete P0-4 or M0

`p0-1-500k-files-macos-to-windows-r2` is **complete** (controlled timeout / files impracticality evidence). Do not schedule another 500k files retry. This runbook does not execute the segments cells by itself.

## Purpose

Exact checklist for the four 500k segments Syncthing measurements planned for P0-1 closure:

1. 500k segments macOS → Windows — initial — **VERIFIED**
2. same cell — approved +100 incremental — **VERIFIED**
3. 500k segments Windows → macOS — initial — **VERIFIED**
4. same cell — approved +100 incremental — **VERIFIED**

## Script capability verification (read-only)

No benchmark implementation change is required for these cells.

| Script | 500k / segments support |
|---|---|
| `run-syncthing-p0-1-initial-sync.mjs` | `events` ∈ {10000,100000,**500000**}; `layout=segments` |
| `run-syncthing-p0-1-incremental-sync.mjs` | `baseline-events=500000`; `layout=segments`; `--segment-events 1000` |
| `prepare-syncthing-p0-1-initial-sync.mjs` | folder-id + expected-files/bytes only |
| `plan-syncthing-p0-1-matrix.mjs` | 500k segments expected files = **500**, bytes = **518000000** |
| `capture-syncthing-p0-1-raw-result.mjs` | UTF-8 + BOM UTF-16LE |
| `retire-syncthing-p0-1-folder.mjs` | after incremental recorded (do not retire early) |

## Shared constants

```text
layout=segments
events=500000
segment_events=1000
payload_bytes=1024
expected_files_initial=500
expected_bytes_initial=518000000
incremental_events=100
incremental_logical_bytes=103600
expected_events_after_append=500100
expected_bytes_after_append=518103600
# expected_files_after_append: MEASURE after append (do not assume); prior 100k was 101
syncthing_version=v2.1.5
transport_scope=explicit_direct_tcp_only
```

Retained corpora (copy only; do not mutate):

```text
macos_segments_corpus=~/Library/Application Support/Cryptex-Dev/benchmarks/p0-1/macos-500000-20260929T165249Z/data/segments-500000
windows_segments_corpus=%LOCALAPPDATA%\Cryptex-Dev\benchmarks\p0-1\windows-500000-20260929T203439Z\data\segments-500000
```

Folder IDs (unique; never reuse failed IDs):

```text
p0-1-500k-segments-macos-to-windows
p0-1-500k-segments-windows-to-macos
```

Recommended timeout: start with `--timeout-seconds 7200` (segments historically finish far sooner; raise only if a clean cell approaches the limit).

---

## Cell A — macOS → Windows

### A0. Preconditions

- [x] `p0-1-500k-files-macos-to-windows-r2` finished (controlled timeout / impracticality; no further 500k files retry)
- [ ] Peer connection verified (direct TCP only)
- [ ] Prior conflicting folder retired from config if needed (`retire-syncthing-p0-1-folder.mjs`, no `--delete-work-path` unless forensic policy says otherwise)
- [ ] Retained macOS segments corpus verified (`verify-retained-corpus.mjs`)

### A1. Destination prepare (Windows)

- [ ] `prepare-syncthing-p0-1-initial-sync.mjs --role destination`  
  `folder-id=p0-1-500k-segments-macos-to-windows`  
  empty receiveonly destination under `syncthing-p0-1-runs/.../destination`

### A2. Source prepare (macOS)

- [ ] `prepare-syncthing-p0-1-initial-sync.mjs --role source`  
  `--source-corpus` = macOS retained segments-500000  
  `--expected-files 500 --expected-bytes 518000000`  
  source sendonly, paused, watcher disabled

### A3. Timed initial

- [x] On macOS source machine: timed initial runner `EXIT_CODE=0`
- [x] Independent Windows destination post-check: idle, local/global 500 / 518000000, physical 500 / 518000000, needs 0
- [x] Record evidence `syncthing-500k-segments-macos-to-windows.md` (VERIFIED)

### A4. Timed incremental (+100)

- [x] Valid initial on **same** folder ID
- [x] `append_setup_ms` recorded outside timed window; not combined with `incremental_sync_total_ms`
- [x] Post-append physical object count measured: 501 / 518103600
- [x] Windows post-check matches measured post-append counts
- [x] Record `syncthing-500k-segments-macos-to-windows-incremental.md` (VERIFIED)

Do **not** retire `p0-1-500k-segments-macos-to-windows` yet.

---

## Cell B — Windows → macOS

### B0. Preconditions

- [x] Cell A incremental recorded (or operators accept parallel only if homes/folders do not collide — default: serialize after A)
- [ ] Retained Windows segments corpus verified
- [ ] Prior Windows source cell retired from config if required

### B1. Destination prepare (macOS)

- [ ] receiveonly empty destination for `p0-1-500k-segments-windows-to-macos`

### B2. Source prepare (Windows)

- [ ] sendonly paused source from Windows `segments-500000` corpus  
  `--expected-files 500 --expected-bytes 518000000`

### B3. Timed initial

- [x] Timed initial on Windows source `EXIT_CODE=0`
- [x] macOS destination post-check: idle, 500 / 518000000, physical 500 / 518000000, needs 0
- [x] Record `syncthing-500k-segments-windows-to-macos.md` (VERIFIED)

### B4. Timed incremental (+100)

- [x] Same folder ID; approved workload
- [x] Post-append physical objects measured: 501 / 518103600
- [x] macOS post-check matches post-append counts
- [x] Record `syncthing-500k-segments-windows-to-macos-incremental.md` (VERIFIED)

Do **not** retire `p0-1-500k-segments-windows-to-macos` yet.

---

## Completion predicates (all four timed runs)

```text
EXIT_CODE=0
peer_completion=100
peer_need_items=0
peer_need_bytes=0
peer_need_deletes=0
source_paused_after_completion=true
transport_scope=explicit_direct_tcp_only
```

Initial additionally:

```text
source_local_files=500
source_local_bytes=518000000
source_global_files=500
source_global_bytes=518000000
source_global_files_verified=500
scope=isolated_p0_1_initial_syncthing_scan_sync_measurement
```

Incremental additionally:

```text
source_local/global files+bytes == measured post-append expectations
scope=isolated_p0_1_incremental_syncthing_scan_sync_measurement
```

## After all four VERIFIED

- [x] Update `evidence/m0/p0-1/README.md` index
- [x] Update `syncthing-initial-sync-protocol.md` execution status
- [x] Update `syncthing-incremental-workload-APPROVED-FOR-MEASUREMENT.md`
- [x] Revisit `docs/adr/ADR-0013-physical-event-storage-layout.md` for ACCEPTED — recorded 2026-10-02; M0 remains incomplete
