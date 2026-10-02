# M0 P0-1 Storage Benchmark Harness

This tool exists only to produce reproducible evidence for **M0 P0-1 — Physical Event Storage Layout**.

Authoritative basis:

- `docs/06_STORAGE_ARCHITECTURE.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/milestones/M0_FOUNDATION.md`
- `docs/30_ADR_INDEX.md`

It does **not** define production event schema, purge algorithm, or replication semantics. Physical packaging is accepted in ADR-0013 (sealed per-device segments). This harness's `segment_events=1000` profile is a benchmark parameter, not a production constant. P0-4 remains separate.

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
m0-storage-bench purge-probe files <payload_bytes> <root>
m0-storage-bench purge-probe segments <payload_bytes> <root> <segment_events>
m0-storage-bench corrupt-probe files <payload_bytes> <root>
m0-storage-bench corrupt-probe segments <payload_bytes> <root> <segment_events>
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

## Physical-machine runners

Repository-controlled runners execute one required event-count point for both layouts and retain the generated synthetic data for the later Syncthing phase.

macOS:

```bash
./tools/m0-storage-bench/scripts/run-local-macos.sh <events> <payload_bytes> <segment_events>
```

Windows PowerShell:

```powershell
./tools/m0-storage-bench/scripts/run-local-windows.ps1 -Events <events> -PayloadBytes <payload_bytes> -SegmentEvents <segment_events>
```

The runners require a clean worktree, build the release harness with the committed Cargo lockfile, record the exact Git commit and platform/toolchain metadata, and write only under the Cryptex-Dev benchmark root.

The input values are experiment parameters, not accepted production defaults. Use the same declared values when comparing platforms and layouts. See `evidence/m0/p0-1/README.md` for the evidence protocol.

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
- purge-complexity probe outputs for files and segments candidates
- corruption blast-radius probe outputs for files and segments candidates

`max_events_per_object` is an objective proxy useful when reasoning about purge complexity and corruption blast radius. It is not itself a purge policy.

## Guarded Syncthing helpers

Repository scripts under `tools/m0-storage-bench/scripts/` manage isolated P0-1 Syncthing experiments. They refuse non-`Cryptex-Dev` homes, repository paths, retained-corpus deletion, and silent reuse of consumed initial-sync folder IDs.

Relevant helpers:

- `configure-syncthing-p0-1.mjs` / `pair-syncthing-p0-1.mjs` / `verify-syncthing-p0-1-peer-connection.mjs`
- `prepare-syncthing-p0-1-initial-sync.mjs`
- `run-syncthing-p0-1-initial-sync.mjs`
- `run-syncthing-p0-1-incremental-sync.mjs`
- `capture-syncthing-p0-1-raw-result.mjs` — writes a guarded runtime capture under `syncthing-p0-1-runs/<folder-id>/`; `--raw-file` accepts UTF-8 and BOM-marked UTF-16LE (`FF FE`) only
- `retire-syncthing-p0-1-folder.mjs`
- `verify-retained-corpus.mjs`
- `plan-syncthing-p0-1-matrix.mjs`

## Measurements that still require platform integration

The harness alone does **not** satisfy P0-1. The accepted spike also requires evidence for:

- initial Syncthing scan/sync
- incremental Syncthing sync
- real disk-allocation overhead, not only logical byte count
- behavior with normal Windows/macOS antivirus/indexing conditions
- crash behavior on both supported operating systems

Purge-complexity and corruption blast-radius probes are now implemented in the harness (`purge-probe`, `corrupt-probe`) and recorded under `evidence/m0/p0-1/`. Syncthing transport measurements remain separate and must still be completed on both platforms for the required matrix.


## CI scope

CI compiles, formats, lints and tests the harness with tiny synthetic fixtures on macOS and Windows. It is a correctness smoke test, not the 10k/100k/500k benchmark and not a substitute for physical-machine verification.
