# 17 — Cryptex Test and Failure Matrix

**Status:** ACCEPTED
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the Cryptex testing and failure-verification contract.

Cryptex is not considered correct merely because it builds, launches or passes a happy-path test.

Relevant behavior must be verified against:

- normal operation
- failure paths
- corruption/integrity failures
- recovery/rebuild behavior
- security boundaries
- cross-platform behavior
- offline/reconnect behavior where applicable

The complete product-wide failure matrix is finalized during M16 Hardening.

Earlier milestones must nevertheless test every failure mode relevant to behavior introduced by that milestone.

## 2. Authority

Testing and verification must remain consistent with:

1. `docs/00_MASTER_SPEC.md`
2. accepted ADRs
3. the active milestone contract
4. `docs/15_PLATFORM_MACOS_WINDOWS.md`
5. `docs/21_DEFINITION_OF_DONE.md`
6. `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
7. `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`

A test must not redefine architecture.

If test evidence contradicts an accepted decision, implementation stops and the architecture/ADR conflict is raised.

## 3. Core quality rule

A feature is complete only after all relevant verification categories pass.

Depending on scope, these include:

- unit tests
- integration tests
- cross-platform verification
- failure-path tests
- security tests/review
- recovery tests
- visual/runtime verification
- required CI checks

A successful compile or launch alone is not sufficient.

Tests must not be weakened merely to make CI green unless the specification has explicitly changed.

## 4. Milestone-scoped testing

Testing follows milestone scope.

A milestone is responsible for:

- behavior introduced in that milestone
- failure modes created by that behavior
- interactions explicitly required by its acceptance criteria
- regressions against already accepted invariants where relevant

Later milestones must not be implemented merely to satisfy a speculative test.

Likewise, a future failure scenario must not become an artificial blocker for an earlier milestone unless the earlier milestone's behavior depends on it.

The complete cross-system failure campaign belongs to M16.

## 5. Test data policy

Early development and automated testing use:

- Golden Brain
- synthetic Dev Brain
- synthetic fixtures
- generated benchmark data

Real Cortex or personal memory must not be used during early milestones.

Test fixtures must not contain:

- real personal memory
- secrets
- credentials
- Recovery Kits
- private production logs

Synthetic fixtures may be committed to the repository when appropriate.

Runtime test output must remain outside the repository unless explicitly retained as a sanitized test artifact.

## 6. Cross-platform rule

macOS and Windows are first-class test targets.

Where functionality is shared or platform-sensitive, relevant behavior must be verified on both supported operating systems.

Success on one platform is not proof of cross-platform correctness.

Platform-sensitive categories include, where applicable:

- filesystem behavior
- runtime paths
- IPC
- daemon lifecycle
- process supervision
- Syncthing integration
- Desktop launch
- update/reconnect behavior
- packaging
- UI/native-feeling behavior

CI configuration alone is not sufficient evidence for runtime behavior that CI does not actually exercise.

## 7. Failure reporting requirements

Integrity-relevant and runtime failures must be explicit.

Implementation must not silently swallow:

- corruption
- signature failures
- schema incompatibility
- authorization failures
- malformed IPC requests
- incompatible protocol versions
- daemon startup failures
- reconnect failures
- rebuild failures

Where applicable, errors should use typed errors and structured diagnostics.

Failure reporting must not expose memory bodies, secrets, recovery material or credentials by default.

## 8. Canonical storage and integrity matrix

When canonical event storage is implemented, relevant verification must include:

| Scenario | Required property |
|---|---|
| valid canonical data | accepted and processed deterministically |
| invalid/corrupt canonical data | detected rather than silently accepted |
| schema incompatibility | explicit compatible handling or rejection |
| signature/integrity failure | never silently swallowed |
| interrupted/failed write | canonical durable state must remain safe |
| derived-state loss | canonical data remains authoritative |
| rebuild from canonical data | disposable state can be reconstructed |
| purge-related integrity bridge | integrity remains verifiable without retaining purged content |

Exact storage tests for M2 depend on the physical packaging accepted in ADR-0013 (sealed per-device segments). Seal size and container filenames remain M2 parameters.

M0 must not implement the full M2 event store opportunistically.

## 9. Derived-state matrix

Derived/disposable state includes SQLite, search indexes, materialized knowledge, caches and Obsidian projections.

Applicable later milestones must verify:

- missing derived database
- corrupt derived database
- stale derived database
- rebuild from canonical state
- interrupted rebuild
- safe retry/restart behavior

Loss of derived state must not redefine or destroy canonical memory.

The complete SQLite/search rebuild suite belongs to the milestone implementing those systems.

## 10. M0 — Physical storage spike failures

P0-1 must evaluate failure characteristics of each physical packaging candidate.

Required evidence includes:

- crash behavior
- corruption blast radius
- purge complexity
- rebuild behavior
- realistic-scale performance on macOS and Windows

The benchmark must compare:

- one immutable file per event
- immutable sealed segment/pack files
- hybrid thresholded layout only if evidence justifies it

The simplest design satisfying scale and recovery goals must be selected.

## 11. M0 — Daemon lifecycle failure matrix

P0-2 must prove relevant failure behavior on both platforms.

Required scenarios include:

| Scenario | Required evidence |
|---|---|
| main window closes | engine behavior matches accepted lifecycle |
| reboot/login | required runtime returns as designed |
| daemon crash | detection/restart behavior is verified |
| duplicate daemon start | only one canonical writer remains active |
| Desktop loses daemon | reconnect behavior is verified |
| Desktop/daemon update | compatible transition is verified |
| failed/partial update | controlled error/rollback path exists |
| lifecycle failure | canonical data is not damaged |

The exact supervision mechanism remains unresolved until M0 evidence and ADR-0012.

## 12. M0 — IPC failure matrix

P0-3 must verify:

| Scenario | Required property |
|---|---|
| authorized current-user client | valid connection |
| unauthorized client | rejected |
| incompatible protocol version | explicit rejection/negotiation |
| malformed request | rejected safely |
| oversized request | rejected safely |
| daemon restart | clients can recover/reconnect |
| invalid client role/capability | denied |
| macOS endpoint | safe local socket behavior |
| Windows endpoint | secure named-pipe behavior |

The exact IPC library/implementation is recorded later in the ADR-0007 addendum.

## 13. M0 — Managed Syncthing failure matrix

P0-4 must prove the managed integration behavior required by the spike.

Applicable scenarios include:

- managed process starts/stops correctly
- isolated Cryptex configuration is used
- REST authentication failure is handled
- pairing/configuration state is observable
- pause/resume/status behavior is observable
- conflict/versioning state is visible
- user-global Syncthing configuration is not modified or collided with
- upgrade-strategy compatibility is evaluated

The exact packaging/supervision mechanism remains unresolved until the M0 proof and ADR-0004 addendum.

## 14. Offline and reconnect matrix

For milestones implementing offline-capable behavior, relevant tests must prove that no master device or central server is required for local operation.

Applicable later scenarios include:

- network unavailable
- peer unavailable
- reconnect after offline operation
- repeated disconnect/reconnect
- sleep/wake where applicable
- delayed replication

Deterministic reconciliation requirements belong to the milestones implementing canonical events, knowledge consolidation and replication.

Complete offline/reconnect soak testing belongs to M16.

## 15. Security failure matrix

Security-sensitive behavior must test the applicable negative paths.

Examples include:

- unauthorized IPC access
- invalid capability request
- untrusted content attempting privileged behavior
- renderer attempting unavailable privileged capability
- secret/log leakage checks
- invalid trust/signature data
- restricted data crossing an AI boundary where prohibited

No test may require weakening least privilege or exposing a public Cryptex API.

Purge and recovery operations must not become autonomous AI tools.

## 16. Backup and recovery matrix

When backup/recovery behavior is implemented, verification must distinguish:

- backup creation
- backup integrity
- backup verification
- restore
- reconstruction

A successful upload/write is not proof that backup is valid.

Applicable later tests include:

- interrupted backup
- damaged backup
- restore into a sandbox
- large-object handling
- recovery-key handling
- first-device trust bootstrap
- reconstruction of derived state

Full backup/restore behavior belongs to the backup/recovery milestone and later hardening.

## 17. Purge matrix

When purge is implemented, relevant failure testing must verify the architecture's limits.

Applicable scenarios include:

- stale replica attempts to restore purged content
- reachable Cryptex-controlled version history cleanup
- attachment reference handling
- purge ledger/integrity bridge behavior
- inaccessible/lost peer limitations

Tests must not claim guaranteed deletion from systems Cryptex cannot control.

## 18. UI and runtime-health verification

Runtime health must reflect actual observed state.

Where health UI is implemented, tests or reproducible runtime checks must distinguish:

- Configured
- Ready
- Active
- Verified
- Attention

Configuration presence alone must not produce a false Active or Verified state.

UI changes also require applicable visual/native-feeling review.

## 19. Doctor verification

Doctor is a product requirement, not optional debugging tooling.

As Doctor checks are added, they must report actual implemented runtime state and produce useful, privacy-safe diagnostics.

Doctor tests must avoid:

- memory-body leakage
- secret leakage
- recovery-material leakage
- credential leakage

M0 provides only the Doctor foundation required by its milestone.

Later milestones expand Doctor alongside the systems they introduce.

## 20. CI responsibilities

CI must grow with implemented behavior.

The repository CI baseline targets macOS and Windows and, as targets become available, includes relevant:

- formatting/linting
- Rust tests
- TypeScript tests
- schema generation/drift checks
- Golden Brain tests
- builds
- later packaging smoke tests

Heavy failure/recovery testing may belong to milestone verification rather than every fast local commit.

A required CI check must be green before the applicable completion gate is considered green.

## 21. Evidence

A test claim must be backed by reproducible evidence.

Evidence may include:

- automated test result
- CI job
- repository-controlled test/fixture
- benchmark output
- deterministic script
- documented manual runtime proof when automation is not yet practical

Configuration presence is not runtime proof.

A feature must not be reported as working merely because the configuration or code path exists.

## 22. M0 minimum failure/testing gate

For M0, only failure/testing requirements relevant to M0 scope are mandatory.

M0 must verify the applicable failure paths for:

- blank Desktop foundation
- Rust workspace foundation
- `cryptexd` proof-of-concept
- local IPC proof
- daemon lifecycle/update spike
- physical event-layout benchmark
- managed Syncthing spike
- Golden Brain foundation
- Doctor foundation

M0 must also satisfy the accepted M0 gate checklist and Definition of Done.

M0 is not required to complete failure tests for product features belonging to M1 and later.

## 23. M16 complete failure campaign

M16 Hardening completes the product-wide failure matrix.

Its scope includes:

- complete failure matrix
- security review
- large-scale tests
- sync stress
- backup/restore drills
- sleep/wake/offline/reconnect soak

Earlier milestones remain responsible for their own relevant failure tests; M16 is not permission to defer obvious correctness failures.

## 24. Source basis

This document consolidates existing requirements from:

- `docs/00_MASTER_SPEC.md`
- `AGENTS.md`
- `docs/18_IMPLEMENTATION_READINESS_PLAN.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/21_DEFINITION_OF_DONE.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`
- `docs/milestones/M0_FOUNDATION.md`

It does not independently introduce new architecture or move later product features into M0.
