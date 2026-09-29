# 12 — Cryptex MCP and AI Adapters

**Status:** PROPOSED  
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the accepted Cryptex AI-integration boundary.

AI is a controlled client of Cryptex, not the canonical memory authority.

This document does not require AI for capture, search or core local operation and does not authorize direct canonical-file access.

## 2. Supported V1 integration targets

Supported V1 local integration targets are:

- Codex
- Cursor
- Claude Code
- Antigravity
- Grok Build through local stdio MCP

The exact adapter implementation for each target belongs to M12.

## 3. Local integration model

Supported AI clients use local MCP/adapters.

They do not require direct access to:

- canonical event folders
- canonical attachment storage
- the generated Obsidian vault path

Cryptex remains the enforcement boundary for classification, authorization and canonical mutation.

## 4. No public API requirement

Cryptex V1 does not require a public Internet API.

The audited plan distinguishes Grok surfaces:

- Grok Build supports local stdio MCP and can integrate locally
- Grok web Custom Connectors require a publicly reachable MCP URL/tunnel and are not a required V1 integration path

Cryptex must not add a public endpoint merely to support an optional web connector.

## 5. Canonical writer boundary

AI clients never write canonical files directly.

Any canonical change requested by an AI client flows through Cryptex-controlled daemon/IPC boundaries with applicable validation and authorization.

AI-generated content is untrusted input until accepted through those boundaries.

## 6. Role and capability policy

M12 includes role/capability policy for AI/MCP access.

A connected client must not gain unrestricted capabilities merely because it can reach a local MCP/IPC endpoint.

Capabilities must remain narrowly scoped to the accepted integration contract.

The exact production capability matrix belongs to M12 and related security specifications.

## 7. Classification filters

Cryptex classification values include:

- `public`
- `internal`
- `private`
- `restricted`

Classification filters are enforced by Cryptex.

Restricted memory is not exposed to AI by default.

An adapter must not weaken classification because a provider or tool supports broader filesystem access.

## 8. Raw-vault privacy boundary

The audit identifies arbitrary user-granted filesystem access as outside Cryptex classification enforcement.

Supported Cryptex adapters therefore use the controlled MCP path rather than requiring raw-vault or canonical-folder access.

Cryptex must not claim to enforce classification on unrelated external filesystem access that the user grants independently of Cryptex.

## 9. Protected namespaces and memory poisoning

Untrusted documents and AI output must not silently install:

- device trust
- security policy
- global system rules
- protected namespace values

Security/system namespaces remain daemon-authoritative.

Authority tiers/protected namespaces must prevent memory poisoning from becoming system policy.

## 10. Capture independence

Quick Capture is durable without an LLM.

Raw Inbox capture is written first; semantic extraction may occur later.

A provider outage must not prevent durable capture.

## 11. Search independence

Core V1 search uses local SQLite FTS5 plus metadata, relationships and filters.

It remains useful offline with no LLM.

Semantic/vector search is optional post-V1 and must not become an AI dependency for basic retrieval.

## 12. Purge and recovery prohibition

Purge and recovery operations are never exposed as autonomous AI tools.

An AI integration must not independently:

- purge memory
- perform disaster trust bootstrap
- manipulate Recovery Kit secrets
- override explicit destructive confirmation

Any future assisted workflow must preserve human/daemon authority required by the applicable specification.

## 13. Secret and diagnostic handling

AI adapter logs and diagnostics must not expose by default:

- memory bodies beyond the authorized response
- credentials
- private keys
- Recovery Kit material
- authentication tokens

Diagnostics follow the local/redacted/opt-in-bundle policy.

## 14. Runtime health

M12 includes runtime smoke/health verification for AI integrations.

Health states must distinguish configuration from actual runtime behavior.

Configured must not be represented as Active or Verified without evidence.

Doctor/AI integration surfaces must consume truthful runtime checks.

## 15. Offline behavior

Local adapters must fit Cryptex's local-first architecture.

Cryptex core operation must not depend on an AI provider being available.

Provider-dependent AI features may report unavailable/attention states without blocking local memory capture, canonical access or core search.

## 16. M0 boundary

M0 only proves local IPC roles required by P0-3, including the MCP client role.

M0 does not productize M12 AI behavior, classification filters or provider adapters.

Proof code must not expand M0 scope into full AI integration.

## 17. M12 scope

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

M12 must follow the security/privacy and Definition of Done requirements.

## 18. Explicitly unresolved

This document does not select:

- final MCP tool names
- final request/response schemas
- final per-client capability matrix
- provider-specific UX
- token/context budgets
- optional post-V1 vector/semantic search provider

These belong to M12 implementation evidence and later accepted specifications.

## 19. Source basis

This document consolidates existing AI-integration requirements from:

- `docs/00_MASTER_SPEC.md`
- `AGENTS.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`

It does not independently add public endpoints or AI authority.
