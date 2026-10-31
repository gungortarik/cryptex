# M0 P0-1 — Incremental Syncthing Measurement (10k segments, Windows→macOS)

**Status:** VERIFIED TIMED MEASUREMENT  
**Scope:** approved incremental workload on valid initial-sync cell `p0-1-10k-segments-windows-to-macos`  
**Architecture effect:** none; does not select a layout or accept ADR-0013

## Prerequisite

Valid initial sync:

- `evidence/m0/p0-1/syncthing-10k-segments-windows-to-macos.md`

Approved workload:

- `evidence/m0/p0-1/syncthing-incremental-workload-APPROVED-FOR-MEASUREMENT.md`

Logical workload (same as files cells):

```text
incremental_events=100
payload_bytes=1024
segment_events=1000
incremental_logical_bytes=103600
operation=append_only_into_active_or_new_segment_objects
```

## Timed runner output (raw)

```text
home=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1
local_device_id=G3IQV46-V6WYJ2U-WDLQQOC-FXMJBGA-QRIXPNV-GVARBCC-IHYTOBI-BOIG6QA
peer_id=MHS264T-3YLCW4F-GL35TOB-DXZ6SUX-4MVY7YK-AZZFNZI-XFOXHN3-DYMMEQM
folder_id=p0-1-10k-segments-windows-to-macos
folder_path=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1-runs\p0-1-10k-segments-windows-to-macos\source
layout=segments
baseline_events=10000
incremental_events=100
payload_bytes=1024
segment_events=1000
source_platform=windows
destination_platform=macos
syncthing_version=v2.1.5
append_setup_ms=181.194
append_command_events=10100
expected_files_after_append=11
expected_bytes_after_append=10463600
incremental_logical_bytes=103600
started_at_utc=2026-09-30T19:59:00.565Z
scan_completed_at_utc=2026-09-30T19:59:00.833Z
completed_at_utc=2026-09-30T19:59:01.861Z
incremental_scan_ms=268.950
incremental_sync_total_ms=1294.808
source_local_files=11
source_local_bytes=10463600
source_global_files=11
source_global_bytes=10463600
peer_completion=100
peer_global_items=12
peer_need_items=0
peer_need_bytes=0
peer_need_deletes=0
source_global_files_verified=11
source_paused_after_completion=true
transport_scope=explicit_direct_tcp_only
scope=isolated_p0_1_incremental_syncthing_scan_sync_measurement
```

Shell wrapper:

```text
INCREMENTAL_START_UTC=2026-09-30T19:59:00Z
INCREMENTAL_END_UTC=2026-09-30T19:59:02Z
EXIT_CODE=0
```

Runtime capture:

```text
%LOCALAPPDATA%\Cryptex-Dev\syncthing-p0-1-runs\p0-1-10k-segments-windows-to-macos\incremental-runner-capture.txt
```

## Commands / tooling

```text
run-syncthing-p0-1-incremental-sync.mjs
bench-bin=d:\github_repos\cryptex\target\release\m0-storage-bench.exe
```

## Physical-object note (segments)

Post-append physical object count was **measured**, not assumed:

- before append: 10 sealed segment objects / 10360000 bytes / 10000 events
- after append: 11 physical objects / 10463600 bytes / 10100 events  
  (10 sealed + 1 new active `.open` containing the +100 records)

This working folder must not be reused as a fresh initial-sync cell.

## Notes

- Append of 100 events occurred while source was paused; append time is setup (`append_setup_ms`), outside timed sync.
- Retained corpus under `benchmarks/p0-1/windows-10000-20260929T055348Z/data/segments-10000` was not modified by append.
- For later comparison: Windows→macOS incremental_sync_total_ms ≈ 1.29s vs macOS→Windows segments ≈ 1.32s at the same +100/+103600 workload.
