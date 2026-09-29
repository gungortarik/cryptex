# 21 — Cryptex Definition of Done

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the minimum conditions required before Cryptex work may be called complete.

Compilation, a successful local launch, or an implementation claim alone is not sufficient evidence of completion.

This document does not replace milestone acceptance criteria. A milestone may add stricter requirements, but may not weaken the Master Spec, accepted ADRs, or this Definition of Done.

## 2. Authority

Completion must be evaluated against, in order:

1. `docs/00_MASTER_SPEC.md`
2. accepted ADRs relevant to the work
3. the current milestone contract under `docs/milestones/`
4. relevant detailed specification documents
5. this Definition of Done

If requirements conflict, implementation stops and the conflict is reported. Architecture must not be silently redesigned for implementation convenience.

## 3. Core completion rule

A feature, change, or milestone is complete only when all relevant acceptance criteria and verification categories have passed.

Depending on the work, relevant verification includes:

- unit tests
- integration tests
- cross-platform verification
- failure-path testing
- security testing/review
- recovery testing
- visual/UI review
- required CI checks

Only checks relevant to the changed behavior are required, unless the milestone contract requires broader verification.

## 4. Implementation and test requirements

For every behavior change:

- add or update relevant tests
- run relevant formatting commands
- run relevant linting commands
- run relevant test suites
- verify the milestone acceptance criteria
- do not weaken a failing test merely to make CI green unless the specification has explicitly changed
- do not silently swallow corruption, signature failures, schema incompatibility, or other integrity failures
- use typed errors and structured diagnostics where required by the implementation

A passing build without the required tests and verification is not Done.

## 5. Architectural invariants

Completion requires preserving all applicable Cryptex invariants, including:

- canonical durable memory remains immutable event data
- existing canonical events are never edited in place
- SQLite databases, search indexes and Obsidian projections remain derived and rebuildable
- `cryptexd` remains the only supported canonical writer on a device
- Desktop, CLI, MCP, Obsidian and AI clients do not directly mutate canonical memory files
- no device becomes the canonical master
- trusted desktops remain capable of offline operation and later reconciliation
- runtime data is never stored inside the source repository
- canonical behavior does not depend on one platform's path, filename, case, line-ending or symlink behavior
- no custom cryptography is introduced

A change that violates an invariant is not Done even if its tests pass.

## 6. Cross-platform completion

macOS and Windows are first-class targets.

Where behavior is platform-dependent or shared across platforms:

- relevant behavior must be verified on both supported operating systems
- canonical text remains UTF-8 with LF
- platform-specific integration belongs behind platform adapters rather than being scattered through core domain logic
- success on one operating system must not be treated as proof of cross-platform completion

Platform-specific work may be verified only on its applicable platform when the milestone contract clearly scopes it that way.

## 7. Security and privacy completion

Where security or privacy is relevant:

- imported memory, Markdown, attachment metadata and AI output are treated as untrusted input
- frontend rendering safely handles untrusted content
- renderer/webview capabilities remain least privilege
- UI code does not gain arbitrary shell or filesystem access
- secrets remain in operating-system secure storage
- logs do not contain memory bodies, secrets, recovery material or credentials by default
- restricted memory is not exposed to AI by default
- purge and recovery operations are not exposed as autonomous AI tools

Security-sensitive milestones require security review before completion.

## 8. Recovery completion

Recovery claims require recovery evidence.

A backup is not considered verified merely because data was written or uploaded successfully. Required restore or reconstruction behavior must also be tested.

Recovery, corruption and failure scenarios required by the milestone must pass before the work is Done.

## 9. UI completion

Where UI is changed:

- functionality must work
- runtime health states must represent actual runtime behavior
- platform conventions must be respected on macOS and Windows
- the result must follow the Cryptex design language
- the application must not regress into a generic AI-generated web-dashboard appearance
- required visual review must pass before completion

UI polish is part of product quality, not an optional post-completion step.

## 10. Documentation and architecture changes

Significant implementation decisions must be recorded in repository Markdown or ADRs rather than existing only in chat history.

If implementation evidence shows that an accepted architectural decision is wrong:

1. do not silently change the architecture in code
2. create or update the relevant ADR with evidence and tradeoffs
3. update the Master Spec if the accepted architecture changes
4. only then implement the newly accepted direction

Documentation required by the milestone must be updated before the milestone is Done.

## 11. Completion workflow

Before merge or milestone completion:

1. the implementation agent produces focused changes
2. required test suites are executed
3. acceptance criteria are verified against actual repository/runtime state
4. security review is performed when security-sensitive
5. UI quality review is performed when UI is affected
6. the human reviewer reviews the diff
7. required CI checks are green
8. the milestone gate is green

A feature or milestone must not be reported as working or complete until the required checks have actually been verified.

## 12. Milestone discipline

Work remains inside the currently selected milestone.

Later milestones must not be implemented opportunistically.

A milestone is complete only after:

- its explicit acceptance criteria pass
- its required tests pass
- its required platform verification passes
- its required technical spikes and ADR gates are resolved
- its completion gate is green

Only then may implementation proceed to the next milestone.

## 13. Source basis

This Definition of Done consolidates existing requirements from:

- `docs/00_MASTER_SPEC.md`
- `AGENTS.md`
- `docs/18_IMPLEMENTATION_READINESS_PLAN.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`
- `docs/33_CURSOR_HANDOFF.md`

It does not independently redesign or extend Cryptex architecture.
