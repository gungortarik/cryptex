# M0 P0-1 — PROPOSED Incremental Syncthing Workload

**Status:** PROPOSED — not executed; awaiting acceptance of exact semantics  
**Scope:** define a deterministic incremental sync workload for P0-1 only  
**Architecture effect:** none until accepted and measured

## Why this is proposed

Authoritative docs require incremental Syncthing sync evidence (`docs/23_REQUIRED_TECHNICAL_SPIKES.md`, `evidence/m0/p0-1/README.md`) but do not freeze an exact change set, byte counts, or timing boundaries.

No silent invention of benchmark semantics has been executed as an accepted measurement.

## Proposed deterministic change set

Use the same profile already used for local filesystem evidence:

```text
payload_bytes=1024
segment_events=1000
record_framing_overhead_bytes=12
bytes_per_event=1036
```

After a verified initial sync for a given scale/layout/direction cell has completed and both sides are idle with the source paused:

### Files layout incremental operation

Append exactly **100** new immutable event objects on the source working copy using the harness `append files` command, continuing sequence numbers from the existing corpus.

```text
incremental_events=100
incremental_physical_files_added=100
incremental_logical_bytes_added=103600
operation=append_only_no_mutation_of_existing_objects
```

### Segments layout equivalent operation

Append exactly **100** new records on the source working copy using `append segments` with the same `segment_events=1000`.

Depending on the residual open-segment room after the initial corpus:

- records append into the active `.open` segment and/or create/seal additional segment objects
- no existing sealed `.seg` object is mutated in place

```text
incremental_events=100
incremental_logical_bytes_added=103600
operation=append_only_into_active_or_new_segment_objects
```

Exact physical object delta for segments is measured from pre/post `scan` output for that run rather than assumed, because residual open-segment occupancy depends on the prior corpus size modulo `segment_events`.

## Timing definition

`t0` is taken on the source machine immediately before:

1. the 100-event append completes into the already-configured source working folder, and
2. the source folder is unpaused (if it was paused after the initial measurement)

Preferred controlled sequence:

1. source paused after initial sync completion
2. append 100 events into the paused source working path (setup cost; optionally reported separately)
3. `t0` immediately before source unpause
4. scan-complete: source localFiles/localBytes reflect baseline + incremental expected counts and state != scanning
5. sync-complete: peer completion >= 100, needBytes/needItems/needDeletes = 0, source globalFiles equals expected post-increment file count
6. pause source again

If append-while-paused is rejected by tooling constraints, an accepted alternate is:

- unpause first only after append while destination remains connected, with `t0` still defined as the unpause instant and append treated as setup outside the timed window

## Completion definition

Same completion predicates as the initial-sync protocol, applied to the post-increment expected counts.

## Reset / precondition rules

- never reuse a working folder that has already received the incremental append as a fresh "initial" run
- incremental measurement requires a prior valid initial sync for the same folder ID
- retained corpora under `Cryptex-Dev/benchmarks/p0-1/` remain untouched; appends occur only in `syncthing-p0-1-runs/<folder-id>/...`
- filesystem watcher remains disabled
- explicit direct TCP only
- unique folder IDs per matrix cell remain mandatory

## Fairness rationale

- identical event count, payload size, and framing for both layouts
- append-only workload matches immutable canonical-write assumptions
- Syncthing transport conditions match the initial-sync controls
- differences then isolate packaging effects (many new files vs few segment object updates) rather than unequal logical change volume

## Required decision

Accept, revise, or reject this exact incremental workload before treating any incremental timing as P0-1 evidence.
