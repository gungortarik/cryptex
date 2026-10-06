# M0 — Foundation and P0 Spikes

**Status:** ACCEPTED
**Baseline:** 2026-09-28

## 1. Purpose

M0 establishes the minimum executable Cryptex foundation and resolves the remaining P0 implementation-sensitive architecture questions before core product milestones begin.

M0 is not a feature-expansion milestone.

No M1 or later product behavior may be implemented opportunistically during M0.

## 2. Authority

Implementation and verification for M0 must follow:

1. `AGENTS.md`
2. `docs/00_MASTER_SPEC.md`
3. accepted ADRs relevant to M0
4. `docs/18_IMPLEMENTATION_READINESS_PLAN.md`
5. `docs/20_MILESTONE_ROADMAP.md`
6. `docs/21_DEFINITION_OF_DONE.md`
7. `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
8. `docs/30_ADR_INDEX.md`
9. `docs/31_TECH_STACK_BASELINE.md`
10. `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`
11. `docs/33_CURSOR_HANDOFF.md`

If implementation evidence conflicts with an accepted architectural decision, stop and report the conflict. Do not silently redesign the architecture.

## 3. M0 scope

M0 includes only the following foundation work:

- repository/toolchain/CI foundation
- blank Tauri Desktop application on macOS and Windows
- Rust workspace build foundation
- `cryptexd` proof-of-concept
- local IPC proof-of-concept
- background runtime and atomic update lifecycle spike
- physical immutable event storage layout benchmark
- managed Syncthing integration spike
- Golden Brain fixture foundation
- Doctor foundation
- required P0 ADR decisions/addenda

Existing pre-implementation readiness work may be reused as evidence where already verified.

## 4. Non-goals

M0 must not implement M1 or later product features.

M0 does not authorize:

- production Brain/device identity
- production event store semantics beyond what is required for the storage-layout spike
- SQLite/search implementation
- knowledge consolidation
- Obsidian projection
- production replication semantics
- backup/recovery feature implementation
- AI/MCP product behavior
- broad product UI
- real Cortex or personal-memory ingestion

Synthetic data and Golden Brain fixtures must be used instead of real personal/Cortex data.

## 5. P0-1 — Physical Event Storage Layout

M0 must benchmark physical immutable event packaging at realistic scale on both macOS and Windows.

The comparison must include:

- one immutable file per event
- immutable sealed per-device segment/pack files
- hybrid thresholded segments only if evidence justifies them

The benchmark must measure the criteria required by `docs/23_REQUIRED_TECHNICAL_SPIKES.md`, including append latency, enumeration, sync behavior, rebuild performance, disk overhead, crash behavior, purge complexity and corruption blast radius.

### Acceptance

Select the simplest option that satisfies scale and recovery goals on both platforms.

Record the decision in ADR-0013 and mark it ACCEPTED before the applicable downstream milestone gate.

**Recorded 2026-10-02:** P0-1 is complete. ADR-0013 is ACCEPTED (sealed per-device segments). M0 remains IN PROGRESS until P0-2, P0-3 and P0-4 and the remaining M0 gate items are verified.

**Recorded 2026-10-05:** Owner accepted ADR-0012's scoped M0 Mode B direction:
per-user Desktop starts at login, closing the window hides the app, and explicit
Quit cleanly stops the owned daemon and exits. Owner also directed the project
to follow the Master Spec's Syncthing requirements and accepted the scoped
ADR-0004 P0-4 implementation addendum: pinned native transport, isolated managed
profile and authenticated backend-owned loopback REST. These decisions do not
accept production identity/secrets, canonical replication semantics, installed
updater/power-loss guarantees or signed distribution. M0 closeout evidence and
required final CI are recorded in `evidence/m0/m0-closure-audit-20261005.md`;
the owner approved the concrete closeout diff on 2026-10-05. The M0 foundation
gate is COMPLETE for its synthetic scope, as recorded in
`checklists/M0_FOUNDATION_GATE.md`. This does not authorize M1 or accept
production identity/secrets, canonical replication semantics, installed
updater/power-loss guarantees or signed distribution.

## 6. P0-2 — Background Runtime / Atomic Update Lifecycle

M0 must prove how `cryptexd` remains available and safely coordinated with Cryptex Desktop on both operating systems.

The spike must prove:

- expected behavior after main-window close
- reboot/login startup
- crash detection/restart
- single daemon instance
- Desktop reconnect
- compatible Desktop + daemon update behavior
- rollback/error path
- no canonical-data damage caused by lifecycle/update behavior

### Acceptance

Record the selected lifecycle/supervision approach in ADR-0012 and mark it ACCEPTED before M1/M2.

## 7. P0-3 — Local IPC Transport

M0 must prove the local IPC implementation for Cryptex clients and `cryptexd`.

The spike must prove:

- macOS local IPC behavior with safe socket path handling
- Windows named-pipe behavior
- current-user-only permissions/ACLs
- protocol version negotiation
- Desktop/CLI/MCP client-role handling
- daemon restart/reconnect behavior
- malformed/oversized request handling

### Acceptance

Record the exact implementation/library and relevant implementation details in the ADR-0007 addendum before production daemon APIs are considered complete.

## 8. P0-4 — Managed Syncthing Integration

M0 must prove that Cryptex can manage a pinned Syncthing binary as an implementation detail on both platforms.

The spike must prove:

- sidecar/process lifecycle
- isolated Cryptex Syncthing config/home
- REST API authentication
- folder/device pairing flow
- pause/resume/status behavior
- conflict/versioning visibility
- upgrade-strategy compatibility
- third-party license/notices handling
- no collision with user-global Syncthing configuration

### Acceptance

Record the chosen packaging/supervision implementation in the ADR-0004 addendum before the replication milestone.

## 9. Executable foundation outputs

M0 must produce and verify:

- repository and CI remain green
- blank Tauri Desktop launches on macOS
- blank Tauri Desktop launches on Windows
- Rust workspace builds successfully
- `cryptexd` proof-of-concept runs
- local IPC proof-of-concept works on both supported platforms
- Golden Brain fixture foundation exists
- Doctor foundation exists

The Desktop shell created in M0 is foundation-only. Broad application UI is outside M0 scope.

## 10. Cross-platform requirements

macOS and Windows are first-class M0 targets.

Platform-sensitive proofs must be verified on both operating systems where applicable.

Success on one operating system must not be treated as sufficient evidence for a cross-platform M0 requirement.

No device may become a canonical master.

Runtime data must remain outside the source repository and use development app-data roots.

## 11. Required evidence

M0 evidence must be reproducible from repository-controlled code, tests, scripts, benchmark output or documented ADR evidence.

Completion claims must be based on actual verification rather than configuration presence.

Where a spike selects an implementation direction, retain sufficient evidence to explain:

- options evaluated
- measurements or proof performed
- chosen direction
- rejected alternatives
- platform implications
- security/recovery implications where relevant

## 12. Testing and verification

M0 must follow `docs/21_DEFINITION_OF_DONE.md`.

At minimum, verification must cover all behavior introduced by M0 and all explicit spike acceptance criteria.

Required checks must include relevant:

- formatting/linting
- unit tests
- integration tests
- cross-platform tests
- failure-path tests
- security checks
- CI checks
- runtime/manual proof where automation cannot yet establish the claim

Tests must not be weakened merely to make CI green.

## 13. M0 completion gate

M0 is complete only when all of the following are verified:

- repository and required CI checks are green
- blank Desktop launches on macOS and Windows
- Rust workspace builds
- `cryptexd` proof-of-concept is verified
- local IPC proof-of-concept is verified
- Golden Brain fixture foundation is verified
- Doctor foundation is verified
- P0-1 physical event-layout benchmark is resolved
- P0-2 background runtime/update lifecycle is resolved
- P0-3 local IPC implementation is resolved
- P0-4 managed Syncthing integration is resolved
- ADR-0012 is accepted
- ADR-0013 is accepted
- ADR-0007 implementation addendum is completed
- ADR-0004 implementation addendum is completed
- any architecture changes caused by spike evidence are reflected in authoritative documentation
- human review is complete
- the M0 gate checklist is green

Only then may M1 begin.

## 14. Completion status

**Completion record (2026-10-05):** M0's applicable synthetic foundation gates
have verified evidence, the owner reviewed and approved the closeout diff, and
the completion checklist is green. M0 is COMPLETE within the scope above. M1
remains unauthorized until separately approved.
