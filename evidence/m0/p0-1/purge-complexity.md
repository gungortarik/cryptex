# M0 P0-1 — Purge Complexity Evidence (Physical Layout Candidates)

**Status:** VERIFIED LOCAL PHYSICAL PROBES (macOS + Windows)  
**Scope:** P0-1 physical packaging purge-complexity only  
**Architecture effect:** none; does not select a production layout or accept ADR-0013

## Authority

Measured against the implemented P0-1 harness candidates in `tools/m0-storage-bench` and the purge-complexity criterion in:

- `docs/06_STORAGE_ARCHITECTURE.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/milestones/M0_FOUNDATION.md`

This is not M10 purge-product behavior. It measures physical packaging cost/complexity only.

## Shared probe profile

Both platforms used the same deterministic harness profile:

```text
command=purge-probe
payload_bytes=1024
files_events_before=20
segments_events_before=20
segment_events=5
purge_target_seq=7
git_branch=m0/p0-1-cursor
```

Platform observations are recorded in separate sections below and must not be collapsed into a single unverified cross-platform claim beyond the raw outputs shown.

---

## macOS physical probe

```text
platform=macos
run_root=~/Library/Application Support/Cryptex-Dev/benchmarks/p0-1/probes-20260930T044034Z
```

### Files candidate (macOS)

Raw output:

```text
command=purge-probe
layout=files
events=19
physical_objects=19
max_events_per_object=1
purge_target_seq=7
purge_physical_objects_removed=1
purge_physical_objects_rewritten=0
purge_bytes_removed=1036
purge_removal_granularity=single_event_object
purge_atomicity=filesystem_unlink_of_immutable_object
purge_interruption_behavior=unlink_is_all_or_nothing_for_one_object
purge_events_before=20
purge_events_after=19
purge_unaffected_events=19
purge_segment_rewrite_required=false
```

Observed physical implications (macOS):

- one immutable `.evt` object is removed
- unaffected event objects remain readable
- no rewrite of neighboring objects is required
- interruption boundary is the filesystem unlink of a single object

### Segments candidate (macOS)

Raw output:

```text
command=purge-probe
layout=segments
events=19
physical_objects=4
max_events_per_object=5
purge_target_seq=7
purge_physical_objects_removed=0
purge_physical_objects_rewritten=1
purge_bytes_removed=1036
purge_removal_granularity=record_inside_sealed_segment
purge_atomicity=temp_rewrite_then_rename_over_sealed_segment
purge_interruption_behavior=temp_rewrite_visible_until_rename; rename replaces whole segment object
purge_interrupted_temp_observed_before_rename=true
purge_events_before=20
purge_events_after=19
purge_segment_events_before=5
purge_segment_events_after=4
purge_segment_rewrite_required=true
purge_replacement_strategy=copy_intact_records_excluding_target_then_rename
```

Observed physical implications (macOS):

- purging one record requires rewriting the sealed segment that contains it
- replacement uses temp write + sync + rename over the sealed object
- a temporary rewrite object is observable before rename commits
- other sealed/open segment objects are not rewritten for this single-record purge

---

## Windows physical probe

```text
platform=windows
os=Microsoft Windows 11 Pro 10.0.26200
git_commit=791c2a619de80ffc355abf8f5ea37f6176512284
run_root=C:\Users\Tarik\AppData\Local\Cryptex-Dev\benchmarks\p0-1\probes-windows-20260930T045521Z
```

Raw outputs were captured under that Cryptex-Dev probe root (`purge-files.out.txt`, `purge-segments.out.txt`). Retained scale benchmark corpora were not modified.

### Files candidate (Windows)

Raw output:

```text
command=purge-probe
layout=files
root=C:\Users\Tarik\AppData\Local\Cryptex-Dev\benchmarks\p0-1\probes-windows-20260930T045521Z\purge-files
events=19
payload_bytes=19456
stored_bytes=19684
physical_objects=19
max_events_per_object=1
enumeration_ms=0.102
rebuild_ms=1.056
truncated_active_tail=false
purge_target_seq=7
purge_physical_objects_removed=1
purge_physical_objects_rewritten=0
purge_bytes_removed=1036
purge_removal_granularity=single_event_object
purge_atomicity=filesystem_unlink_of_immutable_object
purge_interruption_behavior=unlink_is_all_or_nothing_for_one_object
purge_events_before=20
purge_events_after=19
purge_unaffected_events=19
purge_segment_rewrite_required=false
```

Observed physical implications (Windows):

- one immutable `.evt` object is removed
- unaffected event objects remain readable
- no rewrite of neighboring objects is required
- interruption boundary is the filesystem unlink of a single object

### Segments candidate (Windows)

Raw output:

```text
command=purge-probe
layout=segments
root=C:\Users\Tarik\AppData\Local\Cryptex-Dev\benchmarks\p0-1\probes-windows-20260930T045521Z\purge-segments
events=19
payload_bytes=19456
stored_bytes=19684
physical_objects=4
max_events_per_object=5
enumeration_ms=0.094
rebuild_ms=0.390
truncated_active_tail=false
purge_target_seq=7
purge_physical_objects_removed=0
purge_physical_objects_rewritten=1
purge_bytes_removed=1036
purge_removal_granularity=record_inside_sealed_segment
purge_atomicity=temp_rewrite_then_rename_over_sealed_segment
purge_interruption_behavior=temp_rewrite_visible_until_rename; rename replaces whole segment object
purge_interrupted_temp_observed_before_rename=true
purge_events_before=20
purge_events_after=19
purge_segment_events_before=5
purge_segment_events_after=4
purge_segment_rewrite_required=true
purge_replacement_strategy=copy_intact_records_excluding_target_then_rename
```

Observed physical implications (Windows):

- purging one record requires rewriting the sealed segment that contains it
- replacement uses temp write + sync + rename over the sealed object
- a temporary rewrite object is observable before rename commits
- other sealed/open segment objects are not rewritten for this single-record purge

---

## Syncthing / version-history consequences

Not physically measured in these probes. Under Syncthing transport:

- files purge appears as deletion of one synced object
- segment purge appears as replacement of one synced segment object
- if Cryptex-controlled versioning/history retains prior versions, those historical objects may temporarily preserve pre-purge bytes until scrubbed by later purge/version-cleanup milestones (M10/P0-4 concerns)

No claim is made here that Syncthing itself implements Cryptex purge semantics.

## Boundary

No winner is selected. ADR-0013 remains unresolved.
