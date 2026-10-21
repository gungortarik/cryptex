# M0 P0-1 — Corruption Blast Radius Evidence (Physical Layout Candidates)

**Status:** VERIFIED LOCAL PHYSICAL PROBES (macOS + Windows)  
**Scope:** P0-1 physical packaging corruption blast-radius only  
**Architecture effect:** none; does not select a production layout or accept ADR-0013

## Authority

Measured against the implemented P0-1 harness candidates and the corruption blast-radius criterion in:

- `docs/06_STORAGE_ARCHITECTURE.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/milestones/M0_FOUNDATION.md`

Existing crash-probe tests remain intact and are not weakened.

## Shared probe profile

Both platforms used the same deterministic harness profile:

```text
command=corrupt-probe
payload_bytes=1024
files_events=22
segments_events=22
segment_events=5
corrupt_target_seq=7
git_branch=m0/p0-1-cursor
```

Platform observations are recorded in separate sections below.

---

## macOS physical probe

```text
platform=macos
run_root=~/Library/Application Support/Cryptex-Dev/benchmarks/p0-1/probes-20260930T044034Z
```

### Files candidate (macOS)

Deterministic corruption: truncate payload of event object `seq=7`.

```text
command=corrupt-probe
layout=files
corrupt_target_seq=7
corrupt_mode=truncate_event_payload
corrupt_original_object_bytes=1036
corrupt_truncated_object_bytes=524
corrupt_physical_objects_affected=1
corrupt_intact_event_objects=21
corrupt_unreadable_event_objects=1
corrupt_blast_radius_events=1
corrupt_recovery_boundary=delete_or_replace_single_event_object
corrupt_other_objects_remain_independently_readable=true
```

Observed boundary (macOS):

- exactly one physical event object becomes unreadable
- the other 21 event objects remain independently readable
- recovery boundary is replacement/deletion of that single object

### Segments candidate (macOS)

#### Active-tail truncation

```text
corrupt_active_tail_intact_events_before=2
corrupt_active_tail_recovered_events_after=21
corrupt_active_tail_truncated_flag=true
corrupt_active_tail_recovery_boundary=complete_records_before_truncated_tail
```

Observed boundary (macOS):

- incomplete suffix in the active `.open` segment is detected
- complete records before the truncated tail remain recoverable
- sealed segments remain readable in the same scan

#### Sealed-segment truncation

Deterministic truncation mid-payload of `seq=7` inside its sealed segment:

```text
corrupt_sealed_events_before=5
corrupt_sealed_intact_prefix_events=2
corrupt_sealed_offset_bytes=2596
corrupt_sealed_mode=truncate_mid_target_record_payload
corrupt_sealed_scan_ok=false
corrupt_sealed_scan_error=sealed object contains a truncated record payload
corrupt_sealed_recovery_boundary=fail_closed_on_truncated_sealed_object
```

Observed boundary (macOS):

- sealed truncation fails closed for that sealed object
- this is distinct from active-tail truncation, which recovers the complete prefix
- blast radius for a sealed object is the sealed segment being scanned, not a silent partial accept of a truncated sealed file

---

## Windows physical probe

```text
platform=windows
os=Microsoft Windows 11 Pro 10.0.26200
git_commit=791c2a619de80ffc355abf8f5ea37f6176512284
run_root=C:\Users\Tarik\AppData\Local\Cryptex-Dev\benchmarks\p0-1\probes-windows-20260930T045521Z
```

Raw outputs were captured under that Cryptex-Dev probe root (`corrupt-files.out.txt`, `corrupt-segments.out.txt`). Retained scale benchmark corpora were not modified.

### Files candidate (Windows)

Deterministic corruption: truncate payload of event object `seq=7`.

```text
command=corrupt-probe
layout=files
root=C:\Users\Tarik\AppData\Local\Cryptex-Dev\benchmarks\p0-1\probes-windows-20260930T045521Z\corrupt-files
corrupt_target_seq=7
corrupt_mode=truncate_event_payload
corrupt_original_object_bytes=1036
corrupt_truncated_object_bytes=524
corrupt_physical_objects_affected=1
corrupt_intact_event_objects=21
corrupt_unreadable_event_objects=1
corrupt_blast_radius_events=1
corrupt_recovery_boundary=delete_or_replace_single_event_object
corrupt_other_objects_remain_independently_readable=true
```

Observed boundary (Windows):

- exactly one physical event object becomes unreadable
- the other 21 event objects remain independently readable
- recovery boundary is replacement/deletion of that single object

### Segments candidate (Windows)

#### Active-tail truncation

```text
corrupt_active_tail_path=C:\Users\Tarik\AppData\Local\Cryptex-Dev\benchmarks\p0-1\probes-windows-20260930T045521Z\corrupt-segments\segments\00000000000000000004.open
corrupt_active_tail_intact_events_before=2
corrupt_active_tail_recovered_events_after=21
corrupt_active_tail_truncated_flag=true
corrupt_active_tail_recovery_boundary=complete_records_before_truncated_tail
```

Observed boundary (Windows):

- incomplete suffix in the active `.open` segment is detected
- complete records before the truncated tail remain recoverable
- sealed segments remain readable in the same scan

#### Sealed-segment truncation

Deterministic truncation mid-payload of `seq=7` inside its sealed segment:

```text
corrupt_sealed_case_root=C:\Users\Tarik\AppData\Local\Cryptex-Dev\benchmarks\p0-1\probes-windows-20260930T045521Z\corrupt-segments-sealed-case
corrupt_sealed_path=C:\Users\Tarik\AppData\Local\Cryptex-Dev\benchmarks\p0-1\probes-windows-20260930T045521Z\corrupt-segments-sealed-case\segments\00000000000000000001.seg
corrupt_sealed_events_before=5
corrupt_sealed_intact_prefix_events=2
corrupt_sealed_offset_bytes=2596
corrupt_sealed_mode=truncate_mid_target_record_payload
corrupt_sealed_scan_ok=false
corrupt_sealed_scan_error=sealed object contains a truncated record payload
corrupt_sealed_recovery_boundary=fail_closed_on_truncated_sealed_object
```

Observed boundary (Windows):

- sealed truncation fails closed for that sealed object
- this is distinct from active-tail truncation, which recovers the complete prefix
- blast radius for a sealed object is the sealed segment being scanned, not a silent partial accept of a truncated sealed file

---

## Distinction preserved (both platforms)

| Failure mode | Recoverable behavior in harness |
|---|---|
| active `.open` truncated tail | recover complete prefix; flag truncated tail |
| sealed `.seg` truncated mid-record | fail closed for that sealed object |
| single `.evt` truncated | one object unreadable; others intact |

## Boundary

No winner is selected. ADR-0013 remains unresolved. Production event semantics beyond this physical-layout probe are not inferred.
