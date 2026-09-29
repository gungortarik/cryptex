# 24 — Cryptex Operations, Doctor and Diagnostics

**Status:** ACCEPTED
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the operational-health, Doctor and diagnostics boundaries for Cryptex.

Doctor is a product requirement, not optional developer tooling.

The purpose is to ensure that Cryptex can distinguish what is merely configured from what is actually working, while producing useful diagnostics without leaking memory, secrets or recovery material.

M0 provides only the Doctor foundation required by the Foundation milestone.

Doctor expands alongside later product systems as those systems become implemented.

## 2. Authority and scope

This document must remain consistent with:

1. `docs/00_MASTER_SPEC.md`
2. accepted ADRs relevant to runtime/operations
3. `docs/11_DAEMON_IPC_RUNTIME.md`
4. `docs/15_PLATFORM_MACOS_WINDOWS.md`
5. `docs/17_TEST_AND_FAILURE_MATRIX.md`
6. `docs/20_MILESTONE_ROADMAP.md`
7. `docs/21_DEFINITION_OF_DONE.md`
8. `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`
9. `docs/milestones/M0_FOUNDATION.md`
10. `checklists/M0_FOUNDATION_GATE.md`

Doctor and diagnostics must not silently redefine product architecture.

## 3. Doctor is part of the product

Doctor is not merely a debug script.

Cryptex requires operational diagnostics because the product depends on several local systems that may be:

- configured but unavailable
- installed but not running
- running but unhealthy
- functioning but not yet verified
- in an attention state

Doctor must report observed state rather than optimistic configuration assumptions.

A feature or integration must not be described as working solely because its configuration exists.

## 4. Runtime health vocabulary

Cryptex distinguishes these runtime-health states:

- `Configured`
- `Ready`
- `Active`
- `Verified`
- `Attention`

These states must remain distinct.

The application must not collapse configuration presence into a stronger runtime claim.

For example, a configured integration must not automatically be represented as Active or Verified without supporting runtime evidence.

Exact subsystem-specific mapping of these states may be refined by the milestone that implements that subsystem.

This document does not invent a universal state machine beyond the distinctions fixed by the Master Spec and audit.

## 5. Evidence-based health

Health claims must be grounded in actual observations available for the implemented subsystem.

Evidence may come from applicable:

- local runtime probes
- daemon responses
- IPC checks
- filesystem checks
- process checks
- integration status
- verification/rebuild results
- repository-controlled tests
- deterministic diagnostic commands

Configuration presence alone is not sufficient when a stronger runtime state is claimed.

A check must not claim to verify behavior that it does not actually exercise.

## 6. M0 Doctor foundation

M0 must establish a Doctor foundation.

The M0 Doctor scope is limited to systems actually introduced or proven during M0.

It must not pretend that M1+ features already exist.

At the M0 gate:

- a Doctor foundation must exist
- its implemented checks must reflect actual observed state
- configuration-only checks must not be reported as runtime verification
- diagnostics must follow the privacy requirements in this document
- relevant Doctor behavior must be testable or reproducibly verifiable

The presence of a Doctor command/module alone is not sufficient evidence.

## 7. M0 scope boundaries

M0 Doctor may cover the foundation needed for M0 systems such as:

- executable/runtime foundation
- `cryptexd` proof-of-concept
- local IPC proof
- managed Syncthing proof
- local development/runtime environment where relevant

Only checks backed by actual M0 implementation may be reported as functioning.

M0 must not implement broad M1+ product health logic merely to populate Doctor output.

## 8. Growth with later milestones

Doctor grows alongside the systems introduced by later milestones.

Examples of later areas that may become diagnosable when implemented include:

- Brain/device identity
- canonical event integrity
- derived database/rebuild state
- search
- replication
- attachments
- backup/recovery
- AI/MCP integrations
- Obsidian projection
- platform/update behavior

Their exact Doctor checks belong to their applicable milestone contracts.

A future diagnostic category must not be treated as an M0 implementation requirement simply because it appears in this document.

## 9. Diagnostics privacy

Diagnostics must be privacy-preserving by default.

The audited architecture requires:

- local-only diagnostics/logging by default
- redacted diagnostics
- opt-in diagnostic bundle creation

Logs and diagnostics must not include by default:

- memory bodies
- secrets
- credentials
- Recovery Kit material
- private keys
- authentication tokens

Diagnostics must remain useful without turning operational tooling into an alternate memory-export path.

## 10. Diagnostic bundles

Diagnostic bundles are opt-in.

Creating a diagnostic bundle must not imply automatic upload or cloud transmission.

The bundle must be suitable for troubleshooting while respecting Cryptex privacy boundaries.

Where applicable, bundle generation must redact or exclude:

- memory content
- credentials
- secrets
- recovery material
- unnecessary personally sensitive runtime data

The exact bundle container format, retention period and future support/export workflow are not selected by this document.

## 11. Logs

Operational logs must support diagnosis without becoming a second canonical memory store.

Logs are non-canonical runtime data.

They must not be required for reconstruction of a Brain.

Logging must avoid memory bodies, secrets, credentials and recovery material by default.

The exact logging framework, rotation policy and retention limits are implementation details governed by the applicable milestone and later operational evidence.

## 12. Diagnostics cache

Diagnostics cache/state is derived and disposable.

Loss of diagnostics cache must not damage canonical memory.

Diagnostics cache must not become required recovery state.

If diagnostics state becomes corrupt or unavailable, Cryptex must remain able to reason about canonical memory independently of that cache.

## 13. Error behavior

Cryptex must not silently swallow integrity-relevant or runtime failures.

Applicable failures must be explicit and diagnosable.

Project rules require typed errors and structured diagnostics where appropriate.

Examples include implemented failures involving:

- daemon startup
- duplicate daemon instance
- IPC authorization
- incompatible protocol version
- malformed requests
- reconnect behavior
- managed integration process failure
- integrity/corruption failures in later milestones

Doctor may surface these conditions, but it must not convert a failure into a false healthy state.

## 14. Doctor and canonical write boundary

Doctor must respect the canonical writer architecture.

Doctor must not create an alternate direct mutation path to canonical files.

If a future repair operation requires canonical mutation, it must follow:

- `cryptexd` writer authority
- applicable authorization
- applicable milestone requirements
- applicable tests and recovery guarantees

This document does not authorize autonomous repair of canonical memory.

## 15. AI boundary

Doctor and diagnostics must not become a route for bypassing Cryptex AI-access policy.

Restricted memory remains excluded from AI access by default.

Purge and recovery operations must not be exposed as autonomous AI tools.

Diagnostic output exposed to an AI integration must follow the same privacy/classification boundaries required by that integration's milestone.

M0 does not implement that later AI integration behavior.

## 16. Platform requirements

macOS and Windows are first-class operational targets.

Where a Doctor check covers shared platform-sensitive behavior, the check or its verification must account for the relevant differences between both platforms.

Platform-specific implementations may differ.

Product-level health semantics must remain equivalent.

No platform may be treated as the canonical/master operating environment.

## 17. Runtime data placement

Doctor logs, diagnostics caches and generated diagnostic material are runtime data.

They must remain outside the source repository.

Production app-managed runtime roots are:

- macOS: `~/Library/Application Support/Cryptex/`
- Windows: `%LOCALAPPDATA%\Cryptex\`

Development uses corresponding `Cryptex-Dev` roots.

This document does not freeze the final internal diagnostics directory layout.

## 18. Repository safety

The repository may contain:

- diagnostic code
- tests
- synthetic diagnostic fixtures
- documentation

The repository must not contain:

- private runtime logs
- real user diagnostic bundles
- real personal memory
- secrets
- credentials
- Recovery Kits

Any committed diagnostic fixture must be synthetic and privacy-safe.

## 19. Offline operation

Doctor must remain compatible with Cryptex's local-first architecture.

Core Doctor behavior must not require a public Internet endpoint or cloud service merely to inspect local Cryptex health.

A local subsystem may of course report that an external dependency relevant to that subsystem is unavailable, but Doctor itself must not introduce a cloud dependency into the canonical/local runtime architecture.

## 20. Failure transparency

Doctor must distinguish inability to perform a check from a successful check.

An unavailable, unsupported or failed check must not silently become a healthy result.

Likewise, absence of evidence must not be promoted to Verified.

The exact presentation for unavailable or unsupported checks may be refined later, but the health claim must remain truthful.

## 21. Doctor versus monitoring

Cryptex Doctor is local operational diagnostics.

This document does not introduce:

- cloud telemetry
- centralized fleet monitoring
- remote administration
- public health endpoints
- mandatory analytics

Any future telemetry or support workflow would require explicit architectural approval and privacy review.

## 22. Testing

Doctor behavior introduced by a milestone must follow the Definition of Done.

Applicable tests or reproducible verification should cover:

- truthful state reporting
- negative/failure states
- privacy/redaction boundaries
- cross-platform behavior where relevant
- absence of secret/memory leakage
- behavior when an inspected subsystem is unavailable

A configured-but-broken subsystem must not produce a false Verified result.

## 23. M0 acceptance boundary

For M0, Doctor is complete only to the extent required by the Foundation milestone.

Required M0 evidence includes:

- Doctor foundation exists
- implemented checks use actual observed M0 state
- configuration presence alone is not represented as runtime verification
- diagnostic output follows privacy boundaries
- relevant Doctor checks are reproducible/testable
- no M1+ Doctor functionality is falsely reported as implemented

Broader Doctor/Settings product UI belongs to later milestones.

## 24. M13 relationship

The full Desktop product UI includes Doctor/Settings.

M13 productizes the broad user-facing Doctor/Settings experience.

M0 only establishes the operational foundation.

Later UI productization must consume real health evidence rather than creating decorative or optimistic health states.

## 25. M16 relationship

M16 Hardening expands system-wide failure and operational verification.

Doctor may gain additional checks as hardening identifies meaningful operational failure modes.

M16 does not excuse earlier milestones from testing the Doctor behavior they introduce.

## 26. Explicitly unresolved

This document does not select:

- exact diagnostics/logging library
- exact structured diagnostic schema
- final Doctor command/API surface
- final Desktop Doctor UI
- log retention duration
- log rotation mechanism
- diagnostic bundle container format
- diagnostic bundle export/support workflow
- complete subsystem-by-subsystem health-state mapping

These details must be decided only when required by the relevant implementation milestone and evidence.

## 27. Source basis

This document consolidates existing operational/diagnostic requirements from:

- `docs/00_MASTER_SPEC.md`
- `AGENTS.md`
- `docs/11_DAEMON_IPC_RUNTIME.md`
- `docs/15_PLATFORM_MACOS_WINDOWS.md`
- `docs/17_TEST_AND_FAILURE_MATRIX.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/21_DEFINITION_OF_DONE.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`
- `docs/milestones/M0_FOUNDATION.md`
- `checklists/M0_FOUNDATION_GATE.md`

It does not independently add telemetry, cloud monitoring or later-milestone product behavior.
