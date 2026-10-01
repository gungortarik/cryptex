# M0 P0-1 — APPROVED Incremental Syncthing Workload (Measurement Protocol)

**Status:** APPROVED FOR P0-1 MEASUREMENT — partially executed  
**Scope:** deterministic incremental sync workload for P0-1 only  
**Architecture effect:** none; does **not** accept ADR-0013 and does **not** select a layout

## Execution status

Executed so far:

- `p0-1-10k-files-macos-to-windows-r3` files layout — VERIFIED  
  Evidence: `evidence/m0/p0-1/syncthing-10k-files-macos-to-windows-r3-incremental.md`
- `p0-1-10k-segments-macos-to-windows` segments layout — VERIFIED  
  Evidence: `evidence/m0/p0-1/syncthing-10k-segments-macos-to-windows-incremental.md`
- `p0-1-10k-files-windows-to-macos` files layout — VERIFIED  
  Evidence: `evidence/m0/p0-1/syncthing-10k-files-windows-to-macos-incremental.md`
- `p0-1-10k-segments-windows-to-macos` segments layout — VERIFIED  
  Evidence: `evidence/m0/p0-1/syncthing-10k-segments-windows-to-macos-incremental.md`
- `p0-1-100k-files-macos-to-windows-r3` files layout — VERIFIED  
  Evidence: `evidence/m0/p0-1/syncthing-100k-files-macos-to-windows-r3-incremental.md`
- `p0-1-100k-segments-macos-to-windows` segments layout — VERIFIED  
  Evidence: `evidence/m0/p0-1/syncthing-100k-segments-macos-to-windows-incremental.md`
- `p0-1-100k-files-windows-to-macos` files layout — VERIFIED  
  Evidence: `evidence/m0/p0-1/syncthing-100k-files-windows-to-macos-incremental.md`
- `p0-1-100k-segments-windows-to-macos` segments layout — VERIFIED  
  Evidence: `evidence/m0/p0-1/syncthing-100k-segments-windows-to-macos-incremental.md`

Remaining incremental cells still require a prior valid initial-sync measurement for that matrix cell.

## Approval boundary

This document freezes the incremental **benchmark workload and timing protocol** only.

It does not:

- accept ADR-0013
- select files vs segments as the production layout
- authorize treating any incremental timing as complete until a valid initial-sync cell exists and the correct physical source machine executes the timed run

## Why this workload exists

Authoritative docs require incremental Syncthing sync evidence (`docs/23_REQUIRED_TECHNICAL_SPIKES.md`, `evidence/m0/p0-1/README.md`). The exact change set, byte counts, and timing boundaries are defined here so measurements are reproducible and comparable.

## Deterministic change set

Use the same profile already used for local filesystem evidence:

```text
payload_bytes=1024
segment_events=1000
record_framing_overhead_bytes=12
bytes_per_event=1036
```

Approved incremental volume:

```text
incremental_events=100
bytes_per_event=1036
incremental_logical_bytes=103600
operation=append_only_no_mutation_of_existing_objects
```

After a verified initial sync for a given scale/layout/direction cell has completed and both sides are idle with the source paused:

### Files layout incremental operation

Append exactly **100** new immutable event objects on the source working copy using the harness `append files` command, continuing sequence numbers from the existing corpus.

```text
incremental_events=100
incremental_physical_files_added=100
incremental_logical_bytes=103600
operation=append_only_no_mutation_of_existing_objects
```

### Segments layout equivalent operation

Append exactly **100** new records on the source working copy using `append segments` with the same `segment_events=1000`.

```text
incremental_events=100
incremental_logical_bytes=103600
operation=append_only_into_active_or_new_segment_objects
```

Do **not** assume a physical-object delta for segments. Residual open-segment occupancy depends on the prior corpus size modulo `segment_events`.

Before `t0`, measure and freeze the actual post-append expectations from source `scan` / Syncthing status:

- post-append physical-object count
- post-append logical byte count (expected `incremental_logical_bytes` contribution is 103600)

Completion uses those measured post-append expected values, not an assumed segment object delta.

## Timing definition (approved)

Append occurs while the source folder is paused. Append duration is **outside** the timed sync window.

Approved controlled sequence:

1. source paused after initial sync completion
2. append 100 events into the paused source working path (setup cost; optionally reported separately; not part of timed sync)
3. measure and freeze post-append expected physical-object and logical-byte counts
4. `t0` immediately before source unpause, after append has completed
5. scan-complete: source localFiles/localBytes reflect the frozen post-append expected counts and state != scanning
6. sync-complete: peer completion >= 100, needBytes/needItems/needDeletes = 0, source globalFiles equals the frozen post-append expected file/object count as applicable to the layout/completion predicates
7. pause source again

## Completion definition

Same completion predicates as the initial-sync protocol, applied to the measured post-append expected counts frozen before `t0`.

## Reset / precondition rules

- never reuse a working folder that has already received the incremental append as a fresh "initial" run
- incremental measurement requires a prior **valid** initial sync for the same folder ID
- consumed/invalid initial-sync cells (for example `p0-1-10k-files-macos-to-windows`) are not valid prerequisites
- retained corpora under `Cryptex-Dev/benchmarks/p0-1/` remain untouched; appends occur only in `syncthing-p0-1-runs/<folder-id>/...`
- filesystem watcher remains disabled
- explicit direct TCP only
- unique folder IDs per matrix cell remain mandatory

## Fairness rationale

- identical event count, payload size, and framing for both layouts
- append-only workload matches immutable canonical-write assumptions
- Syncthing transport conditions match the initial-sync controls
- differences then isolate packaging effects (many new files vs segment object updates) rather than unequal logical change volume
- segment physical-object expectations are measured, not assumed

## Execution gate

Do **not** execute incremental sync yet unless:

1. a valid initial-sync cell exists for that matrix cell, and
2. the correct physical source machine is available to run the timed protocol

This approval is workload/protocol only. Layout selection and ADR-0013 remain open.
