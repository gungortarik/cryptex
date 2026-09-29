# M0 P0-1 Storage Benchmark Harness

This tool exists only to produce reproducible evidence for **M0 P0-1 — Physical Event Storage Layout**.

Authoritative basis:

- `docs/06_STORAGE_ARCHITECTURE.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/milestones/M0_FOUNDATION.md`
- `docs/30_ADR_INDEX.md`

It does **not** define the production event format, event schema, purge algorithm, replication semantics, or accepted physical layout. ADR-0013 remains pending until the required cross-platform measurements are complete and accepted.

## Candidate layouts represented

`files`

- one immutable canonical object per synthetic event
- write to a same-directory temporary file, sync it, then rename to the final `.evt` path

`segments`

- records append to one active `.open` per-device-style segment
- each appended record is synced
- a full active segment is synced and renamed to immutable `.seg`
- the benchmark framing is intentionally simple and is **not** a production format decision

The hybrid option from the technical spike is intentionally not encoded as a production candidate. It should only be added if evidence from the simpler candidates justifies it.

## Safety

The `generate` and `crash-probe` commands refuse to use a non-empty benchmark root. The tool never deletes an existing benchmark directory for the user.

Run benchmark data under the Cryptex development runtime root, not inside the source repository.

Suggested roots:

- macOS: `~/Library/Application Support/Cryptex-Dev/benchmarks/p0-1/`
- Windows: `%LOCALAPPDATA%\Cryptex-Dev\benchmarks\p0-1\`

## Commands

```text
m0-storage-bench generate files <events> <payload_bytes> <root>
m0-storage-bench generate segments <events> <payload_bytes> <root> <segment_events>
m0-storage-bench append files <events> <payload_bytes> <root>
m0-storage-bench append segments <events> <payload_bytes> <root> <segment_events>
m0-storage-bench scan <files|segments> <root>
m0-storage-bench crash-probe <files|segments> <payload_bytes> <root>
```

Example shape only; payload and segment sizes are experiment inputs and are not accepted defaults:

```bash
cargo run -p m0-storage-bench --release -- \
  generate files 10000 <payload_bytes> <runtime_root>
```

The authoritative event-count matrix is:

- 10,000 events
- 100,000 events
- 500,000 events

Do not treat a small CI smoke/test run as evidence for those scale points.

## Metrics emitted by the tool

The tool prints line-oriented `key=value` measurements so runs can be captured without a parser dependency.

It currently measures:

- append elapsed time
- directory enumeration time
- rebuild/read time
- payload bytes
- stored logical bytes
- physical object count
- maximum events sharing one physical object
- detection/recovery of a truncated active-segment tail

`max_events_per_object` is an objective proxy useful when reasoning about purge complexity and corruption blast radius. It is not itself a purge policy.

## Measurements that still require platform integration

The harness alone does **not** satisfy P0-1. The accepted spike also requires evidence for:

- initial Syncthing scan/sync
- incremental Syncthing sync
- real disk-allocation overhead, not only logical byte count
- behavior with normal Windows/macOS antivirus/indexing conditions
- crash behavior on both supported operating systems
- purge complexity assessment
- corruption blast-radius assessment

Those measurements must be run on both macOS and Windows using the same declared experiment profile and preserved as M0 evidence before ADR-0013 can be accepted.

## CI scope

CI only compiles and tests the harness with tiny synthetic fixtures. It is a correctness smoke test, not the 10k/100k/500k benchmark and not a substitute for physical-machine verification.
