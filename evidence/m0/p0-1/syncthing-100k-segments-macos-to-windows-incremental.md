# M0 P0-1 — Incremental Syncthing Measurement (100k segments, macOS→Windows)

**Status:** VERIFIED TIMED MEASUREMENT  
**Scope:** approved incremental workload on valid initial-sync cell `p0-1-100k-segments-macos-to-windows`  
**Architecture effect:** none; does not select a layout or accept ADR-0013

## Prerequisite

Valid initial sync:

- `evidence/m0/p0-1/syncthing-100k-segments-macos-to-windows.md`

Approved workload:

- `evidence/m0/p0-1/syncthing-incremental-workload-APPROVED-FOR-MEASUREMENT.md`

Logical workload:

```text
incremental_events=100
payload_bytes=1024
segment_events=1000
incremental_logical_bytes=103600
operation=append_only_into_active_or_new_segment_objects
```

## Timed runner output (raw)

```text
folder_id=p0-1-100k-segments-macos-to-windows
layout=segments
baseline_events=100000
incremental_events=100
payload_bytes=1024
segment_events=1000
source_platform=macos
destination_platform=windows
syncthing_version=v2.1.5
append_setup_ms=458.918
append_command_events=100100
expected_files_after_append=101
expected_bytes_after_append=103703600
incremental_logical_bytes=103600
started_at_utc=2026-10-01T03:50:29.770Z
scan_completed_at_utc=2026-10-01T03:50:30.045Z
completed_at_utc=2026-10-01T03:50:31.095Z
incremental_scan_ms=275.727
incremental_sync_total_ms=1317.548
source_local_files=101
source_local_bytes=103703600
source_global_files=101
source_global_bytes=103703600
peer_completion=100
peer_global_items=102
peer_need_items=0
peer_need_bytes=0
peer_need_deletes=0
source_global_files_verified=101
source_paused_after_completion=true
transport_scope=explicit_direct_tcp_only
scope=isolated_p0_1_incremental_syncthing_scan_sync_measurement
```

Shell wrapper:

```text
INCREMENTAL_START_UTC=2026-10-01T03:50:28Z
INCREMENTAL_END_UTC=2026-10-01T03:50:31Z
EXIT_CODE=0
```

## Windows destination physical post-check

Independent destination observation at `2026-10-01T03:52:05Z`:

```text
state=idle
local_files=101
local_bytes=103703600
global_files=101
global_bytes=103703600
need_files=0
need_bytes=0
need_deletes=0
errors=0
pull_errors=0
```

## Physical-object note (segments)

Post-append physical object count was **measured**, not assumed:

- before append: 100 sealed segment objects / 103600000 bytes / 100000 events
- after append: 101 physical objects / 103703600 bytes / 100100 events  
  (100 sealed + 1 new/active object containing the +100 records)

## Notes

- Append of 100 events occurred while source was paused; append time is setup (`append_setup_ms=458.918`) and is **outside** the timed incremental sync window. Do not combine `append_setup_ms` with `incremental_sync_total_ms`.
- Timed window is `started_at_utc` → `completed_at_utc` (`incremental_sync_total_ms=1317.548`), with scan portion `incremental_scan_ms=275.727`.
- This working folder must not be reused as a fresh initial-sync cell.
- Do not retire `p0-1-100k-segments-macos-to-windows` from this documentation update alone.
- Retained corpus under `benchmarks/p0-1/` was not modified by append.
- Initial and incremental remain separate evidence files; this file is incremental only.
- No production-layout conclusion may be drawn from this result.
