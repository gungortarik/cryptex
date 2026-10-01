# M0 P0-1 — Timed Initial Sync Attempt FAILED (500k files macOS→Windows sleep interruption)

**Status:** FAILED / INVALID FOR TIMING — do not reuse this folder ID  
**Scope:** 500k one-file-per-event macOS→Windows initial Syncthing measurement  
**Architecture effect:** none; no layout selected; ADR-0013 remains unresolved

## Attempt

```text
branch=m0/p0-1-cursor
folder_id=p0-1-500k-files-macos-to-windows
layout=files
events=500000
expected_files=500000
expected_bytes=518000000
source_platform=macos
destination_platform=windows
syncthing_version=v2.1.5
START_TIMESTAMP_UTC=2026-10-01T15:05:39Z
COMPLETION_TIMESTAMP_UTC=(none)
EXIT_CODE=(none)
```

## Wrapper / capture state after interruption

```text
timed-initial-capture.txt=41 bytes
timed-initial-stdout.txt=0 bytes
timed-initial-stderr.txt=0 bytes
```

No successful timed-result KV and no completion wrapper timestamps were produced.

Runtime paths retained under:

```text
~/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/p0-1-500k-files-macos-to-windows/
```

## Forensic process check

Observed at `2026-10-01T19:26:20Z`:

```text
known_pids=3464,3465
known_pids_absent=true
runner_processes=none
caffeinate_processes=none
```

## Power-event interpretation (observed facts only)

- macOS did **not** reboot
- the physical source machine experienced a low-power sleep / hibernate interruption during the timed window
- that interruption breaks the continuity required for a valid initial-sync measurement
- interrupted elapsed wall time must **not** be converted into `initial_scan_ms` / `initial_sync_total_ms` benchmark results
- no speculative causal claims beyond the available power-log observation are recorded here

## Source containment

```text
folder_observed_paused_after_interruption=False
source_manually_paused_at_utc=2026-10-01T19:10:42Z
source_paused_after_containment=True
type=sendonly
source_runtime_path_retained=true
```

## Windows destination forensic snapshot

Observed at `2026-10-01T19:12:45Z`:

```text
state=sync-preparing
localFiles=161491
localBytes=167304676
globalFiles=500000
globalBytes=518000000
needFiles=338509
needBytes=350695324
needDeletes=0
errors=0
pullErrors=0
physical_files=161560
physical_bytes=167376160
```

Destination was incomplete. Syncthing DB counts and physical filesystem counts differed during the interrupted state (`localFiles=161491` vs `physical_files=161560`).

## Why this cell is INVALID for timing

- timed runner continuity was broken by source-machine low-power sleep / hibernate
- wrapper produced no completion timestamp / exit code
- destination never reached completion predicates
- this attempt must not be used in layout-comparison timing
- preserve as forensic evidence only

## Actions taken / not taken

- no retry against this folder ID
- no delete of runtime / forensic paths
- no incremental run
- no ADR-0013 acceptance
- no production layout selection
- folder ID `p0-1-500k-files-macos-to-windows` must never be reused as a new initial-timed cell

## Follow-up implication

Any later 500k-files macOS→Windows initial timing attempt requires a fresh unique folder ID. Proposed replacement:

```text
p0-1-500k-files-macos-to-windows-r2
```

Do not start that replacement from this documentation update alone.

## Boundary

P0-1 remains IN PROGRESS. ADR-0013 remains OPEN. No production layout is selected.
