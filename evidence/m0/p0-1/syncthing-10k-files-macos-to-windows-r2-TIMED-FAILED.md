# M0 P0-1 — Timed Initial Sync Attempt FAILED (r2 tooling preflight)

**Status:** FAILED / INVALID FOR TIMING — do not reuse this folder ID  
**Scope:** replacement 10k one-file-per-event macOS→Windows initial Syncthing measurement  
**Architecture effect:** none

## Attempt

```text
START_TIMESTAMP_UTC=2026-09-30T05:15:03Z
COMPLETION_TIMESTAMP_UTC=2026-09-30T05:15:03Z
EXIT_CODE=2
branch=m0/p0-1-cursor
folder_id=p0-1-10k-files-macos-to-windows-r2
layout=files
events=10000
source_platform=macos
destination_platform=windows
syncthing_version=v2.1.5
```

## Runner output

```text
stdout=(empty)
stderr=error: GET http://127.0.0.1:18384/rest/db/completion?folder=p0-1-10k-files-macos-to-windows-r2&device=G3IQV46-... failed: HTTP 404 folder is paused
```

Raw capture (runtime only):

```text
~/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/p0-1-10k-files-macos-to-windows-r2/timed-runner-capture.txt
```

## Observed state immediately after failure

```text
paused=True
type=sendonly
fsWatcherEnabled=False
source_work_files=10000
source_work_bytes=10360000
peer_connected=True
peer_type=tcp-client
peer_address=192.168.50.137:22001
completion_http=404
completion_error=folder is paused
```

The retained corpus remained verified:

```text
files=10000
bytes=10360000
verified=true
```

## Interpretation

The timed runner's pre-unpause "already consumed" preflight called Syncthing's completion endpoint while the source folder was still correctly paused. Syncthing v2.1.5 returns HTTP 404 `folder is paused` for that endpoint in the paused state. The runner treated that as a hard failure and exited before unpausing.

Consequences:

- source remained paused
- timed measurement never started
- this folder ID must not be reused as a new timed initial run per the stop protocol
- a subsequent unique folder ID (r3) is required after the runner is fixed to tolerate paused-folder completion 404 during preflight

## Actions taken

- timed runner executed once and failed closed
- no retry against r2
- no folder reset / recreate / delete
- retained corpus untouched
- runner preflight will be fixed to use soft-fail for paused completion lookups before any r3 attempt
