# M0 P0-1 — Incremental Syncthing Measurement (500k segments, Windows→macOS)

> **Historical status note (2026-10-05):** Status statements below reflect the state when this dated plan/result was recorded, before M0 closeout. Current M0 status is COMPLETE for the synthetic foundation scope; scoped ADR-0012/0004 are ACCEPTED. This note does not expand this artifact's evidence, satisfy production residuals, or authorize M1. See `evidence/m0/m0-closure-audit-20261005.md`.

**Status:** VERIFIED TIMED MEASUREMENT  
**Scope:** approved incremental workload on valid initial-sync cell `p0-1-500k-segments-windows-to-macos`  
**Architecture effect:** none; does not select a layout or accept ADR-0013

## Prerequisite

Valid initial sync:

- `evidence/m0/p0-1/syncthing-500k-segments-windows-to-macos.md`

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
folder_id=p0-1-500k-segments-windows-to-macos
layout=segments
baseline_events=500000
incremental_events=100
payload_bytes=1024
segment_events=1000
source_platform=windows
destination_platform=macos
syncthing_version=v2.1.5
append_setup_ms=311.646
append_command_events=500100
expected_files_after_append=501
expected_bytes_after_append=518103600
incremental_logical_bytes=103600
started_at_utc=2026-10-02T03:07:39.037Z
scan_completed_at_utc=2026-10-02T03:07:39.334Z
completed_at_utc=2026-10-02T03:07:40.225Z
incremental_scan_ms=297.252
incremental_sync_total_ms=1187.504
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
INCREMENTAL_START_UTC=2026-10-02T03:07:38Z
INCREMENTAL_END_UTC=2026-10-02T03:07:40Z
EXIT_CODE=0
```

## macOS destination physical post-check

Independent destination observation (no separate observation timestamp was supplied with this recording):

```text
state=idle
localFiles=501
localBytes=518103600
globalFiles=501
globalBytes=518103600
needFiles=0
needBytes=0
needDeletes=0
errors=0
pullErrors=0
physical_files=501
physical_bytes=518103600
```

## Physical-object note (segments)

Post-append physical object count was **measured**, not assumed:

```text
before_append_events=500000
before_append_physical_objects=500
before_append_stored_bytes=518000000
after_append_events=500100
after_append_physical_objects=501
after_append_stored_bytes=518103600
```

## Notes

- Append of 100 events occurred while source was paused; append time is setup (`append_setup_ms=311.646`) and is **outside** the timed incremental sync window. Do not combine `append_setup_ms` with `incremental_sync_total_ms`.
- Timed window is `started_at_utc` → `completed_at_utc` (`incremental_sync_total_ms=1187.504`), with scan portion `incremental_scan_ms=297.252`.
- This evidence verifies this **incremental Syncthing transport cell only**. It does not prove Cryptex replication semantics, does not complete P0-4, and does not select files vs segments.
- This working folder must not be reused as a fresh initial-sync cell.
- Do not retire `p0-1-500k-segments-windows-to-macos` from this documentation update alone.
- Retained corpus under `benchmarks/p0-1/` was not modified by append.
- Initial and incremental remain separate evidence files; this file is incremental only.
- No production-layout conclusion may be drawn from this result. ADR-0013 remains PROPOSED / PENDING. P0-1 remains IN PROGRESS until ADR-0013 is explicitly accepted.
