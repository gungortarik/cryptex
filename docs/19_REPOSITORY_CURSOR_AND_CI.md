# 19 — Cryptex Repository, Cursor and CI Contract

**Status:** ACCEPTED
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the repository, coding-agent, editor and continuous-integration contract for Cryptex.

The repository must remain:

- deterministic
- reviewable
- cross-platform
- free of runtime/personal data
- governed by written specifications
- continuously verifiable on macOS and Windows

Cursor and other coding agents implement the written architecture. They do not invent or silently redesign it.

## 2. Authority

Repository and implementation work must follow:

1. `AGENTS.md`
2. `docs/00_MASTER_SPEC.md`
3. accepted ADRs
4. the active milestone contract
5. relevant detailed specifications
6. `docs/21_DEFINITION_OF_DONE.md`

If requirements conflict, implementation stops and the conflict is reported.

Implementation convenience is not authority to change architecture.

## 3. Repository model

Cryptex uses one private GitHub monorepo:

`gungortarik/cryptex`

The repository contains source code, tests, synthetic fixtures, schemas, documentation and repository-controlled development configuration.

It is not a Cryptex runtime-data location.

`main` must remain buildable and tested according to the checks available for the currently implemented milestone.

## 4. Source checkout versus runtime data

Developer source checkouts are separate from Cryptex runtime data.

Typical development checkouts are:

- macOS: `~/github_repos/cryptex`
- Windows: local developer checkout such as `D:\github_repos\cryptex`

The exact source checkout location is not canonical product state.

Production runtime roots are outside the repository.

Development runtime uses `Cryptex-Dev` app-data roots.

The source tree must never become a runtime brain or application-data directory.

## 5. Repository safety

The repository must never contain:

- real personal brains
- real Cortex memory
- secrets
- credentials
- Recovery Kits
- private runtime logs
- production runtime databases

Synthetic Golden Brain and test fixtures may be repository-controlled.

Generated, temporary and runtime artifacts must be ignored where appropriate.

## 6. Git text and portability policy

Repository-controlled source text uses normalized LF behavior where applicable.

Canonical text is UTF-8 with LF.

`.gitattributes` and repository conventions must prevent accidental platform-specific line-ending drift.

Git policy must not introduce canonical path, case, filename or symlink assumptions that work on only one supported operating system.

Binary files must not be treated as normalized source text.

## 7. Change discipline

Prefer small, reviewable commits.

Do not mix unrelated architectural refactors into milestone work.

Significant milestone implementation should use feature branches and PR-style review even for a solo project where practical.

The currently selected milestone defines scope.

Later milestone features must not be implemented opportunistically.

## 8. Parallel agent/write discipline

Read-only analysis or verification agents may operate in parallel.

Multiple write-capable implementation agents must not edit the same working tree concurrently.

Separate Git branches/worktrees must be used for concurrent write-capable implementation work.

Human review remains part of the completion workflow.

## 9. Repository-controlled agent instructions

Repository-controlled coding instructions include:

- root `AGENTS.md`
- `.cursor/rules/`
- `.cursor/agents/`

These files are part of the implementation control surface and must remain reviewable in Git.

Chat history is not a replacement for repository-controlled architecture or milestone instructions.

## 10. Cursor rules

`.cursor/rules/` contains focused repository-controlled instructions for areas such as:

- architecture
- Rust/core implementation
- security
- testing
- UI when applicable

Rules must reinforce the authoritative specification hierarchy.

They must not redefine architecture independently of the specification/ADR process.

## 11. Cursor agents

Initial repository-controlled agent roles include:

- verifier
- security-auditor
- test-runner

A UI-quality-reviewer is added/used when UI work begins.

Agent definitions are helpers for implementation and verification.

They do not replace:

- the Master Spec
- accepted ADRs
- milestone acceptance criteria
- human review
- CI evidence

## 12. Verifier role

The verifier checks claims against actual repository/runtime state.

Verification must distinguish:

- configured
- implemented
- running
- tested
- verified

A feature must not be reported as working merely because configuration or code exists.

The verifier must evaluate applicable milestone acceptance criteria against evidence.

## 13. Security-auditor role

Security-sensitive work requires focused review where applicable.

The security auditor checks for issues such as:

- privilege expansion
- unsafe renderer/webview capabilities
- secret leakage
- logging leakage
- unauthorized IPC behavior
- direct canonical-file access
- trust-boundary violations

Security review does not authorize architectural redesign.

## 14. Test-runner role

The test-runner executes the relevant repository-controlled verification for the active change.

Depending on implemented scope, this may include:

- formatting
- linting
- Rust tests
- TypeScript tests
- integration tests
- failure tests
- Golden Brain
- cross-platform checks

Tests must not be weakened merely to make CI green unless the specification has explicitly changed.

## 15. Editor extensions

Editor/tooling extensions must remain minimal.

Useful development support includes:

- rust-analyzer
- TOML support
- Tauri tooling where useful
- TypeScript/React support
- trusted SQLite viewer where needed

Tailwind IntelliSense is relevant only if Tailwind is selected later.

Optional convenience extensions must not become repository/runtime dependencies.

Avoid unnecessary AI/plugin sprawl.

## 16. Toolchain pinning

Repository-controlled files and lockfiles pin the development baseline required for deterministic builds.

Pinned areas include:

- Rust toolchain
- Node baseline
- pnpm exact version
- Tauri/framework dependencies
- Cargo lockfile
- pnpm lockfile

Accepted exact versions are governed by `docs/31_TECH_STACK_BASELINE.md`.

Dependencies must not be upgraded opportunistically outside the appropriate validation/acceptance process.

## 17. Formatting and linting

Rust uses:

- `rustfmt`
- `clippy`

TypeScript/web formatting/linting tooling must follow the accepted baseline.

Biome remains a candidate whose exact version is pinned after M0 scaffold validation, as defined by the readiness plan.

Do not invent a premature formatter/linter baseline merely to fill configuration space.

## 18. Continuous integration

Cryptex CI uses GitHub Actions with macOS and Windows coverage.

CI must grow with the implemented product.

As relevant targets exist, CI includes:

- formatting/lint
- Rust tests
- TypeScript tests
- schema generation/drift checks
- Golden Brain tests
- builds
- later packaging smoke builds

A check must only claim to verify behavior it actually exercises.

Configuration presence is not runtime proof.

## 19. Cross-platform CI rule

macOS and Windows are first-class targets.

A shared change is not considered cross-platform verified merely because one CI runner passes.

Platform-specific failure must be investigated rather than ignored because the other operating system succeeds.

Runtime/manual verification remains required where CI cannot establish the product claim.

## 20. Schema drift

When schema generation exists, Rust domain types are the source used to generate the required JSON Schema/TypeScript bindings according to the accepted architecture.

CI must detect generated-schema/binding drift once that system is implemented.

M0 must not implement later schema/domain behavior opportunistically merely to activate this check early.

## 21. Golden Brain

Golden Brain is synthetic test data.

CI may use Golden Brain as relevant systems become implemented.

Golden Brain must never contain real personal/Cortex memory.

Its tests grow alongside milestone behavior.

The presence of the fixture directory alone is not evidence that a product feature works.

## 22. Dependency and supply-chain checks

Lockfiles are required.

Dependency/security auditing and third-party notice checks must be added before beta as required by the readiness/audit plan.

Syncthing third-party licensing/notices must be handled when its managed packaging is implemented.

Do not claim supply-chain verification before the relevant checks actually exist and run.

## 23. CI speed versus deep verification

Fast checks may run locally or pre-commit where useful.

Heavy failure, recovery, soak and large-scale suites may run in milestone verification or appropriate CI workflows rather than every fast developer loop.

Moving a heavy test out of the fast loop does not remove it from Definition of Done when the milestone requires it.

## 24. Completion workflow

Before work is reported complete:

1. implementation stays within milestone scope
2. relevant formatting/linting runs
3. relevant tests run
4. verifier checks acceptance criteria against actual state
5. security review occurs when applicable
6. UI quality review occurs when applicable
7. human diff/review occurs
8. required CI checks are green
9. milestone gate is green

Compilation alone is not completion.

## 25. M0 repository/CI boundary

M0 includes repository/toolchain/CI foundation.

Existing verified pre-implementation repository work may be reused as evidence.

M0 may extend CI only as required by the targets introduced during M0.

M0 must not opportunistically implement later product features simply to populate future CI checks.

By the M0 gate, repository and required CI checks must be green for the actual implemented M0 scope.

## 26. M0 Cursor boundary

M0 coding agents must receive the repository-controlled authority and milestone context before implementation.

At minimum this includes:

- `AGENTS.md`
- `docs/00_MASTER_SPEC.md`
- `docs/milestones/M0_FOUNDATION.md`
- relevant detailed specs
- relevant ADR/index status
- `docs/21_DEFINITION_OF_DONE.md`

Agents must not rely on chat memory alone to reconstruct architecture.

## 27. Explicit non-goals

This document does not:

- select unresolved M0 P0 architecture
- authorize M1+ feature implementation
- require cloud build/runtime services beyond GitHub repository CI
- make Cursor an architectural authority
- make CI success sufficient evidence for runtime behavior CI does not test
- place runtime data in the repository
- replace human review

## 28. Source basis

This document consolidates existing repository/Cursor/CI requirements from:

- `AGENTS.md`
- `docs/00_MASTER_SPEC.md`
- `docs/18_IMPLEMENTATION_READINESS_PLAN.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/21_DEFINITION_OF_DONE.md`
- `docs/31_TECH_STACK_BASELINE.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`
- `docs/33_CURSOR_HANDOFF.md`
- `docs/milestones/M0_FOUNDATION.md`

It does not independently redesign Cryptex architecture or development workflow.
