# M0 P0-1 — Incremental Syncthing Measurement (10k segments, macOS→Windows)

**Status:** VERIFIED TIMED MEASUREMENT  
**Scope:** approved incremental workload on valid initial-sync cell `p0-1-10k-segments-macos-to-windows`  
**Architecture effect:** none; does not select a layout or accept ADR-0013

## Prerequisite

Valid initial sync:

- `evidence/m0/p0-1/syncthing-10k-segments-macos-to-windows.md`

Approved workload:

- `evidence/m0/p0-1/syncthing-incremental-workload-APPROVED-FOR-MEASUREMENT.md`

Logical workload (same as files cell):

```text
incremental_events=100
payload_bytes=1024
segment_events=1000
incremental_logical_bytes=103600
operation=append_only_into_active_or_new_segment_objects
```

## Timed runner output (raw)

```text
home=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1
local_device_id=MHS264T-3YLCW4F-GL35TOB-DXZ6SUX-4MVY7YK-AZZFNZI-XFOXHN3-DYMMEQM
peer_id=G3IQV46-V6WYJ2U-WDLQQOC-FXMJBGA-QRIXPNV-GVARBCC-IHYTOBI-BOIG6QA
folder_id=p0-1-10k-segments-macos-to-windows
folder_path=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/p0-1-10k-segments-macos-to-windows/source
layout=segments
baseline_events=10000
incremental_events=100
payload_bytes=1024
segment_events=1000
source_platform=macos
destination_platform=windows
syncthing_version=v2.1.5
append_setup_ms=424.346
append_command_events=10100
expected_files_after_append=11
expected_bytes_after_append=10463600
incremental_logical_bytes=103600
started_at_utc=2026-09-30T19:19:09.638Z
scan_completed_at_utc=2026-09-30T19:19:09.914Z
completed_at_utc=2026-09-30T19:19:10.963Z
incremental_scan_ms=276.749
incremental_sync_total_ms=1319.043
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
INCREMENTAL_START_UTC=2026-09-30T19:19:08Z
INCREMENTAL_END_UTC=2026-09-30T19:19:11Z
EXIT_CODE=0
```

Runtime capture:

```text
~/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/p0-1-10k-segments-macos-to-windows/incremental-runner-capture.txt
```

## Physical-object note (segments)

Post-append physical object count was **measured**, not assumed:

- before append: 10 sealed segment objects / 10360000 bytes / 10000 events
- after append: 11 physical objects / 10463600 bytes / 10100 events  
  (10 sealed + 1 new active `.open` containing the +100 records)

This working folder must not be reused as a fresh initial-sync cell.
