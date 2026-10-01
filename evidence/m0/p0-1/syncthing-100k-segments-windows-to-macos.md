# M0 P0-1 — Valid Initial Syncthing Measurement (100k segments, Windows→macOS)

**Status:** VERIFIED TIMED MEASUREMENT  
**Scope:** P0-1 initial Syncthing scan/sync for sealed-segment packaging at 100k logical events, Windows source → macOS destination  
**Architecture effect:** none; does not select a production layout or accept ADR-0013

## Cell

```text
folder_id=p0-1-100k-segments-windows-to-macos
layout=segments
events=100000
expected_files=100
expected_bytes=103600000
source_platform=windows
destination_platform=macos
syncthing_version=v2.1.5
branch=m0/p0-1-cursor
```

## Timed runner output (raw)

```text
folder_id=p0-1-100k-segments-windows-to-macos
layout=segments
events=100000
source_platform=windows
destination_platform=macos
syncthing_version=v2.1.5
started_at_utc=2026-10-01T10:45:23.086Z
scan_completed_at_utc=2026-10-01T10:45:23.395Z
completed_at_utc=2026-10-01T10:45:26.874Z
initial_scan_ms=309.655
initial_sync_total_ms=3788.200
source_local_files=100
source_local_bytes=103600000
source_global_files=100
source_global_bytes=103600000
peer_completion=100
peer_global_items=101
peer_need_items=0
peer_need_bytes=0
peer_need_deletes=0
source_global_files_verified=100
source_paused_after_completion=true
transport_scope=explicit_direct_tcp_only
scope=isolated_p0_1_initial_syncthing_scan_sync_measurement
```

Shell wrapper timestamps:

```text
START_TIMESTAMP_UTC=2026-10-01T10:45:22Z
COMPLETION_TIMESTAMP_UTC=2026-10-01T10:45:26Z
EXIT_CODE=0
```

Runtime raw capture (outside repository):

```text
C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1-runs\p0-1-100k-segments-windows-to-macos\timed-result-2026-10-01T104527118Z.txt
```

## Paths

```text
windows_source=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1-runs\p0-1-100k-segments-windows-to-macos\source
macos_destination=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/p0-1-100k-segments-windows-to-macos/destination
```

## macOS destination physical post-check

Independent destination observation at `2026-10-01T14:01:51Z`:

```text
state=idle
localFiles=100
localBytes=103600000
globalFiles=100
globalBytes=103600000
needFiles=0
needBytes=0
needDeletes=0
errors=0
pullErrors=0
physical_files=100
physical_bytes=103600000
```

The later physical post-check timestamp is **not** part of `initial_sync_total_ms`.

## Post-conditions verified

- success predicates satisfied (`source_local_files/bytes`, peer completion/needs, source re-paused)
- peer need items/bytes/deletes all zero at completion
- source global files verified at 100
- source paused after completion
- transport remained explicit direct TCP only
- macOS destination independently idle with matching local/global/physical counts and zero needs/errors

## Retention for incremental measurement

This cell is the valid initial-sync base for the approved incremental measurement on the same logical matrix point. Do **not** retire, recreate, or reuse this folder ID as a new initial cell until that incremental run is recorded. Do not start incremental from this documentation update alone. Do not treat this file as incremental evidence. Incremental for this cell = NOT YET EXECUTED.

## Boundary

This evidence verifies the **initial Syncthing transport cell only**. It does not prove Cryptex replication semantics, does not complete P0-4, and does not justify choosing files vs segments.

This is one valid initial-sync matrix cell only (100k segments, Windows→macOS). Remaining 500k matrix cells, layout selection, and ADR-0013 remain open. P0-1 remains IN PROGRESS. No production layout is inferred from this result.
