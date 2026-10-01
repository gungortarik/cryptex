# M0 P0-1 — Initial Syncthing Scan/Sync Protocol

**Status:** IN PROGRESS  
**Scope:** P0-1 initial Syncthing scan/sync measurement only  
**Architecture effect:** none; this protocol does not select a production storage layout or complete P0-1/P0-4.

## Authority and requirement

`docs/23_REQUIRED_TECHNICAL_SPIKES.md` requires P0-1 to measure initial Syncthing scan/sync for the one-file-per-event and sealed-segment candidates at realistic scale. `docs/milestones/M0_FOUNDATION.md` requires the evidence to be reproducible from repository-controlled code/scripts and physically verified on macOS and Windows.

## Existing verified preconditions

Before this protocol starts:

- isolated Cryptex-Dev Syncthing homes exist on macOS and Windows
- both current benchmark processes use Syncthing v2.1.5
- GUI/API is bound only to `127.0.0.1:18384`
- sync listener is `tcp://0.0.0.0:22001`
- global discovery, local discovery, relay, NAT and automatic upgrades are disabled for the benchmark profiles
- peer addresses are explicit direct-LAN TCP addresses
- bidirectional direct TCP reachability is verified
- both peer configurations are verified and both runtime connections are established
- no benchmark folder existed before initial-sync preparation

These are P0-1 benchmark controls only, not production pairing semantics.

## Corpus preservation

The retained local-filesystem benchmark corpus is not used directly as a Syncthing folder.

For each initial-sync run, the repository preparation helper creates a new working copy under the runtime-only root:

```text
Cryptex-Dev/syncthing-p0-1-runs/<folder-id>/...
```

The retained source corpus under `Cryptex-Dev/benchmarks/p0-1/...` remains untouched. Copy time is setup cost and is explicitly outside the Syncthing timing window.

The helper refuses to delete or automatically reuse an existing work path.

## Folder roles

Each cross-platform run uses the same unique Syncthing folder ID on both peers.

Destination is prepared first:

- empty working directory
- folder type: `receiveonly`
- unpaused
- filesystem watcher disabled
- explicit intended peer only
- allowed to become idle before the timed run starts

Source is prepared second:

- working copy of the retained corpus
- folder type: `sendonly`
- paused
- filesystem watcher disabled
- explicit intended peer only
- source file count and logical byte count verified before Syncthing timing begins

The source remains paused until the timed runner starts. This removes human delay between separate-machine preparation commands from the measured interval.

## Timing definition

The timed runner executes on the source machine.

`t0` is taken immediately before the source folder is unpaused through the local Syncthing REST API.

Initial scan completion is the first observed state where:

- source `localFiles` equals the expected physical file count for the candidate
- source `localBytes` equals the expected stored logical byte count
- source folder state is no longer `scanning`

Initial synchronization completion is the first observation, after source scan completion, where Syncthing's peer completion endpoint reports:

- completion `>= 100`
- `needBytes = 0`
- `needItems = 0`
- `needDeletes = 0`

and the source folder status reports:

- `globalFiles` equals the expected physical file count for the candidate

Note: Syncthing `globalItems` also counts directories, so P0-1 timing completion verifies `globalFiles`, not `globalItems`.

The runner then pauses the source folder again to freeze the measured state.

Reported metrics include:

- UTC start / scan-complete / sync-complete timestamps
- initial source scan elapsed milliseconds
- total initial scan+sync elapsed milliseconds
- source local/global file and byte counts
- peer completion / need counts
- Syncthing version
- source and destination platform labels
- candidate layout and event count

## First controlled run

The first measurement is the 10k one-file-per-event candidate in this direction:

```text
source=macOS
source retained corpus=macos-10000-20260929T054105Z/data/files-10000
destination=Windows
layout=files
events=10000
expected physical files=10000
expected stored logical bytes=10360000
```

Working folder ID:

```text
p0-1-10k-files-macos-to-windows
```

### Windows destination preparation — VERIFIED

The guarded preparation helper was run on the physical Windows benchmark machine after the direct TCP peer connection checkpoint passed.

Observed output:

```text
home=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1
local_device_id=G3IQV46-V6WYJ2U-WDLQQOC-FXMJBGA-QRIXPNV-GVARBCC-IHYTOBI-BOIG6QA
peer_id=MHS264T-3YLCW4F-GL35TOB-DXZ6SUX-4MVY7YK-AZZFNZI-XFOXHN3-DYMMEQM
folder_id=p0-1-10k-files-macos-to-windows
folder_path=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1-runs\p0-1-10k-files-macos-to-windows\destination
role=destination
folder_type=receiveonly
fs_watcher_enabled=false
pre_config_files=0
pre_config_bytes=0
paused=false
peer_runtime_connected=true
scope=isolated_p0_1_initial_sync_preparation_only
```

This verifies that the Windows destination working folder was created empty, configured as `receiveonly`, left unpaused, and prepared while the intended macOS benchmark peer was connected. No source folder had yet been configured, so the timed sync had not started at this checkpoint.

### macOS source preparation — VERIFIED

The guarded preparation helper was then run on the physical macOS benchmark machine against the retained 10k one-file-per-event corpus.

Observed output:

```text
home=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1
local_device_id=MHS264T-3YLCW4F-GL35TOB-DXZ6SUX-4MVY7YK-AZZFNZI-XFOXHN3-DYMMEQM
peer_id=G3IQV46-V6WYJ2U-WDLQQOC-FXMJBGA-QRIXPNV-GVARBCC-IHYTOBI-BOIG6QA
folder_id=p0-1-10k-files-macos-to-windows
folder_path=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/p0-1-10k-files-macos-to-windows/source
role=source
folder_type=sendonly
fs_watcher_enabled=false
pre_config_files=10000
pre_config_bytes=10360000
source_corpus=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/benchmarks/p0-1/macos-10000-20260929T054105Z/data/files-10000
paused=true
peer_runtime_connected=true
scope=isolated_p0_1_initial_sync_preparation_only
```

This verifies that the retained corpus was copied into the isolated runtime working root, the working copy contains exactly 10,000 files and 10,360,000 logical bytes, and the source folder is `sendonly`, watcher-disabled, and paused. The intended Windows peer remained connected. Copy/setup time is outside the timed Syncthing measurement window.

Both endpoints are now prepared for the timed 10k one-file-per-event initial scan/sync measurement. The timed source runner may unpause the source, record source scan completion and peer synchronization completion, then pause the source again. After this run is physically verified and evidence is recorded, the equivalent 10k sealed-segment run will be performed under a distinct working folder ID. Reverse-direction and larger scale points remain required before P0-1 acceptance.

### Timed measurement attempt — FAILED / INVALID

The timed runner was executed once on macOS at `2026-09-30T04:35:51Z` and failed closed:

```text
EXIT_CODE=2
stderr=error: source folder must be paused before timed initial sync start
```

Live state at failure showed the source already unpaused and the Windows peer already at completion 100 with zero need counts. The prepared initial-sync experiment had been consumed outside the timed runner. Evidence:

- `evidence/m0/p0-1/syncthing-10k-files-macos-to-windows-TIMED-FAILED.md`

Per protocol, folder ID `p0-1-10k-files-macos-to-windows` must not be reused as a new initial run. A replacement cell must use a unique folder ID and fresh working paths.

## Consumed cell retirement (Windows) — VERIFIED

After the invalid timed attempt, the Windows isolated Syncthing profile still had folder ID `p0-1-10k-files-macos-to-windows` configured. The guarded retirement helper was run on the physical Windows machine **without** `--delete-work-path` so forensic working data is retained.

```text
home=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1
local_device_id=G3IQV46-V6WYJ2U-WDLQQOC-FXMJBGA-QRIXPNV-GVARBCC-IHYTOBI-BOIG6QA
peer_id=MHS264T-3YLCW4F-GL35TOB-DXZ6SUX-4MVY7YK-AZZFNZI-XFOXHN3-DYMMEQM
folder_id=p0-1-10k-files-macos-to-windows
folder_path=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1-runs\p0-1-10k-files-macos-to-windows\destination
config_removed=true
work_path_deleted=false
retained_corpus_untouched=true
scope=isolated_p0_1_folder_retirement_only
```

Post-retire verification on Windows:

- remaining isolated active folder count = 0 for the consumed ID
- forensic work path still present under `Cryptex-Dev/syncthing-p0-1-runs/p0-1-10k-files-macos-to-windows/`
- intended macOS peer remained connected over direct TCP (`192.168.50.104:22001`, type `tcp-server`)

## Consumed cell retirement (macOS) — VERIFIED

The matching consumed source folder was retired from the isolated macOS Syncthing config without deleting forensic work data:

```text
folder_id=p0-1-10k-files-macos-to-windows
config_removed=true
work_path_deleted=false
retained_corpus_untouched=true
scope=isolated_p0_1_folder_retirement_only
```

## Replacement cell `p0-1-10k-files-macos-to-windows-r2`

Replacement working folder ID:

```text
p0-1-10k-files-macos-to-windows-r2
```

Same logical matrix cell (10k files, macOS → Windows). The consumed ID must not be reused.

### Windows r2 destination preparation — VERIFIED

```text
home=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1
local_device_id=G3IQV46-V6WYJ2U-WDLQQOC-FXMJBGA-QRIXPNV-GVARBCC-IHYTOBI-BOIG6QA
peer_id=MHS264T-3YLCW4F-GL35TOB-DXZ6SUX-4MVY7YK-AZZFNZI-XFOXHN3-DYMMEQM
folder_id=p0-1-10k-files-macos-to-windows-r2
folder_path=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1-runs\p0-1-10k-files-macos-to-windows-r2\destination
role=destination
folder_type=receiveonly
fs_watcher_enabled=false
pre_config_files=0
pre_config_bytes=0
paused=false
peer_runtime_connected=true
scope=isolated_p0_1_initial_sync_preparation_only
```

### macOS r2 source preparation — VERIFIED

```text
folder_id=p0-1-10k-files-macos-to-windows-r2
role=source
folder_type=sendonly
fs_watcher_enabled=false
pre_config_files=10000
pre_config_bytes=10360000
paused=true
peer_runtime_connected=true
scope=isolated_p0_1_initial_sync_preparation_only
```

### Timed measurement attempt (r2) — FAILED / INVALID

The timed runner was executed once on macOS at `2026-09-30T05:15:03Z` and failed closed before unpausing:

```text
EXIT_CODE=2
stderr=error: GET .../rest/db/completion?... failed: HTTP 404 folder is paused
```

Root cause: the pre-unpause consumed-state preflight called Syncthing completion while the source was correctly paused; Syncthing v2.1.5 returns HTTP 404 `folder is paused` for that call. The source remained paused. Per stop protocol, folder ID `p0-1-10k-files-macos-to-windows-r2` must not be reused. Evidence:

- `evidence/m0/p0-1/syncthing-10k-files-macos-to-windows-r2-TIMED-FAILED.md`

A later unique cell (r3) is required after the runner treats paused-folder completion 404 as non-consumed during preflight.

## Replacement cell `p0-1-10k-files-macos-to-windows-r3`

### r2 retirement (macOS) before r3 — VERIFIED

```text
folder_id=p0-1-10k-files-macos-to-windows-r2
config_removed=true
work_path_deleted=false
retained_corpus_untouched=true
scope=isolated_p0_1_folder_retirement_only
```

### macOS r3 source preparation — VERIFIED

```text
folder_id=p0-1-10k-files-macos-to-windows-r3
role=source
folder_type=sendonly
fs_watcher_enabled=false
pre_config_files=10000
pre_config_bytes=10360000
paused=true
peer_runtime_connected=true
scope=isolated_p0_1_initial_sync_preparation_only
```

### Timed initial measurement (r3) — VERIFIED

```text
started_at_utc=2026-09-30T19:00:29.983Z
completed_at_utc=2026-09-30T19:01:26.705Z
initial_scan_ms=2324.080
initial_sync_total_ms=56716.159
source_local_files=10000
source_local_bytes=10360000
peer_completion=100
peer_need_items=0
peer_need_bytes=0
peer_need_deletes=0
source_paused_after_completion=true
syncthing_version=v2.1.5
```

Evidence: `evidence/m0/p0-1/syncthing-10k-files-macos-to-windows-r3.md`

### Incremental measurement (r3 files) — VERIFIED

Approved workload (+100 events / +103600 bytes) executed on the same valid cell after initial sync:

```text
incremental_sync_total_ms=2344.387
incremental_scan_ms=533.633
append_setup_ms=909.490
expected_files_after_append=10100
expected_bytes_after_append=10463600
peer_completion=100
source_paused_after_completion=true
```

Evidence: `evidence/m0/p0-1/syncthing-10k-files-macos-to-windows-r3-incremental.md`

This r3 working folder must not be reused as a fresh initial-sync cell.

## Cell `p0-1-10k-segments-macos-to-windows`

### r3 retirement (macOS) before segments cell — VERIFIED

```text
folder_id=p0-1-10k-files-macos-to-windows-r3
config_removed=true
work_path_deleted=false
retained_corpus_untouched=true
scope=isolated_p0_1_folder_retirement_only
```

### macOS segments source preparation — VERIFIED

```text
folder_id=p0-1-10k-segments-macos-to-windows
source_corpus=.../macos-10000-20260929T054105Z/data/segments-10000
role=source
folder_type=sendonly
fs_watcher_enabled=false
pre_config_files=10
pre_config_bytes=10360000
paused=true
peer_runtime_connected=true
scope=isolated_p0_1_initial_sync_preparation_only
```

### Timed initial measurement (segments) — VERIFIED

```text
started_at_utc=2026-09-30T19:18:47.659Z
completed_at_utc=2026-09-30T19:18:49.508Z
initial_scan_ms=290.412
initial_sync_total_ms=1843.674
source_local_files=10
source_local_bytes=10360000
peer_completion=100
peer_need_items=0
peer_need_bytes=0
peer_need_deletes=0
source_paused_after_completion=true
syncthing_version=v2.1.5
```

Evidence: `evidence/m0/p0-1/syncthing-10k-segments-macos-to-windows.md`

### Incremental measurement (segments) — VERIFIED

```text
append_setup_ms=424.346
incremental_scan_ms=276.749
incremental_sync_total_ms=1319.043
expected_files_after_append=11
expected_bytes_after_append=10463600
incremental_logical_bytes=103600
peer_completion=100
source_paused_after_completion=true
```

Evidence: `evidence/m0/p0-1/syncthing-10k-segments-macos-to-windows-incremental.md`

This segments working folder must not be reused as a fresh initial-sync cell.


## Cell `p0-1-10k-files-windows-to-macos`

### segments destination retirement (Windows) before reverse files cell — VERIFIED

```text
folder_id=p0-1-10k-segments-macos-to-windows
config_removed=true
work_path_deleted=false
retained_corpus_untouched=true
scope=isolated_p0_1_folder_retirement_only
```

### Windows files source preparation — VERIFIED

```text
folder_id=p0-1-10k-files-windows-to-macos
source_corpus=C:\Users\Tarik\AppData\Local\Cryptex-Dev\benchmarks\p0-1\windows-10000-20260929T055348Z\data\files-10000
role=source
folder_type=sendonly
fs_watcher_enabled=false
pre_config_files=10000
pre_config_bytes=10360000
paused=true
peer_runtime_connected=true
scope=isolated_p0_1_initial_sync_preparation_only
```

### Timed initial measurement (Windows→macOS files) — VERIFIED

```text
started_at_utc=2026-09-30T19:35:43.106Z
completed_at_utc=2026-09-30T19:36:43.739Z
initial_scan_ms=5342.066
initial_sync_total_ms=60630.917
source_local_files=10000
source_local_bytes=10360000
peer_completion=100
peer_need_items=0
peer_need_bytes=0
peer_need_deletes=0
source_paused_after_completion=true
syncthing_version=v2.1.5
```

Evidence: `evidence/m0/p0-1/syncthing-10k-files-windows-to-macos.md`

### Incremental measurement (Windows→macOS files) — VERIFIED

```text
append_setup_ms=1891.786
incremental_scan_ms=2813.945
incremental_sync_total_ms=3362.245
expected_files_after_append=10100
expected_bytes_after_append=10463600
incremental_logical_bytes=103600
peer_completion=100
source_paused_after_completion=true
```

Evidence: `evidence/m0/p0-1/syncthing-10k-files-windows-to-macos-incremental.md`

This Windows→macOS files working folder must not be reused as a fresh initial-sync cell.


## Cell `p0-1-10k-segments-windows-to-macos`

### reverse-files source retirement (Windows) before reverse segments cell — VERIFIED

```text
folder_id=p0-1-10k-files-windows-to-macos
config_removed=true
work_path_deleted=false
retained_corpus_untouched=true
scope=isolated_p0_1_folder_retirement_only
```

### Windows segments source preparation — VERIFIED

```text
folder_id=p0-1-10k-segments-windows-to-macos
source_corpus=C:\Users\Tarik\AppData\Local\Cryptex-Dev\benchmarks\p0-1\windows-10000-20260929T055348Z\data\segments-10000
role=source
folder_type=sendonly
fs_watcher_enabled=false
pre_config_files=10
pre_config_bytes=10360000
paused=true
peer_runtime_connected=true
scope=isolated_p0_1_initial_sync_preparation_only
```

### Timed initial measurement (Windows→macOS segments) — VERIFIED

```text
started_at_utc=2026-09-30T19:58:47.197Z
completed_at_utc=2026-09-30T19:58:48.754Z
initial_scan_ms=276.231
initial_sync_total_ms=1555.402
source_local_files=10
source_local_bytes=10360000
peer_completion=100
peer_need_items=0
peer_need_bytes=0
peer_need_deletes=0
source_paused_after_completion=true
syncthing_version=v2.1.5
```

Evidence: `evidence/m0/p0-1/syncthing-10k-segments-windows-to-macos.md`

### Incremental measurement (Windows→macOS segments) — VERIFIED

```text
append_setup_ms=181.194
incremental_scan_ms=268.950
incremental_sync_total_ms=1294.808
expected_files_after_append=11
expected_bytes_after_append=10463600
incremental_logical_bytes=103600
peer_completion=100
source_paused_after_completion=true
```

Evidence: `evidence/m0/p0-1/syncthing-10k-segments-windows-to-macos-incremental.md`

This Windows→macOS segments working folder must not be reused as a fresh initial-sync cell.

## Cell `p0-1-100k-files-macos-to-windows` (and replacements)

### Timed measurement attempt — FAILED / INVALID

Default 7200s timeout expired at ~75% peer completion. Folder ID must not be reused.

Evidence: `evidence/m0/p0-1/syncthing-100k-files-macos-to-windows-TIMED-FAILED.md`

### Replacement cell `p0-1-100k-files-macos-to-windows-r2` — FAILED / INVALID

Timed runner disappeared before completion while Syncthing itself continued transferring. No confirmed root cause is claimed for the runner disappearance. Folder ID must not be reused as a new initial-timed cell.

Evidence: `evidence/m0/p0-1/syncthing-100k-files-macos-to-windows-r2-TIMED-FAILED.md`

### Replacement cell `p0-1-100k-files-macos-to-windows-r3` — VERIFIED

```text
folder_id=p0-1-100k-files-macos-to-windows-r3
layout=files
events=100000
source_platform=macos
destination_platform=windows
syncthing_version=v2.1.5
START_TIMESTAMP_UTC=2026-10-01T02:11:43Z
started_at_utc=2026-10-01T02:11:43.263Z
scan_completed_at_utc=2026-10-01T02:12:15.634Z
completed_at_utc=2026-10-01T02:53:21.928Z
COMPLETION_TIMESTAMP_UTC=2026-10-01T02:53:21Z
EXIT_CODE=0
initial_scan_ms=32371.464
initial_sync_total_ms=2498632.230
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
```

Evidence: `evidence/m0/p0-1/syncthing-100k-files-macos-to-windows-r3.md`

Retain this r3 working folder for the approved incremental measurement on the same cell. Do not retire it yet. Do not start incremental from this documentation update alone.

## Boundary

A successful Syncthing transport measurement does not prove Cryptex replication semantics or complete P0-4. Production pairing/device trust remains governed by the appropriate later milestone and accepted architecture.
