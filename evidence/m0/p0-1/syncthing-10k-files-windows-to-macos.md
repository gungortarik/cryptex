# M0 P0-1 — Valid Initial Syncthing Measurement (10k files, Windows→macOS)

**Status:** VERIFIED TIMED MEASUREMENT  
**Scope:** P0-1 initial Syncthing scan/sync for one-file-per-event at 10k, Windows source → macOS destination  
**Architecture effect:** none; does not select a production layout or accept ADR-0013

## Cell

```text
folder_id=p0-1-10k-files-windows-to-macos
layout=files
events=10000
expected_files=10000
expected_bytes=10360000
source_platform=windows
destination_platform=macos
syncthing_version=v2.1.5
branch=m0/p0-1-cursor
direction=windows_to_macos
connection_mode=explicit_direct_tcp_only
```

## Retained corpus

```text
source_corpus=C:\Users\Tarik\AppData\Local\Cryptex-Dev\benchmarks\p0-1\windows-10000-20260929T055348Z\data\files-10000
corpus_identity=windows-10000-20260929T055348Z / files-10000
corpus_files=10000
corpus_bytes=10360000
verified=true
```

Only one `files-10000` directory exists under the Windows P0-1 benchmark root for this run id. The retained corpus was not regenerated. Independent recount and `verify-retained-corpus.mjs` both matched before source preparation and again after both timed runs.

## Preconditions

- prior Windows destination cell `p0-1-10k-segments-macos-to-windows` retired from Windows Syncthing config only (`config_removed=true`, `work_path_deleted=false`, forensic path retained)
- macOS destination already prepared as `receiveonly`, empty, unpaused, watcher disabled, peer connected
- Windows source prepared `sendonly`, `paused=true`, watcher disabled, `pre_config_files=10000`, `pre_config_bytes=10360000`
- peer connected over direct LAN TCP (`tcp-client` to `192.168.50.104:22001`); Syncthing v2.1.5 both sides
- isolated transport: GUI `127.0.0.1:18384`, listener `tcp://0.0.0.0:22001`, global/local discovery off, relays off, NAT off, auto-upgrade off

## Timed runner output (raw)

```text
home=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1
local_device_id=G3IQV46-V6WYJ2U-WDLQQOC-FXMJBGA-QRIXPNV-GVARBCC-IHYTOBI-BOIG6QA
peer_id=MHS264T-3YLCW4F-GL35TOB-DXZ6SUX-4MVY7YK-AZZFNZI-XFOXHN3-DYMMEQM
folder_id=p0-1-10k-files-windows-to-macos
folder_path=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1-runs\p0-1-10k-files-windows-to-macos\source
layout=files
events=10000
source_platform=windows
destination_platform=macos
syncthing_version=v2.1.5
started_at_utc=2026-09-30T19:35:43.106Z
scan_completed_at_utc=2026-09-30T19:35:48.448Z
completed_at_utc=2026-09-30T19:36:43.739Z
initial_scan_ms=5342.066
initial_sync_total_ms=60630.917
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

Shell wrapper:

```text
START_TIMESTAMP_UTC=2026-09-30T19:35:42Z
COMPLETION_TIMESTAMP_UTC=2026-09-30T19:36:44Z
EXIT_CODE=0
```

Runtime captures (outside repository):

```text
%LOCALAPPDATA%\Cryptex-Dev\syncthing-p0-1-runs\p0-1-10k-files-windows-to-macos\timed-runner-capture.txt
%LOCALAPPDATA%\Cryptex-Dev\syncthing-p0-1-runs\p0-1-10k-files-windows-to-macos\timed-result-2026-09-30T193644092Z.txt
```

## Commands / tooling

```text
verify-retained-corpus.mjs
retire-syncthing-p0-1-folder.mjs (segments destination; no --delete-work-path)
prepare-syncthing-p0-1-initial-sync.mjs --role source
run-syncthing-p0-1-initial-sync.mjs
capture-syncthing-p0-1-raw-result.mjs
```

## Observations (direction/layout comparison later)

- Reverse of the valid r3 macOS→Windows files cell at the same logical scale.
- Windows→macOS initial_sync_total_ms ≈ 60.6s vs r3 macOS→Windows ≈ 56.7s at identical 10000/10360000.
- initial_scan_ms on Windows source (5342 ms) was higher than macOS r3 source scan (2324 ms).
- Success predicates matched: peer_completion=100, needs=0, source re-paused.

## Boundary

One valid reverse-direction files initial-sync matrix cell only. Remaining reverse segments cell, 100k/500k matrix, layout selection, and ADR-0013 remain open. P0-1 is not complete.
