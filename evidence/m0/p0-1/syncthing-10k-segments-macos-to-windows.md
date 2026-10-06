# M0 P0-1 — Valid Initial Syncthing Measurement (10k segments, macOS→Windows)

> **Historical status note (2026-10-05):** This dated benchmark/protocol records the state before the P0-1 decision. ADR-0013 is now ACCEPTED with sealed per-device segment/pack files, and M0 is COMPLETE for the synthetic foundation scope. This note does not revise the measurements or make `segment_events=1000` a production constant. See `docs/adr/ADR-0013-physical-event-storage-layout.md` and `evidence/m0/m0-closure-audit-20261005.md`.

**Status:** VERIFIED TIMED MEASUREMENT  
**Scope:** P0-1 initial Syncthing scan/sync for sealed-segment packaging at 10k logical events, macOS source → Windows destination  
**Architecture effect:** none; does not select a production layout or accept ADR-0013

## Cell

```text
folder_id=p0-1-10k-segments-macos-to-windows
layout=segments
events=10000
expected_files=10
expected_bytes=10360000
source_platform=macos
destination_platform=windows
syncthing_version=v2.1.5
branch=m0/p0-1-cursor
```

## Retained corpus

```text
source_corpus=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/benchmarks/p0-1/macos-10000-20260929T054105Z/data/segments-10000
corpus_files=10
corpus_bytes=10360000
verified=true
```

Only one `segments-10000` directory exists under the P0-1 benchmark root. The retained corpus was not regenerated.

## Preconditions

- r3 files cell retired from macOS Syncthing config (`config_removed=true`, `work_path_deleted=false`)
- Windows destination already prepared as `receiveonly`, empty, unpaused, peer connected
- source prepared `sendonly`, `paused=true`, watcher disabled, `pre_config_files=10`, `pre_config_bytes=10360000`
- peer connected over direct LAN TCP; Syncthing v2.1.5

## Timed runner output (raw)

```text
home=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1
local_device_id=MHS264T-3YLCW4F-GL35TOB-DXZ6SUX-4MVY7YK-AZZFNZI-XFOXHN3-DYMMEQM
peer_id=G3IQV46-V6WYJ2U-WDLQQOC-FXMJBGA-QRIXPNV-GVARBCC-IHYTOBI-BOIG6QA
folder_id=p0-1-10k-segments-macos-to-windows
folder_path=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/p0-1-10k-segments-macos-to-windows/source
layout=segments
events=10000
source_platform=macos
destination_platform=windows
syncthing_version=v2.1.5
started_at_utc=2026-09-30T19:18:47.659Z
scan_completed_at_utc=2026-09-30T19:18:47.949Z
completed_at_utc=2026-09-30T19:18:49.508Z
initial_scan_ms=290.412
initial_sync_total_ms=1843.674
source_local_files=10
source_local_bytes=10360000
source_global_files=10
source_global_bytes=10360000
peer_completion=100
peer_global_items=11
peer_need_items=0
peer_need_bytes=0
peer_need_deletes=0
source_global_files_verified=10
source_paused_after_completion=true
transport_scope=explicit_direct_tcp_only
scope=isolated_p0_1_initial_syncthing_scan_sync_measurement
```

Shell wrapper:

```text
START_TIMESTAMP_UTC=2026-09-30T19:18:47Z
COMPLETION_TIMESTAMP_UTC=2026-09-30T19:18:49Z
EXIT_CODE=0
```

Runtime captures (outside repository):

```text
~/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/p0-1-10k-segments-macos-to-windows/timed-runner-capture.txt
~/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/p0-1-10k-segments-macos-to-windows/timed-result-2026-09-30T191849595Z.txt
```

## Boundary

One valid segments initial-sync matrix cell only. Layout selection and ADR-0013 remain open.
