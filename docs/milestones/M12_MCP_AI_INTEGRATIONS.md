# M12 — MCP + AI Integrations

**Status:** PROPOSED  
**Baseline:** 2026-09-28

## Purpose

M12 productizes controlled local AI integrations without making AI the memory authority or a dependency for core capture/search.

## Authority

Follow `AGENTS.md`, `docs/00_MASTER_SPEC.md`, `docs/12_MCP_AND_AI_ADAPTERS.md`, `docs/28_PRIVACY_CLASSIFICATION_AND_AI_AUTHORITY.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/21_DEFINITION_OF_DONE.md` and this contract.

## Scope

M12 includes:

- local MCP bridge
- role/capability policy
- classification filters
- Codex integration
- Cursor integration
- Claude Code integration
- Antigravity integration
- Grok Build local MCP integration
- runtime smoke/health verification

## Required invariants

- AI clients never directly mutate canonical files.
- Supported adapters do not require direct canonical-folder/generated-vault access.
- Restricted memory is not exposed to AI by default.
- AI output is untrusted input.
- Purge and recovery are not autonomous AI tools.
- Core capture/search continue to work without AI availability.
- V1 does not require a public Cryptex Internet API.

## Verification

M12 must verify relevant role/capability enforcement, classification filtering, client integration smoke tests, provider-unavailable behavior, runtime health truthfulness and privacy-safe logging.

## Completion gate

M12 is complete only when scope items and relevant security/privacy/failure tests pass, required CI is green, Definition of Done is satisfied and human review is complete.

Only then may M13 begin.
