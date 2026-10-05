# M0 P0-1 — Incremental Syncthing Measurement (10k files, macOS→Windows, r3)

**Status:** VERIFIED TIMED MEASUREMENT  
**Scope:** approved incremental workload on valid initial-sync cell `p0-1-10k-files-macos-to-windows-r3`  
**Architecture effect:** none; does not select a layout or accept ADR-0013

## Prerequisite

Valid initial sync for this folder ID is recorded in:

- `evidence/m0/p0-1/syncthing-10k-files-macos-to-windows-r3.md`

Workload protocol:

- `evidence/m0/p0-1/syncthing-incremental-workload-APPROVED-FOR-MEASUREMENT.md`

## Timed runner output (raw)

```text
home=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1
local_device_id=MHS264T-3YLCW4F-GL35TOB-DXZ6SUX-4MVY7YK-AZZFNZI-XFOXHN3-DYMMEQM
peer_id=G3IQV46-V6WYJ2U-WDLQQOC-FXMJBGA-QRIXPNV-GVARBCC-IHYTOBI-BOIG6QA
folder_id=p0-1-10k-files-macos-to-windows-r3
folder_path=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/p0-1-10k-files-macos-to-windows-r3/source
layout=files
baseline_events=10000
incremental_events=100
payload_bytes=1024
source_platform=macos
destination_platform=windows
syncthing_version=v2.1.5
append_setup_ms=909.490
append_command_events=10100
expected_files_after_append=10100
expected_bytes_after_append=10463600
incremental_logical_bytes=103600
started_at_utc=2026-09-30T19:03:21.464Z
scan_completed_at_utc=2026-09-30T19:03:21.997Z
completed_at_utc=2026-09-30T19:03:23.814Z
incremental_scan_ms=533.633
incremental_sync_total_ms=2344.387
source_local_files=10100
source_local_bytes=10463600
source_global_files=10100
source_global_bytes=10463600
peer_completion=100
peer_global_items=10101
peer_need_items=0
peer_need_bytes=0
peer_need_deletes=0
source_global_files_verified=10100
source_paused_after_completion=true
transport_scope=explicit_direct_tcp_only
scope=isolated_p0_1_incremental_syncthing_scan_sync_measurement
```

Shell wrapper:

```text
INCREMENTAL_START_UTC=2026-09-30T19:03:19Z
INCREMENTAL_END_UTC=2026-09-30T19:03:23Z
EXIT_CODE=0
```

Runtime capture:

```text
~/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/p0-1-10k-files-macos-to-windows-r3/incremental-runner-capture.txt
```

## Notes

- Append of 100 events occurred while source was paused; append time is setup (`append_setup_ms`), outside timed sync.
- This working folder must not be reused as a fresh initial-sync cell.
- Retained corpus under `benchmarks/p0-1/` was not modified by append.
