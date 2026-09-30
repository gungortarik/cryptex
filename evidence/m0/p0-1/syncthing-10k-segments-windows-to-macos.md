# M0 P0-1 — Valid Initial Syncthing Measurement (10k segments, Windows→macOS)

**Status:** VERIFIED TIMED MEASUREMENT  
**Scope:** P0-1 initial Syncthing scan/sync for sealed-segment packaging at 10k logical events, Windows source → macOS destination  
**Architecture effect:** none; does not select a production layout or accept ADR-0013

## Cell

```text
folder_id=p0-1-10k-segments-windows-to-macos
layout=segments
events=10000
expected_files=10
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
source_corpus=C:\Users\Tarik\AppData\Local\Cryptex-Dev\benchmarks\p0-1\windows-10000-20260929T055348Z\data\segments-10000
corpus_identity=windows-10000-20260929T055348Z / segments-10000
corpus_files=10
corpus_bytes=10360000
verified=true
```

Only one `segments-10000` directory exists under the Windows P0-1 benchmark root. The retained corpus was not regenerated. Independent recount and `verify-retained-corpus.mjs` both matched before source preparation.

## Preconditions

- prior reverse-files source cell `p0-1-10k-files-windows-to-macos` retired from Windows Syncthing config only (`config_removed=true`, `work_path_deleted=false`, forensic path retained)
- macOS destination already prepared as `receiveonly`, empty, unpaused, watcher disabled, peer connected
- Windows source prepared `sendonly`, `paused=true`, watcher disabled, `pre_config_files=10`, `pre_config_bytes=10360000`
- peer connected over direct LAN TCP (`tcp-client` to `192.168.50.104:22001`); Syncthing v2.1.5 both sides
- isolated transport: GUI `127.0.0.1:18384`, listener `tcp://0.0.0.0:22001`, global/local discovery off, relays off, NAT off, auto-upgrade off

## Timed runner output (raw)

```text
home=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1
local_device_id=G3IQV46-V6WYJ2U-WDLQQOC-FXMJBGA-QRIXPNV-GVARBCC-IHYTOBI-BOIG6QA
peer_id=MHS264T-3YLCW4F-GL35TOB-DXZ6SUX-4MVY7YK-AZZFNZI-XFOXHN3-DYMMEQM
folder_id=p0-1-10k-segments-windows-to-macos
folder_path=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1-runs\p0-1-10k-segments-windows-to-macos\source
layout=segments
events=10000
source_platform=windows
destination_platform=macos
syncthing_version=v2.1.5
started_at_utc=2026-09-30T19:58:47.197Z
scan_completed_at_utc=2026-09-30T19:58:47.473Z
completed_at_utc=2026-09-30T19:58:48.754Z
initial_scan_ms=276.231
initial_sync_total_ms=1555.402
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
START_TIMESTAMP_UTC=2026-09-30T19:58:47Z
COMPLETION_TIMESTAMP_UTC=2026-09-30T19:58:49Z
EXIT_CODE=0
```

Runtime captures (outside repository):

```text
%LOCALAPPDATA%\Cryptex-Dev\syncthing-p0-1-runs\p0-1-10k-segments-windows-to-macos\timed-runner-capture.txt
%LOCALAPPDATA%\Cryptex-Dev\syncthing-p0-1-runs\p0-1-10k-segments-windows-to-macos\timed-result-2026-09-30T195849111Z.txt
```

## Commands / tooling

```text
retire-syncthing-p0-1-folder.mjs (files reverse cell; no --delete-work-path)
verify-retained-corpus.mjs
prepare-syncthing-p0-1-initial-sync.mjs --role source
run-syncthing-p0-1-initial-sync.mjs
capture-syncthing-p0-1-raw-result.mjs
```

## Observations (direction/layout comparison later)

- Reverse of the valid macOS→Windows segments cell at the same logical scale.
- Windows→macOS initial_sync_total_ms ≈ 1.56s vs macOS→Windows segments ≈ 1.84s.
- Far faster than the reverse-files cell at this scale (≈ 60.6s), consistent with packaging effect (10 objects vs 10000).
- Success predicates matched: peer_completion=100, needs=0, source re-paused.

## Boundary

One valid reverse-direction segments initial-sync matrix cell only. 100k/500k matrix, layout selection, and ADR-0013 remain open. P0-1 is not complete.
