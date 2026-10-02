# M0 P0-1 — Valid Initial Syncthing Measurement (500k segments, macOS→Windows)

**Status:** VERIFIED TIMED MEASUREMENT  
**Scope:** P0-1 initial Syncthing scan/sync for sealed-segment packaging at 500k logical events, macOS source → Windows destination  
**Architecture effect:** none; does not select a production layout or accept ADR-0013

## Cell

```text
folder_id=p0-1-500k-segments-macos-to-windows
layout=segments
events=500000
expected_files=500
expected_bytes=518000000
source_platform=macos
destination_platform=windows
syncthing_version=v2.1.5
branch=m0/p0-1-cursor
```

## Timed runner output (raw)

```text
folder_id=p0-1-500k-segments-macos-to-windows
layout=segments
events=500000
source_platform=macos
destination_platform=windows
syncthing_version=v2.1.5
started_at_utc=2026-10-02T02:27:56.968Z
scan_completed_at_utc=2026-10-02T02:27:57.244Z
completed_at_utc=2026-10-02T02:28:15.356Z
initial_scan_ms=276.274
initial_sync_total_ms=18382.591
source_local_files=500
source_local_bytes=518000000
source_global_files=500
source_global_bytes=518000000
peer_completion=100
peer_global_items=501
peer_need_items=0
peer_need_bytes=0
peer_need_deletes=0
source_global_files_verified=500
source_paused_after_completion=true
transport_scope=explicit_direct_tcp_only
```

Shell wrapper timestamps:

```text
START_TIMESTAMP_UTC=2026-10-02T02:27:56Z
COMPLETION_TIMESTAMP_UTC=2026-10-02T02:28:15Z
EXIT_CODE=0
```

## Windows destination physical post-check

Independent destination observation (no separate observation timestamp was supplied with this recording):

```text
state=idle
local_files=500
local_bytes=518000000
global_files=500
global_bytes=518000000
need_files=0
need_bytes=0
errors=0
pull_errors=0
physical_files=500
physical_bytes=518000000
```

## Post-conditions verified

- success predicates satisfied (`source_local_files/bytes`, peer completion/needs, source re-paused)
- peer need items/bytes/deletes all zero at completion
- source global files verified at 500
- source paused after completion
- transport remained explicit direct TCP only
- Windows destination independently idle with matching local/global/physical counts and zero needs/errors

## Retention

Approved incremental measurement for this same cell is recorded separately in:

- `evidence/m0/p0-1/syncthing-500k-segments-macos-to-windows-incremental.md`

Do **not** retire or recreate this folder ID yet. Do not treat this file as incremental evidence.

## Boundary

This is one valid initial-sync matrix cell only (500k segments, macOS→Windows). Remaining 500k reverse-direction segments cells, layout selection, and ADR-0013 remain open. P0-1 remains IN PROGRESS. No production layout is inferred from this result.
