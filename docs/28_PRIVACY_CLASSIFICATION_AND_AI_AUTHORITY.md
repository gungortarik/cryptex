# 28 — Cryptex Privacy, Classification and AI Authority

**Status:** PROPOSED  
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the accepted privacy/classification and authority boundaries for Cryptex memory and AI integrations.

It prevents classification from becoming a presentation-only label and prevents untrusted content or AI output from acquiring system authority.

## 2. Classification model

Cryptex classification values are:

- `public`
- `internal`
- `private`
- `restricted`

Classification semantics must be preserved across derived views, search and supported AI access boundaries.

The exact storage-field representation is not selected by this document.

## 3. Restricted memory

Restricted memory is not exposed to AI by default.

Supported Cryptex adapters must enforce classification filters at the Cryptex boundary.

A derived projection or convenience API must not silently weaken the classification assigned to underlying memory.

## 4. Supported AI path

Supported V1 AI clients use controlled local MCP/adapters.

They do not require direct access to canonical event folders or the generated Obsidian vault path.

Classification enforcement belongs to Cryptex, not to assumptions about provider behavior.

## 5. Raw-filesystem limitation

The audit explicitly identifies arbitrary user-granted filesystem access as outside Cryptex classification enforcement.

If a user independently grants an external tool raw filesystem access, Cryptex cannot honestly claim to mediate all content that tool can read.

Supported integrations therefore use the controlled Cryptex adapter path.

## 6. AI output is untrusted input

AI output is treated as untrusted input.

It must not silently become:

- trusted canonical memory
- device trust
- global security policy
- protected system rules
- Recovery Kit configuration
- unrestricted write authority

Canonical changes still require Cryptex-controlled validation/authorization.

## 7. Authority tiers and protected namespaces

The audited architecture requires authority tiers/protected namespaces to prevent memory poisoning.

Security/system namespaces are daemon-authoritative.

Ordinary imported content, inferred facts or AI suggestions must not be able to install or replace protected authority state merely by expressing it as text.

The exact namespace/authority data model is implemented by the applicable security/knowledge milestones.

## 8. Canonical writer boundary

AI clients, Desktop, CLI and Obsidian do not write canonical files directly.

`cryptexd` remains the supported canonical writer and enforcement point on a device.

Privacy/classification enforcement must not be bypassed by introducing a second write path.

## 9. Capture and search independence

Core durable capture does not depend on an AI provider.

Core V1 search remains useful offline with no LLM.

Privacy policy therefore must not force the user to send memory to an AI provider merely to capture or retrieve it.

## 10. Forget interaction

Forgotten content is removed from normal retrieval and AI context while remaining reversible.

Forget must therefore be reflected in AI-access filtering according to the accepted lifecycle implementation.

Forgotten is not equivalent to physically purged.

## 11. Purge and recovery authority

Purge and recovery operations are never exposed as autonomous AI tools.

AI must not independently:

- purge memory
- authorize the first recovered trusted device
- use Recovery Kit secret material
- bypass destructive confirmation

Human/daemon-controlled authority remains required.

## 12. Secrets and recovery material

Secrets, credentials, private keys and Recovery Kit material are not normal memory content for AI exposure.

Logs and diagnostics must not include such material by default.

Any future workflow involving sensitive recovery material requires explicit dedicated security design.

## 13. Provenance and inferred knowledge

Derived/inferred knowledge should preserve provenance and confidence semantics required by the memory model.

An `inferred` fact must not silently become equivalent to trusted system policy.

Confidence and classification are separate concerns: high confidence does not grant higher security authority.

## 14. External providers

Provider-specific capabilities must not redefine Cryptex privacy semantics.

Cryptex must distinguish supported local integration behavior from optional external product surfaces that require different networking/access models.

In particular, Grok Build local stdio MCP is a supported local direction; a web Custom Connector requiring public reachability is not a required V1 path.

## 15. Diagnostics

Operational diagnostics are local/redacted by default.

Diagnostic bundles are opt-in.

Privacy-sensitive content must not be included merely because a subsystem is being debugged.

## 16. Cross-platform requirement

Privacy/classification guarantees apply equally on macOS and Windows.

Platform-specific implementation details may differ, but neither platform may receive a weaker classification or authority model for convenience.

## 17. M12 boundary

M12 productizes:

- local MCP bridge
- role/capability policy
- classification filters
- supported client integrations
- runtime smoke/health verification

M12 must prove that supported adapters respect these privacy/authority boundaries.

## 18. M0 boundary

M0 only proves enough client-role/IPC behavior for the P0 transport spike.

It does not implement the full AI classification/authority system.

M0 proof code must not expose broad AI authority merely to simplify testing.

## 19. Explicitly unresolved

This document does not select:

- exact classification inheritance rules
- exact protected namespace list
- final role/capability matrix
- final MCP tool schema
- provider-specific prompt/context policies
- post-V1 semantic/vector search privacy policy

These require their applicable milestone/specification evidence.

## 20. Source basis

This document consolidates existing privacy/AI-authority requirements from:

- `docs/00_MASTER_SPEC.md`
- `AGENTS.md`
- `docs/12_MCP_AND_AI_ADAPTERS.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`

It does not independently grant AI authority or weaken classification.
