# 02 — Cryptex V1 Scope and Non-Goals

**Status:** PROPOSED  
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the current Cryptex V1 product boundary already established by the Master Spec, Roadmap and audited planning set.

It prevents implementation from silently expanding V1 scope or introducing dependencies that contradict the local-first architecture.

It does not reorder milestones or independently introduce new product commitments.

## 2. V1 product scope

Cryptex V1 is a personal, local-first, cross-platform memory and knowledge system for one user across trusted macOS and Windows devices.

The planned V1 product includes, through the milestone roadmap:

- durable structured memory
- immutable canonical event history
- local derived search/indexing
- entity/fact/relation knowledge views
- offline capture and later reconciliation
- trusted-device replication
- an Obsidian-compatible derived knowledge projection
- attachment storage and integrity
- Forget and Purge semantics
- encrypted filesystem-target backup and disaster restore
- controlled local AI/MCP integrations
- a premium macOS/Windows desktop product
- operational Doctor/diagnostics
- platform packaging/update behavior
- controlled Cortex fresh re-ingestion
- hardening and private-beta/release-candidate gates

Milestone order and completion gates remain governed by `docs/20_MILESTONE_ROADMAP.md`.

## 3. Local-first boundary

V1 does not require a primary server, primary desktop or cloud database for normal local operation.

Trusted desktops operate locally and reconcile later.

No device is the canonical master.

An always-on machine may be operationally useful for replication or scheduled backup, but it remains a peer.

## 4. Canonical and derived boundary

V1 canonical durable data consists only of the categories explicitly defined as canonical by the Master Spec.

SQLite, FTS indexes, materialized knowledge state, caches, generated Obsidian views and diagnostics cache remain derived and rebuildable.

A derived representation must not become canonical merely because it is convenient for implementation.

## 5. Search scope

V1 core search uses SQLite FTS5 plus metadata, relationships and filters.

Core search must remain useful offline and without an LLM.

Semantic/vector search is optional post-V1 and must not become a prerequisite for V1 search correctness.

## 6. Capture scope

V1 durable capture must not depend on AI availability.

Quick Capture writes durable Inbox data first.

Semantic extraction may happen later.

Provider outage must not prevent memory capture.

## 7. Replication scope

Syncthing is the accepted V1 transport direction.

It transports canonical files/blobs between trusted peers but does not define Cryptex semantics.

Cryptex remains responsible for trust, validation, integrity, knowledge consolidation, purge behavior and recovery.

The exact managed Syncthing packaging/supervision is resolved through the required M0 spike and ADR addendum.

## 8. Obsidian scope

V1 can materialize a complete local Obsidian-compatible vault from canonical memory.

The generated vault is derived and is not the replication source of truth.

V1 uses Obsidian Graph for full graph visualization.

Cryptex may provide compact relationship summaries, but V1 does not require rebuilding a separate full graph-visualization engine.

Controlled write-back belongs to its dedicated milestone and must pass the required feature-specific spike before productization.

## 9. Backup and recovery scope

V1 backup is an encrypted filesystem-target abstraction.

Supported target examples may include:

- a Google Drive for Desktop folder
- a USB drive
- another local filesystem path

Cryptex V1 does not require Google OAuth or a direct Google Drive API integration.

One explicitly designated device performs scheduled backup/retention in V1 to avoid multi-writer pruning races.

Recovery must restore both encrypted content and trust/bootstrap capability.

Backup is not considered verified merely because data was written or uploaded; restore/reconstruction must be tested.

## 10. AI integration scope

Supported V1 local integration targets include:

- Codex
- Cursor
- Claude Code
- Antigravity
- Grok Build through local stdio MCP

Supported AI integrations use controlled local MCP/adapters.

They do not require direct access to canonical event folders or the generated Obsidian vault path.

Classification filters remain enforced by Cryptex.

Restricted memory is not exposed to AI by default.

Purge and recovery operations are not autonomous AI tools.

## 11. Desktop scope

Cryptex Desktop is a premium desktop system utility and knowledge application.

Planned product areas include:

- Health
- Memory
- Timeline
- Search
- Devices
- AI/Integrations
- Backup/Recovery
- Doctor/Settings

Tauri is the cross-platform shell.

“Native” means native-feeling behavior and platform conventions, not a requirement to use literal SwiftUI or WinUI controls.

Broad release UI quality belongs to the dedicated Desktop Product UI milestone and later platform polish.

## 12. Cortex transition scope

Cortex is not directly migrated as a runtime/system architecture.

It becomes a read-only legacy source for controlled fresh re-ingestion into a new Cryptex Brain.

Legacy structure and runtime machinery are not inherited automatically.

Real personal Brain onboarding occurs only after backup/recovery confidence at the release-candidate stage.

## 13. Explicit V1 non-goals

Cryptex V1 is not:

- a chat application
- a cloud database
- a team knowledge base
- a raw conversation archive
- a public Internet API service
- a system requiring a master desktop
- a system requiring VPS/Tailscale/cloud DB infrastructure
- an AI-dependent capture or search product
- a replacement full graph-visualization engine for Obsidian Graph
- a direct migration of Cortex runtime architecture
- a platform that gives AI direct canonical-file mutation access
- a platform that guarantees deletion from inaccessible lost devices or provider-controlled historical versions outside Cryptex control

## 14. Extraction boundary

V1 includes artifact storage/integrity behavior according to its milestones.

The audit explicitly does not silently assume universal OCR/PDF/DOCX extraction as a V1 requirement.

Any extraction feature beyond the written milestone/specification must be explicitly approved rather than opportunistically added.

## 15. Cloud boundary

No cloud dependency may become required for canonical memory, local search, local capture, daemon authority or local IPC without explicit architectural approval.

Cloud-synchronized folders may be used as filesystem backup targets where permitted by the backup architecture.

That does not make the cloud provider part of canonical application semantics.

## 16. Security boundary

V1 does not authorize:

- custom cryptography
- arbitrary shell access from UI code
- unrestricted filesystem access from the renderer
- public remote Cryptex control endpoints
- AI-driven trust changes
- AI-driven purge/recovery actions

Security/system namespaces remain daemon-authoritative.

## 17. M0 boundary

M0 is Foundation and P0 spikes, not feature implementation for later milestones.

M0 establishes or proves only the foundation required by its accepted contract, including:

- repository/toolchain/CI foundation
- blank desktop on both platforms
- daemon + IPC proof
- background lifecycle/update proof
- physical event-layout benchmark
- managed Syncthing proof
- Golden Brain foundation
- Doctor foundation

M1+ product behavior must not be pulled into M0 opportunistically.

## 18. Scope-change rule

A new feature, dependency or product guarantee is not part of V1 merely because it seems useful during implementation.

If scope needs to change:

1. identify the conflict or new requirement
2. evaluate architecture and milestone impact
3. update the appropriate specification/ADR
4. obtain acceptance
5. only then implement the changed scope

## 19. Source basis

This document consolidates existing V1 boundaries from:

- `docs/00_MASTER_SPEC.md`
- `AGENTS.md`
- `docs/18_IMPLEMENTATION_READINESS_PLAN.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`
- `docs/33_CURSOR_HANDOFF.md`

It does not independently expand Cryptex V1 scope.
