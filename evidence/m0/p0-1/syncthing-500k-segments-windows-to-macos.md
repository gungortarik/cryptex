# M0 P0-1 — Valid Initial Syncthing Measurement (500k segments, Windows→macOS)

> **Historical status note (2026-10-05):** Status statements below reflect the state when this dated plan/result was recorded, before M0 closeout. Current M0 status is COMPLETE for the synthetic foundation scope; scoped ADR-0012/0004 are ACCEPTED. This note does not expand this artifact's evidence, satisfy production residuals, or authorize M1. See `evidence/m0/m0-closure-audit-20261005.md`.

**Status:** VERIFIED TIMED MEASUREMENT  
**Scope:** P0-1 initial Syncthing scan/sync for sealed-segment packaging at 500k logical events, Windows source → macOS destination  
**Architecture effect:** none; does not select a production layout or accept ADR-0013

## Cell

```text
folder_id=p0-1-500k-segments-windows-to-macos
layout=segments
events=500000
expected_files=500
expected_bytes=518000000
source_platform=windows
destination_platform=macos
syncthing_version=v2.1.5
branch=m0/p0-1-cursor
```

## Timed runner output (raw)

```text
folder_id=p0-1-500k-segments-windows-to-macos
layout=segments
events=500000
source_platform=windows
destination_platform=macos
syncthing_version=v2.1.5
started_at_utc=2026-10-02T03:03:36.671Z
scan_completed_at_utc=2026-10-02T03:03:37.260Z
completed_at_utc=2026-10-02T03:03:52.237Z
initial_scan_ms=589.162
initial_sync_total_ms=15565.032
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
START_TIMESTAMP_UTC=2026-10-02T03:03:36Z
COMPLETION_TIMESTAMP_UTC=2026-10-02T03:03:52Z
EXIT_CODE=0
```

## macOS destination physical post-check

Independent destination observation (no separate observation timestamp was supplied with this recording):

```text
state=idle
localFiles=500
localBytes=518000000
globalFiles=500
globalBytes=518000000
needFiles=0
needBytes=0
needDeletes=0
errors=0
pullErrors=0
physical_files=500
physical_bytes=518000000
```

## Post-conditions verified

- success predicates satisfied (`source_local_files/bytes`, peer completion/needs, source re-paused)
- peer need items/bytes/deletes all zero at completion
- source global files verified at 500
- source paused after completion
- transport remained explicit direct TCP only
- macOS destination independently idle with matching local/global/physical counts and zero needs/errors

## Retention

Approved incremental measurement for this same cell is recorded separately in:

- `evidence/m0/p0-1/syncthing-500k-segments-windows-to-macos-incremental.md`

Do **not** retire or recreate this folder ID yet. Do not treat this file as incremental evidence.

## Boundary

This evidence verifies the **initial Syncthing transport cell only**. It does not prove Cryptex replication semantics, does not complete P0-4, and does not accept ADR-0013.

This is one valid initial-sync matrix cell only (500k segments, Windows→macOS). P0-1 remains IN PROGRESS until ADR-0013 is explicitly accepted. No production layout is inferred from this result.
