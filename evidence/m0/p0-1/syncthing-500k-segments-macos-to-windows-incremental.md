# M0 P0-1 — Incremental Syncthing Measurement (500k segments, macOS→Windows)

> **Historical status note (2026-10-05):** This dated benchmark/protocol records the state before the P0-1 decision. ADR-0013 is now ACCEPTED with sealed per-device segment/pack files, and M0 is COMPLETE for the synthetic foundation scope. This note does not revise the measurements or make `segment_events=1000` a production constant. See `docs/adr/ADR-0013-physical-event-storage-layout.md` and `evidence/m0/m0-closure-audit-20261005.md`.

**Status:** VERIFIED TIMED MEASUREMENT  
**Scope:** approved incremental workload on valid initial-sync cell `p0-1-500k-segments-macos-to-windows`  
**Architecture effect:** none; does not select a layout or accept ADR-0013

## Prerequisite

Valid initial sync:

- `evidence/m0/p0-1/syncthing-500k-segments-macos-to-windows.md`

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
folder_id=p0-1-500k-segments-macos-to-windows
layout=segments
baseline_events=500000
incremental_events=100
payload_bytes=1024
segment_events=1000
source_platform=macos
destination_platform=windows
syncthing_version=v2.1.5
append_setup_ms=544.390
append_command_events=500100
expected_files_after_append=501
expected_bytes_after_append=518103600
incremental_logical_bytes=103600
started_at_utc=2026-10-02T02:34:07.810Z
scan_completed_at_utc=2026-10-02T02:34:08.080Z
completed_at_utc=2026-10-02T02:34:09.147Z
incremental_scan_ms=270.648
incremental_sync_total_ms=1336.169
source_local_files=501
source_local_bytes=518103600
source_global_files=501
source_global_bytes=518103600
peer_completion=100
peer_global_items=502
peer_need_items=0
peer_need_bytes=0
peer_need_deletes=0
source_global_files_verified=501
source_paused_after_completion=true
transport_scope=explicit_direct_tcp_only
```

Shell wrapper:

```text
INCREMENTAL_START_UTC=2026-10-02T02:34:06Z
INCREMENTAL_END_UTC=2026-10-02T02:34:09Z
EXIT_CODE=0
```

## Windows destination physical post-check

Independent destination observation (no separate observation timestamp was supplied with this recording):

```text
state=idle
local_files=501
local_bytes=518103600
global_files=501
global_bytes=518103600
need_files=0
need_bytes=0
need_deletes=0
errors=0
pull_errors=0
physical_files=501
physical_bytes=518103600
```

## Physical-object note (segments)

Post-append physical object count was **measured**, not assumed:

- before append: 500 sealed segment objects / 518000000 bytes / 500000 events
- after append: 501 physical objects / 518103600 bytes / 500100 events  
  (500 sealed + 1 new/active object containing the +100 records)

## Notes

- Append of 100 events occurred while source was paused; append time is setup (`append_setup_ms=544.390`) and is **outside** the timed incremental sync window. Do not combine `append_setup_ms` with `incremental_sync_total_ms`.
- Timed window is `started_at_utc` → `completed_at_utc` (`incremental_sync_total_ms=1336.169`), with scan portion `incremental_scan_ms=270.648`.
- This working folder must not be reused as a fresh initial-sync cell.
- Do not retire `p0-1-500k-segments-macos-to-windows` from this documentation update alone.
- Retained corpus under `benchmarks/p0-1/` was not modified by append.
- Initial and incremental remain separate evidence files; this file is incremental only.
- No production-layout conclusion may be drawn from this result.
- ADR-0013 remains PROPOSED / PENDING.
