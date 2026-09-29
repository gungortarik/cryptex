# 25 — Cryptex Performance and Scale

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the accepted Cryptex performance/scale verification boundaries supported by the current specification set.

It does not invent latency or throughput targets that are not present in the authoritative sources.

Where exact numeric budgets are not yet specified, the active milestone must establish measurable acceptance criteria before claiming the relevant performance requirement complete.

## 2. General rule

Performance is part of product correctness when poor performance would invalidate the architecture, recovery model or supported scale.

Performance claims require measured evidence on the relevant platforms and data scales.

Configuration presence or microbenchmarks that do not represent the actual workload are not sufficient.

## 3. M0 physical event-layout benchmark

M0 P0-1 is the primary early scale gate.

It must compare:

- one immutable file per event
- immutable sealed per-device segment/pack files
- a hybrid thresholded approach only if evidence justifies it

at realistic event counts:

- 10k events
- 100k events
- 500k events

on both macOS and Windows.

## 4. Required P0-1 measurements

The storage-layout benchmark must measure:

- event append latency
- directory enumeration
- initial Syncthing scan/sync
- incremental sync
- rebuild speed
- disk overhead
- crash behavior
- purge complexity
- corruption blast radius

The selected design must be the simplest option that satisfies scale and recovery goals on both platforms.

## 5. No premature physical-layout assumption

One-file-per-event must not be treated as accepted production architecture before the benchmark.

Likewise, sealed segments or hybrid packaging must not be selected merely because they appear more scalable in theory.

ADR-0013 records the evidence-based decision.

## 6. Cross-platform scale requirement

A performance result obtained on only one operating system is insufficient for a cross-platform architecture decision.

Filesystem, antivirus/indexing, process and path behavior may differ materially between macOS and Windows.

The required P0 benchmark therefore evaluates both platforms.

## 7. Search and projection performance

M3 implements SQLite/FTS5 search and rebuild behavior.

Search performance must remain useful offline without an LLM.

Relevant M3 evidence should cover the workloads introduced by that milestone, including rebuild/checkpoint and corruption-rebuild behavior.

This document does not invent specific query-latency budgets absent from the authoritative sources; the M3 contract must define measurable acceptance before completion.

## 8. Knowledge-engine performance

M4 adds entity resolution, aliases, facts, relations, confidence/state and semantic consolidation.

Knowledge consolidation must remain deterministic and testable at the supported workload.

Exact performance budgets for consolidation are not specified by the current sources and must be defined when the M4 contract requires them.

## 9. Obsidian projection performance

M6 materializes a complete local derived vault.

Projection and rebuild behavior must remain practical at the supported Brain scale.

Attachment projection has a dedicated P1 spike comparing hard-link versus copy fallback behavior and cleanup on both operating systems.

Exact timing budgets are not yet sourced.

## 10. Replication performance

Syncthing is transport, not semantic authority.

Replication scale evidence begins in M0 through initial/incremental sync measurements in P0-1 and the managed-Syncthing P0-4 proof.

M8 productizes replication behavior.

M16 performs broader sync stress and offline/reconnect soak testing.

## 11. Backup and restore performance

The backup encryption/container P1 spike must exercise large objects, interrupted backup, restore and key recovery.

M11 must verify backup/restore behavior with real reconstruction evidence.

M16 includes broader backup/restore drills.

A fast backup is not sufficient if restore or verification is unreliable.

## 12. Failure performance

Performance testing must not ignore failure/recovery cost.

Examples include:

- rebuild after derived-state loss
- recovery after daemon/process failure
- corruption blast radius
- interrupted backup/restore
- offline/reconnect catch-up
- purge cleanup complexity

Architecture choices must not optimize only happy-path latency while making recovery impractical.

## 13. UI responsiveness

Cryptex Desktop is a premium desktop application.

UI work must remain responsive enough for the implemented user workflows and must not hide long-running operations behind false healthy states.

Exact interaction budgets are not specified in the current sources and must be established by the relevant UI milestone if needed for acceptance.

## 14. Runtime/Doctor truthfulness

Doctor and runtime-health surfaces must not claim a subsystem is Verified merely because a lightweight configuration probe passes.

Where performance or scale is part of readiness, the relevant verification must exercise the real workload required by the milestone.

## 15. Benchmark data policy

Scale tests use synthetic data unless a later explicitly approved milestone requires otherwise.

Real personal/Cortex memory must not be used during early milestones.

Benchmark generation must be reproducible enough to compare candidate architectures fairly.

## 16. Reproducibility

Performance evidence should record enough context to reproduce meaningful comparisons, including where applicable:

- platform/architecture
- toolchain/build revision
- data scale
- candidate/layout tested
- relevant runtime configuration
- measured workload
- result summary

This document does not mandate a specific benchmark harness format.

## 17. M16 hardening

M16 includes:

- large-scale tests
- sync stress
- backup/restore drills
- sleep/wake/offline/reconnect soak

M16 complements, rather than replaces, milestone-specific performance verification performed earlier.

## 18. Performance budgets not yet sourced

The current authoritative sources provide explicit M0 event-count scales and required benchmark dimensions but do not provide universal numeric latency/throughput budgets for every subsystem.

Therefore this document intentionally does not invent them.

When a later milestone depends on a concrete budget, that milestone/specification must define and accept the budget before completion is claimed.

## 19. Source basis

This document consolidates performance/scale requirements from:

- `docs/00_MASTER_SPEC.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`

It does not independently add unsupported numeric targets.
