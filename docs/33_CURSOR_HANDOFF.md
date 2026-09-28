# 33 — Cursor Implementation Handoff

## Purpose

This is the operating procedure for handing Cryptex implementation to Cursor/agents without allowing architecture drift.

## At the start of every milestone

Give the implementation agent:
1. `AGENTS.md`
2. `docs/00_MASTER_SPEC.md`
3. the milestone file under `docs/milestones/`
4. only the relevant detailed specification documents
5. accepted ADRs relevant to the work
6. `docs/21_DEFINITION_OF_DONE.md`

## Prompt contract

Use instructions equivalent to:

> Implement only milestone Mxx according to the authoritative Cryptex specification. Do not redesign architecture. If the milestone conflicts with an accepted ADR or a required detail is genuinely unresolved, stop and report the conflict. Add tests and evidence required by the milestone. Do not use real personal/Cortex data.

## Parallel work

- read-only verifier/security/test agents may run concurrently
- write-capable agents use separate branches/worktrees
- avoid parallel edits to shared schema/domain files unless explicitly coordinated

## Completion workflow

1. implementation agent produces focused changes
2. test-runner executes required suites
3. verifier checks acceptance criteria
4. security-auditor reviews security-sensitive milestones
5. UI-quality-reviewer reviews UI milestones
6. human reviews diff
7. merge only when milestone gate is green

## Architecture changes

If implementation evidence shows an accepted decision is wrong:
- do not silently change code architecture
- create/update an ADR with evidence/tradeoffs
- update Master Spec if decision changes
- then implement the newly accepted direction

## Documentation discipline

Chat messages are not the long-term project authority. Significant decisions must land in Markdown/ADR before becoming implementation assumptions.
