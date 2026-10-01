# M0 P0-1 — Valid Initial Syncthing Measurement (100k files, Windows→macOS)

**Status:** VERIFIED TIMED MEASUREMENT  
**Scope:** P0-1 initial Syncthing scan/sync for one-file-per-event at 100k, Windows source → macOS destination  
**Architecture effect:** none; does not select a production layout or accept ADR-0013

## Cell

```text
folder_id=p0-1-100k-files-windows-to-macos
layout=files
events=100000
expected_files=100000
expected_bytes=103600000
source_platform=windows
destination_platform=macos
syncthing_version=v2.1.5
branch=m0/p0-1-cursor
```

## Timed runner output (raw)

```text
folder_id=p0-1-100k-files-windows-to-macos
layout=files
events=100000
source_platform=windows
destination_platform=macos
syncthing_version=v2.1.5
started_at_utc=2026-10-01T04:17:50.927Z
scan_completed_at_utc=2026-10-01T04:19:24.124Z
completed_at_utc=2026-10-01T05:41:22.680Z
initial_scan_ms=93197.326
initial_sync_total_ms=5011750.868
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
START_TIMESTAMP_UTC=2026-10-01T04:17:50Z
COMPLETION_TIMESTAMP_UTC=2026-10-01T05:41:22Z
EXIT_CODE=0
```

## Paths

```text
windows_source=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1-runs\p0-1-100k-files-windows-to-macos\source
macos_destination=~/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/p0-1-100k-files-windows-to-macos/destination
```

## Runtime raw captures (outside repository)

```text
C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1-runs\p0-1-100k-files-windows-to-macos\timed-runner-stdout.txt
C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1-runs\p0-1-100k-files-windows-to-macos\timed-runner-stdout-utf8.txt
C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1-runs\p0-1-100k-files-windows-to-macos\timed-result-2026-10-01T101735124Z.txt
```

### Raw-capture provenance

- Original PowerShell redirected stdout `timed-runner-stdout.txt` is UTF-16LE (BOM `FF FE`) and was **not** modified.
- The initial `capture-syncthing-p0-1-raw-result.mjs` invocation failed because that helper reads `--raw-file` as UTF-8.
- A separate BOM-less UTF-8 normalized copy was created as `timed-runner-stdout-utf8.txt` (original left intact).
- Both required markers were verified `True` before the successful capture into `timed-result-2026-10-01T101735124Z.txt`.
- This encoding/capture issue is **not** a benchmark failure and does **not** invalidate the timed measurement.
- Verified scope of the capture problem: UTF-16LE stdout vs UTF-8 raw-file reader. No broader root cause is claimed.
- Tooling fix is **not** applied in this evidence update; recorded as an OPEN P0-1 tooling follow-up at the time of measurement.
- Later follow-up: `capture-syncthing-p0-1-raw-result.mjs` was updated to accept BOM-marked UTF-16LE (`FF FE`) in addition to UTF-8. The historical first-capture failure above remains accurate for the measurement window and is not rewritten.

## macOS destination physical post-check

Independent destination observation at `2026-10-01T10:11:54Z`:

```text
state=idle
localFiles=100000
localBytes=103600000
globalFiles=100000
globalBytes=103600000
needFiles=0
needBytes=0
needDeletes=0
errors=0
pullErrors=0
physical_files=100000
physical_bytes=103600000
```

## Post-conditions verified

- success predicates satisfied (`source_local_files/bytes`, peer completion/needs, source re-paused)
- peer need items/bytes/deletes all zero at completion
- source global files verified at 100000
- source paused after completion
- transport remained explicit direct TCP only
- macOS destination independently idle with matching local/global/physical counts and zero needs/errors

## Retention for incremental measurement

This cell is the valid initial-sync base for the approved incremental measurement on the same logical matrix point. Do **not** retire, recreate, or reuse this folder ID as a new initial cell until that incremental run is recorded. Do not start incremental from this documentation update alone. Do not treat this file as incremental evidence.

## Boundary

This is one valid initial-sync matrix cell only (100k files, Windows→macOS). Remaining 100k reverse segments cell, 500k matrix cells, layout selection, and ADR-0013 remain open. P0-1 remains IN PROGRESS. No production layout is inferred from this result.
