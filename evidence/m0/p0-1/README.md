# M0 P0-1 — Physical Event Storage Benchmark Evidence

**Status:** COMPLETE (P0-1 spike resolved; M0 remains IN PROGRESS)

This directory records reproducible evidence for M0 P0-1. The production physical packaging decision is recorded in ADR-0013 (ACCEPTED: sealed per-device segments). Individual measurement files do not themselves complete P0-4 or M0.

## Authority

The benchmark follows:

- `docs/06_STORAGE_ARCHITECTURE.md`
- `docs/22_RISK_REGISTER.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/milestones/M0_FOUNDATION.md`
- `checklists/M0_FOUNDATION_GATE.md`

If benchmark evidence conflicts with an accepted architectural decision, stop and use the ADR/specification process.

## Fixed requirements

The comparison must include:

1. one immutable file per synthetic event
2. immutable sealed segment/pack files
3. a hybrid only if evidence from the simpler candidates justifies it

The required event-count matrix is:

- 10,000
- 100,000
- 500,000

The required evidence includes:

- append latency
- directory enumeration
- initial Syncthing scan/sync
- incremental Syncthing sync
- rebuild performance
- disk overhead
- crash behavior
- purge complexity
- corruption blast radius

Platform-sensitive results must be collected on both macOS and Windows. CI smoke tests are not substitutes for physical-machine measurements.

## Safety and reproducibility rules

- Use only synthetic data.
- Store generated benchmark data under the `Cryptex-Dev` application-data root, never inside the source repository.
- Do not delete or reuse an existing benchmark root automatically.
- Run from a clean Git worktree and record the exact commit.
- Use the same declared `payload_bytes` and `segment_events` values when comparing platforms and layouts at a given experiment profile.
- Experiment inputs are benchmark parameters, not production defaults or architecture decisions.
- Preserve raw text output until the P0-1 decision is accepted.

## Local filesystem runners

The repository provides platform runners that build the release benchmark binary and execute one required scale point for both candidate layouts plus crash probes.

macOS:

```bash
./tools/m0-storage-bench/scripts/run-local-macos.sh <events> <payload_bytes> <segment_events>
```

Windows PowerShell:

```powershell
./tools/m0-storage-bench/scripts/run-local-windows.ps1 -Events <events> -PayloadBytes <payload_bytes> -SegmentEvents <segment_events>
```

Allowed values for `<events>` are exactly `10000`, `100000`, or `500000`.

Each runner records platform/toolchain metadata and keeps the generated data so the same corpus can be used for the Syncthing phase. The runners never treat their own output as an accepted result.

## Syncthing phase

The local harness does not claim Syncthing performance evidence. For each required scale point and layout, record separately:

- Syncthing version
- source platform and destination platform
- isolated benchmark folder paths
- initial scan start/end and initial synchronization completion time
- incremental workload definition
- incremental scan/synchronization completion time
- conflict/versioning observations, if any
- antivirus/indexing conditions that materially affect the run

Benchmark transport folders must remain synthetic and isolated from any real Cryptex brain or personal data. Syncthing transport success is not Cryptex semantic acceptance.

The exact managed-Syncthing product integration remains P0-4 and must not be inferred from this benchmark.

## Disk-overhead evidence

The harness reports logical stored bytes and physical object counts. macOS runner output also records `du` allocated KiB for each generated root.

Windows real allocation/directory-metadata overhead was captured as a separate platform measurement on the retained 500k files corpus; logical byte count alone is insufficient.

## Decision record

ADR-0013 is **ACCEPTED**. Canonical packaging is immutable sealed per-device segment/pack files. One-file-per-event is rejected. Hybrid is not selected.

`segment_events=1000` was the verified P0-1 benchmark profile and is **not** a frozen production constant.

M0 remains IN PROGRESS. P0-2, P0-3 and P0-4 are unchanged.

## Current evidence index (non-exhaustive)

Local filesystem scale matrix (macOS + Windows, 10k/100k/500k) is recorded in the dated `macos-*` and `windows-*` files in this directory.

Additional P0-1 evidence files:

- `syncthing-setup.md`, `syncthing-direct-tcp-isolation.md`, `syncthing-direct-tcp-reachability.md`, `syncthing-peer-pairing.md`
- `syncthing-initial-sync-protocol.md` — includes consumed-cell Windows retirement and r2 Windows destination preparation
- `syncthing-10k-files-macos-to-windows-TIMED-FAILED.md` — first timed initial-sync attempt failed because preparation was consumed; do not reuse that folder ID
- `syncthing-10k-files-macos-to-windows-r2-TIMED-FAILED.md` — r2 timed attempt failed on paused-folder completion preflight before unpause; do not reuse r2
- `syncthing-10k-files-macos-to-windows-r3.md` — **valid** 10k files macOS→Windows initial sync timing
- `syncthing-10k-files-macos-to-windows-r3-incremental.md` — **valid** approved incremental sync on the r3 files cell
- `syncthing-10k-segments-macos-to-windows.md` — **valid** 10k segments macOS→Windows initial sync timing
- `syncthing-10k-segments-macos-to-windows-incremental.md` — **valid** approved incremental sync on the segments cell
- `syncthing-10k-files-windows-to-macos.md` — **valid** 10k files Windows→macOS initial sync timing
- `syncthing-10k-files-windows-to-macos-incremental.md` — **valid** approved incremental sync on the Windows→macOS files cell
- `syncthing-10k-segments-windows-to-macos.md` — **valid** 10k segments Windows→macOS initial sync timing
- `syncthing-10k-segments-windows-to-macos-incremental.md` — **valid** approved incremental sync on the Windows→macOS segments cell
- `syncthing-100k-files-macos-to-windows-TIMED-FAILED.md` — 100k files macOS→Windows timed initial sync timed out at default 7200s (~75% peer completion); do not reuse that folder ID
- `syncthing-100k-files-macos-to-windows-r2-TIMED-FAILED.md` — r2 timed runner disappeared before completion while Syncthing continued transferring; INVALID for timing; do not reuse r2; no confirmed root cause claimed
- `syncthing-100k-files-macos-to-windows-r3.md` — **valid** 100k files macOS→Windows initial sync timing; retain cell (do not retire yet)
- `syncthing-100k-files-macos-to-windows-r3-incremental.md` — **valid** approved incremental sync on the r3 100k files cell
- `syncthing-100k-segments-macos-to-windows.md` — **valid** 100k segments macOS→Windows initial sync timing; retain cell (do not retire yet)
- `syncthing-100k-segments-macos-to-windows-incremental.md` — **valid** approved incremental sync on the 100k segments cell
- `syncthing-100k-files-windows-to-macos.md` — **valid** 100k files Windows→macOS initial sync timing; retain cell (do not retire yet)
- `syncthing-100k-files-windows-to-macos-incremental.md` — **valid** approved incremental sync on the Windows→macOS 100k files cell
- `syncthing-100k-segments-windows-to-macos.md` — **valid** 100k segments Windows→macOS initial sync timing; retain cell (do not retire yet)
- `syncthing-100k-segments-windows-to-macos-incremental.md` — **valid** approved incremental sync on the Windows→macOS 100k segments cell
- `syncthing-500k-files-macos-to-windows-TIMED-FAILED.md` — 500k files macOS→Windows timed initial sync interrupted by source low-power sleep/hibernate; FAILED/INVALID for timing; do not reuse that folder ID
- `syncthing-500k-files-macos-to-windows-r2-TIMED-FAILED.md` — 500k files macOS→Windows r2 **completed controlled timeout / impracticality** at 21600s (~21.2% peer completion); not a Syncthing crash; not a timing result; no further 500k files retry; do not reuse r2
- `syncthing-500k-segments-macos-to-windows.md` — **valid** 500k segments macOS→Windows initial sync timing; retain cell (do not retire yet)
- `syncthing-500k-segments-macos-to-windows-incremental.md` — **valid** approved incremental sync on the 500k segments macOS→Windows cell
- `syncthing-500k-segments-windows-to-macos.md` — **valid** 500k segments Windows→macOS initial sync timing; retain cell (do not retire yet)
- `syncthing-500k-segments-windows-to-macos-incremental.md` — **valid** approved incremental sync on the 500k segments Windows→macOS cell
- `syncthing-500k-segments-runbook.md` — runbook/checklist; Cell A and Cell B timed measurements VERIFIED; do not retire folders from this file
- `docs/adr/ADR-0013-physical-event-storage-layout.md` — **ACCEPTED** (sealed per-device segments; files rejected; hybrid not selected)
- `purge-complexity.md` — physical purge-complexity probes for both candidates, with macOS and Windows sections separated
- `corruption-blast-radius.md` — physical corruption blast-radius probes for both candidates, with macOS and Windows sections separated
- `syncthing-incremental-workload-APPROVED-FOR-MEASUREMENT.md` — incremental workload protocol; required cells executed; does not accept ADR-0013
- `syncthing-incremental-workload-PROPOSED.md` — pointer to the approved measurement protocol

