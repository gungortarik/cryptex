# M0 P0-1 — DRAFT Evidence Template (500k segments, macOS→Windows, initial)

**Status:** DRAFT / PENDING MEASUREMENT — not a verified result  
**Scope:** P0-1 initial Syncthing scan/sync for sealed-segment packaging at 500k logical events, macOS source → Windows destination  
**Architecture effect:** none; does not select a production layout or accept ADR-0013

Replace all `PENDING` fields with measured values after a successful timed run. Do not commit filled metrics until physically verified.

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

## Timed runner output (raw) — PENDING

```text
folder_id=p0-1-500k-segments-macos-to-windows
layout=segments
events=500000
source_platform=macos
destination_platform=windows
syncthing_version=v2.1.5
started_at_utc=PENDING
scan_completed_at_utc=PENDING
completed_at_utc=PENDING
initial_scan_ms=PENDING
initial_sync_total_ms=PENDING
source_local_files=500
source_local_bytes=518000000
source_global_files=500
source_global_bytes=518000000
peer_completion=100
peer_global_items=PENDING
peer_need_items=0
peer_need_bytes=0
peer_need_deletes=0
source_global_files_verified=500
source_paused_after_completion=true
transport_scope=explicit_direct_tcp_only
scope=isolated_p0_1_initial_syncthing_scan_sync_measurement
```

## Expected predicates

- `EXIT_CODE=0`
- source local/global files = `500`, bytes = `518000000`
- peer needs all zero; `peer_completion=100`
- source paused after completion
- destination independent post-check idle with matching counts
- no runtime paths inside the Git repository

## Windows destination physical post-check — PENDING

```text
timestamp=PENDING
state=idle
local_files=500
local_bytes=518000000
global_files=500
global_bytes=518000000
need_files=0
need_bytes=0
need_deletes=0
errors=0
pull_errors=0
```

## Retention

Retain this folder ID for the approved +100 incremental on the same cell. Do not retire yet. Do not treat this DRAFT file as verified evidence.
