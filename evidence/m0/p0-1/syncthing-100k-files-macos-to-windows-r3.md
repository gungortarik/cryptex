# M0 P0-1 — Valid Initial Syncthing Measurement (100k files, macOS→Windows, r3)

**Status:** VERIFIED TIMED MEASUREMENT  
**Scope:** P0-1 initial Syncthing scan/sync for one-file-per-event at 100k, macOS source → Windows destination  
**Architecture effect:** none; does not select a production layout or accept ADR-0013

## Cell

```text
folder_id=p0-1-100k-files-macos-to-windows-r3
layout=files
events=100000
expected_files=100000
expected_bytes=103600000
source_platform=macos
destination_platform=windows
syncthing_version=v2.1.5
branch=m0/p0-1-cursor
```

Prior invalid cells for this logical matrix point were not reused:

- `p0-1-100k-files-macos-to-windows` — timed out at default 7200s (~75% peer completion)
- `p0-1-100k-files-macos-to-windows-r2` — timed runner disappeared before completion while Syncthing continued transferring (INVALID; no confirmed root cause claimed)

## Timed runner output (raw)

```text
home=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1
local_device_id=MHS264T-3YLCW4F-GL35TOB-DXZ6SUX-4MVY7YK-AZZFNZI-XFOXHN3-DYMMEQM
peer_id=G3IQV46-V6WYJ2U-WDLQQOC-FXMJBGA-QRIXPNV-GVARBCC-IHYTOBI-BOIG6QA
folder_id=p0-1-100k-files-macos-to-windows-r3
folder_path=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/p0-1-100k-files-macos-to-windows-r3/source
layout=files
events=100000
source_platform=macos
destination_platform=windows
syncthing_version=v2.1.5
started_at_utc=2026-10-01T02:11:43.263Z
scan_completed_at_utc=2026-10-01T02:12:15.634Z
completed_at_utc=2026-10-01T02:53:21.928Z
initial_scan_ms=32371.464
initial_sync_total_ms=2498632.230
source_local_files=100000
source_local_bytes=103600000
source_global_files=100000
source_global_bytes=103600000
peer_completion=100
peer_global_items=100001
peer_need_items=0
peer_need_bytes=0
peer_need_deletes=0
source_global_files_verified=100000
source_paused_after_completion=true
transport_scope=explicit_direct_tcp_only
scope=isolated_p0_1_initial_syncthing_scan_sync_measurement
```

Shell wrapper timestamps:

```text
START_TIMESTAMP_UTC=2026-10-01T02:11:43Z
COMPLETION_TIMESTAMP_UTC=2026-10-01T02:53:21Z
EXIT_CODE=0
```

Runtime raw captures (outside repository):

```text
~/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/p0-1-100k-files-macos-to-windows-r3/timed-runner-capture.txt
~/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/p0-1-100k-files-macos-to-windows-r3/timed-result-2026-10-01T025321992Z.txt
```

## Post-conditions verified

- success predicates satisfied (`source_local_files/bytes`, peer completion/needs, source re-paused)
- peer need items/bytes/deletes all zero at completion
- source global files verified at 100000
- source paused after completion
- transport remained explicit direct TCP only
- working tree remains under guarded `syncthing-p0-1-runs` root

## Retention for incremental measurement

This r3 cell is the valid initial-sync base for the approved incremental measurement on the same logical matrix point. Do **not** retire or recreate this folder ID until that incremental run is recorded. Do not treat this file as incremental evidence.

## Boundary

This is one valid initial-sync matrix cell only (100k files, macOS→Windows). Remaining 100k reverse/segments cells, 500k matrix cells, layout selection, and ADR-0013 remain open. P0-1 remains IN PROGRESS. No production layout is inferred from this result.
