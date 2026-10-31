# M0 P0-1 — Incremental Syncthing Measurement (100k files, macOS→Windows, r3)

**Status:** VERIFIED TIMED MEASUREMENT  
**Scope:** approved incremental workload on valid initial-sync cell `p0-1-100k-files-macos-to-windows-r3`  
**Architecture effect:** none; does not select a layout or accept ADR-0013

## Prerequisite

Valid initial sync for this folder ID is recorded in:

- `evidence/m0/p0-1/syncthing-100k-files-macos-to-windows-r3.md`

Workload protocol:

- `evidence/m0/p0-1/syncthing-incremental-workload-APPROVED-FOR-MEASUREMENT.md`

Logical workload:

```text
incremental_events=100
payload_bytes=1024
incremental_logical_bytes=103600
incremental_physical_files_added=100
operation=append_only_no_mutation_of_existing_objects
```

## Timed runner output (raw)

```text
folder_id=p0-1-100k-files-macos-to-windows-r3
layout=files
baseline_events=100000
incremental_events=100
payload_bytes=1024
source_platform=macos
destination_platform=windows
syncthing_version=v2.1.5
append_setup_ms=10012.087
append_command_events=100100
expected_files_after_append=100100
expected_bytes_after_append=103703600
incremental_logical_bytes=103600
started_at_utc=2026-10-01T03:06:32.863Z
scan_completed_at_utc=2026-10-01T03:06:37.959Z
completed_at_utc=2026-10-01T03:06:38.878Z
incremental_scan_ms=5096.537
incremental_sync_total_ms=6007.964
source_local_files=100100
source_local_bytes=103703600
source_global_files=100100
source_global_bytes=103703600
peer_completion=100
peer_global_items=100101
peer_need_items=0
peer_need_bytes=0
peer_need_deletes=0
source_global_files_verified=100100
source_paused_after_completion=true
transport_scope=explicit_direct_tcp_only
scope=isolated_p0_1_incremental_syncthing_scan_sync_measurement
```

Shell wrapper:

```text
INCREMENTAL_START_UTC=2026-10-01T03:06:11Z
INCREMENTAL_END_UTC=2026-10-01T03:06:38Z
EXIT_CODE=0
```

## Windows destination physical post-check

Independent destination observation at `2026-10-01T03:08:34Z`:

```text
state=idle
local_files=100100
local_bytes=103703600
global_files=100100
global_bytes=103703600
need_files=0
need_bytes=0
need_deletes=0
errors=0
pull_errors=0
```

## Notes

- Append of 100 events occurred while source was paused; append time is setup (`append_setup_ms=10012.087`) and is **outside** the timed incremental sync window. Do not combine `append_setup_ms` with `incremental_sync_total_ms`.
- Timed window is `started_at_utc` → `completed_at_utc` (`incremental_sync_total_ms=6007.964`), with scan portion `incremental_scan_ms=5096.537`.
- This working folder must not be reused as a fresh initial-sync cell.
- Do not retire `p0-1-100k-files-macos-to-windows-r3` from this documentation update alone.
- Retained corpus under `benchmarks/p0-1/` was not modified by append.
- Initial and incremental remain separate evidence files; this file is incremental only.
