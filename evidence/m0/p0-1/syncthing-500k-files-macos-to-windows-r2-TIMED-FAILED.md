# M0 P0-1 — Timed Initial Sync COMPLETED CONTROLLED TIMEOUT (500k files macOS→Windows r2)

> **Historical status note (2026-10-05):** Status statements below reflect the state when this dated plan/result was recorded, before M0 closeout. Current M0 status is COMPLETE for the synthetic foundation scope; scoped ADR-0012/0004 are ACCEPTED. This note does not expand this artifact's evidence, satisfy production residuals, or authorize M1. See `evidence/m0/m0-closure-audit-20261005.md`.

**Status:** COMPLETED CONTROLLED TIMEOUT / IMPRACTICALITY EVIDENCE — do not reuse this folder ID  
**Scope:** replacement 500k one-file-per-event macOS→Windows initial Syncthing measurement  
**Architecture effect:** none; no layout selected; ADR-0013 remains PROPOSED / PENDING

This cell is **not** a Syncthing crash. The timed runner reached its declared timeout and failed closed with `EXIT_CODE=2`. Do **not** convert the 21600s wall window into `initial_scan_ms` / `initial_sync_total_ms`. No further 500k files retry is scheduled.

## Attempt

```text
branch=m0/p0-1-cursor
folder_id=p0-1-500k-files-macos-to-windows-r2
layout=files
events=500000
expected_files=500000
expected_bytes=518000000
source_platform=macos
destination_platform=windows
timeout_seconds=21600
START_TIMESTAMP_UTC=2026-10-01T19:45:24Z
COMPLETION_TIMESTAMP_UTC=2026-10-02T01:45:25Z
EXIT_CODE=2
```

Prior invalid cell for this logical matrix point (`p0-1-500k-files-macos-to-windows`, sleep/hibernate interruption) was not reused. This r2 cell was the unique replacement.

## Runner output

```text
stderr:
error: peer synchronization did not reach completion within 21600 seconds
```

The runner process was gone after the timeout, which is expected for a closed `EXIT_CODE=2` wrapper. This is not evidence of a Syncthing crash.

## Post-timeout forensic snapshot (source)

```text
state=idle
localFiles=500000
localBytes=518000000
globalFiles=500000
globalBytes=518000000
errors=0
pullErrors=0
peer_completion=21.2256
peer_need_items=393872
peer_need_bytes=408051392
peer_need_deletes=0
source_paused=False
folder_type=sendonly
peer_connected=True
peer_type=tcp-client
peer_address=192.168.50.137:22001
```

Source scan/local inventory matched the expected 500000 files / 518000000 bytes. Peer synchronization had not reached completion when the 21600-second timeout expired (~21.2% peer completion; 393872 items / 408051392 bytes still needed). Peer remained connected over direct TCP.

## Why this is impracticality evidence, not a timing result

- the wrapper completed continuously at the declared timeout (`EXIT_CODE=2`)
- destination peer completion predicates were not met
- remaining need was large (~79% of items still needed)
- this must not be used as a layout-comparison timing number
- it **is** usable as controlled evidence that the 500k files candidate did not complete initial Syncthing sync within six hours on this macOS→Windows LAN cell

## Retirement

```text
macos_config_removed=true
windows_config_removed=true
work_paths_retained=true
retained_corpus_untouched=true
delete_work_path=false
```

Work paths were retained on both sides. The retained corpus was not mutated. `retire-syncthing-p0-1-folder.mjs` was not used with `--delete-work-path`.

## Actions taken / not taken

- timed runner executed once and failed closed on timeout
- no Syncthing crash is claimed
- no conversion of 21600s into `initial_sync_total_ms`
- no incremental run on this folder ID
- no further 500k files retry scheduled
- no ADR-0013 acceptance
- no production layout selection
- folder ID `p0-1-500k-files-macos-to-windows-r2` must not be reused as a new initial-timed cell

## Follow-up implication

Remaining P0-1 Syncthing evidence is the 500k **segments** cells (see `syncthing-500k-segments-runbook.md`). Do not start another 500k files cell.

## Boundary

P0-1 remains IN PROGRESS. ADR-0013 remains PROPOSED / PENDING. No production layout is selected.
