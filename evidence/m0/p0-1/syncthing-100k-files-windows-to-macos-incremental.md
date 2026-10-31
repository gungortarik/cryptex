# M0 P0-1 — Incremental Syncthing Measurement (100k files, Windows→macOS)

**Status:** VERIFIED TIMED MEASUREMENT  
**Scope:** approved incremental workload on valid initial-sync cell `p0-1-100k-files-windows-to-macos`  
**Architecture effect:** none; does not select a layout or accept ADR-0013

## Prerequisite

Valid initial sync for this folder ID is recorded in:

- `evidence/m0/p0-1/syncthing-100k-files-windows-to-macos.md`

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
folder_id=p0-1-100k-files-windows-to-macos
layout=files
baseline_events=100000
incremental_events=100
payload_bytes=1024
source_platform=windows
destination_platform=macos
syncthing_version=v2.1.5
append_setup_ms=10764.116
append_command_events=100100
expected_files_after_append=100100
expected_bytes_after_append=103703600
incremental_logical_bytes=103600
started_at_utc=2026-10-01T10:31:02.485Z
scan_completed_at_utc=2026-10-01T10:31:23.538Z
completed_at_utc=2026-10-01T10:31:23.837Z
incremental_scan_ms=21053.008
incremental_sync_total_ms=21351.494
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
INCREMENTAL_START_UTC=2026-10-01T10:30:41Z
INCREMENTAL_END_UTC=2026-10-01T10:31:23Z
EXIT_CODE=0
```

## Paths

```text
windows_source=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1-runs\p0-1-100k-files-windows-to-macos\source
macos_destination=~/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/p0-1-100k-files-windows-to-macos/destination
```

## macOS destination physical post-check

Independent destination observation at `2026-10-01T10:32:33Z`:

```text
state=idle
localFiles=100100
localBytes=103703600
globalFiles=100100
globalBytes=103703600
needFiles=0
needBytes=0
needDeletes=0
errors=0
pullErrors=0
physical_files=100100
physical_bytes=103703600
```

## Physical-object note (files)

Post-append physical object count was **physically verified**, not assumed:

- before append: 100000 files / 103600000 bytes / 100000 events
- after append: 100100 files / 103703600 bytes / 100100 events  
  (+100 events = +103600 logical bytes)

## Notes

- Append of 100 events occurred while source was paused; append time is setup (`append_setup_ms=10764.116`) and is **outside** the timed incremental sync window. Do not combine `append_setup_ms` with `incremental_sync_total_ms`.
- Timed window is `started_at_utc` → `completed_at_utc` (`incremental_sync_total_ms=21351.494`), with scan portion `incremental_scan_ms=21053.008`.
- This working folder must not be reused as a fresh initial-sync cell.
- Do not retire `p0-1-100k-files-windows-to-macos` from this documentation update alone.
- Retained corpus under `benchmarks/p0-1/` was not modified by append.
- Initial and incremental remain separate evidence files; this file is incremental only.
- No production-layout conclusion may be drawn from this result.
