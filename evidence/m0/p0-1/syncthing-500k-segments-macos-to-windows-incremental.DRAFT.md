# M0 P0-1 — DRAFT Evidence Template (500k segments, macOS→Windows, incremental)

**Status:** DRAFT / PENDING MEASUREMENT — not a verified result  
**Scope:** approved incremental workload on cell `p0-1-500k-segments-macos-to-windows`  
**Architecture effect:** none; does not select a layout or accept ADR-0013

Requires a prior VERIFIED initial evidence file for the same folder ID.

## Logical workload

```text
incremental_events=100
payload_bytes=1024
segment_events=1000
incremental_logical_bytes=103600
operation=append_only_into_active_or_new_segment_objects
```

## Timed runner output (raw) — PENDING

```text
folder_id=p0-1-500k-segments-macos-to-windows
layout=segments
baseline_events=500000
incremental_events=100
payload_bytes=1024
segment_events=1000
source_platform=macos
destination_platform=windows
syncthing_version=v2.1.5
append_setup_ms=PENDING
append_command_events=500100
expected_files_after_append=PENDING_MEASURED
expected_bytes_after_append=518103600
incremental_logical_bytes=103600
started_at_utc=PENDING
scan_completed_at_utc=PENDING
completed_at_utc=PENDING
incremental_scan_ms=PENDING
incremental_sync_total_ms=PENDING
source_local_files=PENDING_MEASURED
source_local_bytes=518103600
source_global_files=PENDING_MEASURED
source_global_bytes=518103600
peer_completion=100
peer_need_items=0
peer_need_bytes=0
peer_need_deletes=0
source_paused_after_completion=true
transport_scope=explicit_direct_tcp_only
scope=isolated_p0_1_incremental_syncthing_scan_sync_measurement
```

## Expected predicates

- `EXIT_CODE=0`
- `append_setup_ms` recorded but **outside** timed window; do not combine with `incremental_sync_total_ms`
- post-append physical object count **measured**, not assumed
- expected bytes after append = `518103600`
- peer needs all zero; source re-paused
- Windows destination post-check matches measured post-append counts

## Physical-object note (segments) — PENDING

```text
before_append_events=500000
before_append_physical_objects=500
before_append_stored_bytes=518000000
after_append_events=500100
after_append_physical_objects=PENDING_MEASURED
after_append_stored_bytes=518103600
```

## Windows destination physical post-check — PENDING

```text
timestamp=PENDING
state=idle
local_files=PENDING_MEASURED
local_bytes=518103600
global_files=PENDING_MEASURED
global_bytes=518103600
need_files=0
need_bytes=0
need_deletes=0
errors=0
pull_errors=0
```
