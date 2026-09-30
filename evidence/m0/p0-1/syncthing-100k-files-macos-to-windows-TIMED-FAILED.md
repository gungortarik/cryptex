# M0 P0-1 — Timed Initial Sync Attempt FAILED (100k files macOS→Windows timeout)

**Status:** FAILED / INVALID FOR TIMING — do not reuse this folder ID  
**Scope:** 100k one-file-per-event macOS→Windows initial Syncthing measurement  
**Architecture effect:** none

## Attempt

```text
START_TIMESTAMP_UTC=2026-09-30T20:10:58Z
COMPLETION_TIMESTAMP_UTC=2026-09-30T22:10:59Z
EXIT_CODE=2
branch=m0/p0-1-cursor
folder_id=p0-1-100k-files-macos-to-windows
layout=files
events=100000
expected_files=100000
expected_bytes=103600000
source_platform=macos
destination_platform=windows
syncthing_version=v2.1.5
timeout_seconds=7200
```

## Runner output

```text
stdout=(empty)
stderr=error: peer synchronization did not reach completion within 7200 seconds
```

Raw capture (runtime only):

```text
~/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/p0-1-100k-files-macos-to-windows/timed-runner-capture.txt
```

## Retained corpus (precondition)

```text
source_corpus=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/benchmarks/p0-1/macos-100000-20260929T062316Z/data/files-100000
run_id=macos-100000-20260929T062316Z
corpus_files=100000
corpus_bytes=103600000
verified=true
```

Evidence identity: `evidence/m0/p0-1/macos-100000-20260929T062316Z.md`

## Observed state immediately after failure

```text
paused=False
type=sendonly
state=idle
localFiles=100000
localBytes=103600000
globalFiles=100000
inSyncFiles=100000
peer_completion=75.165
peer_need_items=24835
peer_need_bytes=25729060
peer_need_deletes=0
peer_connected=True
peer_type=tcp-server
```

Source scan reached expected local counts, but peer synchronization had not reached completion when the default 7200s runner timeout expired (~75% peer completion).

## Actions taken / not taken

- timed runner executed once and failed closed on timeout
- no retry
- no folder reset / recreate / delete
- no incremental run
- retained corpus verified untouched after failure
- folder ID must not be reused as a new initial-timed cell

## Follow-up implication

A replacement unique folder ID is required for any later 100k-files macOS→Windows initial timing attempt. Any such replacement should use an explicitly larger `--timeout-seconds` appropriate to observed 100k one-file-per-event LAN transfer rates; that is tooling/protocol adjustment for a **new** cell, not a retry of this one.
