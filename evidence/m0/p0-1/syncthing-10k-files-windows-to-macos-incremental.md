# M0 P0-1 — Incremental Syncthing Measurement (10k files, Windows→macOS)

**Status:** VERIFIED TIMED MEASUREMENT  
**Scope:** approved incremental workload on valid initial-sync cell `p0-1-10k-files-windows-to-macos`  
**Architecture effect:** none; does not select a layout or accept ADR-0013

## Prerequisite

Valid initial sync for this folder ID is recorded in:

- `evidence/m0/p0-1/syncthing-10k-files-windows-to-macos.md`

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
home=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1
local_device_id=G3IQV46-V6WYJ2U-WDLQQOC-FXMJBGA-QRIXPNV-GVARBCC-IHYTOBI-BOIG6QA
peer_id=MHS264T-3YLCW4F-GL35TOB-DXZ6SUX-4MVY7YK-AZZFNZI-XFOXHN3-DYMMEQM
folder_id=p0-1-10k-files-windows-to-macos
folder_path=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1-runs\p0-1-10k-files-windows-to-macos\source
layout=files
baseline_events=10000
incremental_events=100
payload_bytes=1024
source_platform=windows
destination_platform=macos
syncthing_version=v2.1.5
append_setup_ms=1891.786
append_command_events=10100
expected_files_after_append=10100
expected_bytes_after_append=10463600
incremental_logical_bytes=103600
started_at_utc=2026-09-30T19:37:58.498Z
scan_completed_at_utc=2026-09-30T19:38:01.311Z
completed_at_utc=2026-09-30T19:38:01.862Z
incremental_scan_ms=2813.945
incremental_sync_total_ms=3362.245
source_local_files=10100
source_local_bytes=10463600
source_global_files=10100
source_global_bytes=10463600
peer_completion=100
peer_global_items=10101
peer_need_items=0
peer_need_bytes=0
peer_need_deletes=0
source_global_files_verified=10100
source_paused_after_completion=true
transport_scope=explicit_direct_tcp_only
scope=isolated_p0_1_incremental_syncthing_scan_sync_measurement
```

Shell wrapper:

```text
INCREMENTAL_START_UTC=2026-09-30T19:37:54Z
INCREMENTAL_END_UTC=2026-09-30T19:38:02Z
EXIT_CODE=0
```

Runtime capture:

```text
%LOCALAPPDATA%\Cryptex-Dev\syncthing-p0-1-runs\p0-1-10k-files-windows-to-macos\incremental-runner-capture.txt
```

## Commands / tooling

```text
run-syncthing-p0-1-incremental-sync.mjs
bench-bin=d:\github_repos\cryptex\target\release\m0-storage-bench.exe
```

## Notes

- Append of 100 events occurred while source was paused; append time is setup (`append_setup_ms`), outside timed sync.
- Post-incremental physical counts: 10100 files / 10463600 bytes.
- This working folder must not be reused as a fresh initial-sync cell.
- Retained corpus under `benchmarks/p0-1/windows-10000-20260929T055348Z/data/files-10000` remained `10000` files / `10360000` bytes after both runs.
- For later comparison: Windows→macOS incremental_sync_total_ms ≈ 3.36s vs r3 macOS→Windows ≈ 2.34s at the same +100/+103600 workload.
