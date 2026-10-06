# M0 P0-1 — Valid Initial Syncthing Measurement (10k files, macOS→Windows, r3)

**Status:** VERIFIED TIMED MEASUREMENT  
**Scope:** P0-1 initial Syncthing scan/sync for one-file-per-event at 10k, macOS source → Windows destination  
**Architecture effect:** none; does not select a production layout or accept ADR-0013

## Cell

```text
folder_id=p0-1-10k-files-macos-to-windows-r3
layout=files
events=10000
expected_files=10000
expected_bytes=10360000
source_platform=macos
destination_platform=windows
syncthing_version=v2.1.5
branch=m0/p0-1-cursor
```

Prior invalid cells for this logical matrix point (`p0-1-10k-files-macos-to-windows`, `...-r2`) were not reused.

## Timed runner output (raw)

```text
home=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1
local_device_id=MHS264T-3YLCW4F-GL35TOB-DXZ6SUX-4MVY7YK-AZZFNZI-XFOXHN3-DYMMEQM
peer_id=G3IQV46-V6WYJ2U-WDLQQOC-FXMJBGA-QRIXPNV-GVARBCC-IHYTOBI-BOIG6QA
folder_id=p0-1-10k-files-macos-to-windows-r3
folder_path=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/p0-1-10k-files-macos-to-windows-r3/source
layout=files
events=10000
source_platform=macos
destination_platform=windows
syncthing_version=v2.1.5
started_at_utc=2026-09-30T19:00:29.983Z
scan_completed_at_utc=2026-09-30T19:00:32.307Z
completed_at_utc=2026-09-30T19:01:26.705Z
initial_scan_ms=2324.080
initial_sync_total_ms=56716.159
source_local_files=10000
source_local_bytes=10360000
source_global_files=10000
source_global_bytes=10360000
peer_completion=100
peer_global_items=10001
peer_need_items=0
peer_need_bytes=0
peer_need_deletes=0
source_global_files_verified=10000
source_paused_after_completion=true
transport_scope=explicit_direct_tcp_only
scope=isolated_p0_1_initial_syncthing_scan_sync_measurement
```

Shell wrapper timestamps:

```text
START_TIMESTAMP_UTC=2026-09-30T19:00:29Z
COMPLETION_TIMESTAMP_UTC=2026-09-30T19:01:26Z
EXIT_CODE=0
```

Runtime raw captures (outside repository):

```text
~/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/p0-1-10k-files-macos-to-windows-r3/timed-runner-capture.txt
~/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/p0-1-10k-files-macos-to-windows-r3/timed-result-2026-09-30T190126790Z.txt
```

## Post-conditions verified

- success predicates satisfied (`source_local_files/bytes`, peer completion/needs, source re-paused)
- retained corpus still `10000` files / `10360000` bytes
- source working tree remains under guarded `syncthing-p0-1-runs` root

## Boundary

This is one valid initial-sync matrix cell only. Remaining scales, layouts, reverse direction, and incremental sync remain open. ADR-0013 remains unresolved.
