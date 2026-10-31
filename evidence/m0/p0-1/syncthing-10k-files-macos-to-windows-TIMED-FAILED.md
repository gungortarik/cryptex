# M0 P0-1 — Timed Initial Sync Attempt FAILED (consumed preparation)

**Status:** FAILED / INVALID FOR TIMING  
**Scope:** first controlled 10k one-file-per-event macOS→Windows initial Syncthing measurement  
**Architecture effect:** none; no layout selected; ADR-0013 remains unresolved

## Attempt

```text
START_TIMESTAMP_UTC=2026-09-30T04:35:51Z
COMPLETION_TIMESTAMP_UTC=2026-09-30T04:35:51Z
EXIT_CODE=2
branch=m0/p0-1-cursor
command=node tools/m0-storage-bench/scripts/run-syncthing-p0-1-initial-sync.mjs
folder_id=p0-1-10k-files-macos-to-windows
layout=files
events=10000
source_platform=macos
destination_platform=windows
syncthing_version=v2.1.5
```

## Runner output

```text
stdout=(empty)
stderr=error: source folder must be paused before timed initial sync start
```

## Observed Syncthing state at failure (macOS source)

```text
paused=False
type=sendonly
fsWatcherEnabled=False
state=idle
stateChanged=2026-09-30T00:20:51-04:00
localFiles=10000
localBytes=10360000
globalFiles=10000
globalBytes=10360000
inSyncFiles=10000
peer_completion=100
peer_need_items=0
peer_need_bytes=0
peer_need_deletes=0
peer_global_items=10001
peer_sequence=20002
```

Raw capture (runtime only, outside repository):

```text
~/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/p0-1-10k-files-macos-to-windows/timed-runner-attempt-failed.txt
```

## Interpretation

Preparation evidence previously recorded `paused=true` for this source folder. Before the timed runner executed, the live configuration already showed `paused=false` and peer completion already reported `100` with zero need counts.

This means the prepared initial-sync experiment was consumed outside the timed runner. Any near-zero timing that could be obtained by forcing an unpaused re-measurement would not represent a valid initial scan/sync.

Per the P0-1 protocol, this folder ID must not be reused as a new "initial" run.

## Actions taken

- timed runner executed once and failed closed
- no folder reset
- no folder recreation
- no deletion of retained corpus
- no second timed attempt against this folder ID

## Required follow-up

A replacement initial-sync measurement for the same logical matrix cell must use:

- a unique new Syncthing folder ID
- fresh isolated working paths under `Cryptex-Dev/syncthing-p0-1-runs/`
- untouched retained corpus under `Cryptex-Dev/benchmarks/p0-1/`
- destination prepared empty first
- source prepared paused second
- timed unpause only after both sides verify the paused/empty preconditions

Windows destination verification of the already-synced working folder remains a cross-machine observation and does not convert this attempt into valid timing evidence.
