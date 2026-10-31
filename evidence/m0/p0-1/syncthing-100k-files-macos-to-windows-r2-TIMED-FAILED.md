# M0 P0-1 — Timed Initial Sync Attempt INVALID (100k files macOS→Windows r2 runner loss)

**Status:** FAILED / INVALID FOR TIMING — do not reuse this folder ID  
**Scope:** replacement 100k one-file-per-event macOS→Windows initial Syncthing measurement  
**Architecture effect:** none; no layout selected; ADR-0013 remains unresolved

## Attempt

```text
branch=m0/p0-1-cursor
folder_id=p0-1-100k-files-macos-to-windows-r2
layout=files
events=100000
expected_files=100000
expected_bytes=103600000
source_platform=macos
destination_platform=windows
syncthing_version=v2.1.5
```

Prior invalid cell for this logical matrix point (`p0-1-100k-files-macos-to-windows`, default 7200s timeout) was not reused. This r2 cell was the unique replacement.

## Why this cell is INVALID for timing

The timed runner process disappeared before completion while Syncthing itself continued transferring. Empty runner stdout/stderr were left behind; no successful timed-result KV was produced by the runner.

A forensic snapshot taken after runner loss (runtime only):

```text
~/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/p0-1-100k-files-macos-to-windows-r2/forensic-runner-loss-macos.txt
```

Observed at forensic time (`2026-10-01T01:57:20Z`):

```text
source_paused=false
source_local_files=100000
source_local_bytes=103600000
source_global_files=100000
source_global_bytes=103600000
peer_completion=94.283
peer_need_items=5717
peer_need_bytes=5922812
peer_need_deletes=0
```

Interpretation limits:

- this cell is consumed/invalid for timing evidence
- Syncthing continued transferring after the runner was gone
- **no confirmed root cause is claimed** for the runner disappearance
- folder ID `p0-1-100k-files-macos-to-windows-r2` must not be reused as a new initial-timed cell

## Actions taken / not taken

- no retry against r2 as a timed initial cell
- no claim of valid initial_scan_ms / initial_sync_total_ms from r2
- no ADR-0013 acceptance
- a subsequent unique folder ID (r3) was required for a fresh timed attempt

## Follow-up

Valid replacement: `p0-1-100k-files-macos-to-windows-r3`  
Evidence: `evidence/m0/p0-1/syncthing-100k-files-macos-to-windows-r3.md`
