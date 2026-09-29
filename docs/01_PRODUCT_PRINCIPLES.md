# 01 — Cryptex Product Principles

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## 1. Purpose

This document records the non-negotiable product principles already established by the Cryptex specification set.

It is not a replacement for the Master Spec and does not independently introduce new architecture.

When implementation convenience conflicts with these principles, the implementation must stop and the conflict must be raised rather than silently changing the product.

## 2. Product identity

Cryptex is a personal, local-first, cross-platform memory and knowledge system for one user across trusted macOS and Windows devices.

Cryptex is not:

- a chat application
- a cloud database
- a team knowledge base
- a raw conversation archive

The product exists to provide durable structured memory, history, search, replication, derived knowledge views, recovery and controlled AI access without making cloud availability a prerequisite for core operation.

## 3. Local-first and offline-first

Local operation is the default architectural assumption.

Every trusted desktop must be able to perform the locally supported behavior of its milestone while offline and reconcile later when peers become available.

No primary server is required.

No desktop is the canonical master.

An always-on device may be operationally preferred for replication or scheduled backup, but it remains a peer rather than the source of truth.

## 4. Canonical memory is durable and immutable

Canonical durable memory is represented by immutable event data and the other explicitly canonical durable objects defined by the Master Spec.

Existing canonical events are never edited in place.

Derived representations must never quietly become canonical because they are convenient to query or display.

## 5. Derived state is disposable and rebuildable

SQLite databases, search indexes, materialized knowledge state, caches, generated Obsidian projections and diagnostics caches are derived state.

They are disposable and rebuildable.

Loss of a local database, index, generated vault or UI state must not make recovery of canonical memory impossible.

The system must preserve the ability to reconstruct derived state from canonical durable data according to the applicable milestone contract.

## 6. Single canonical writer

`cryptexd` is the only supported writer to canonical local Cryptex storage on a device.

Desktop, CLI, MCP clients, Obsidian integrations and AI adapters request canonical changes through daemon-controlled boundaries.

No client may gain a direct canonical-file mutation path for implementation convenience.

## 7. Recovery is a product property

Recovery is not optional operational tooling.

A user should be able to lose or reinstall one computer and reconstruct that device from canonical replicated memory plus required attachment data.

No local database, cache, generated Markdown vault or UI state may be required as the sole recovery source.

Backup claims require restore/reconstruction evidence; successful upload or file creation alone is not sufficient verification.

## 8. Cross-platform parity

macOS and Windows are first-class platforms.

Shared product guarantees must hold on both operating systems even when platform-specific implementation mechanisms differ.

Canonical behavior must not depend on one platform's path, filename, case, line-ending or symlink behavior.

Platform integration belongs behind platform-specific boundaries rather than being scattered through core domain logic.

## 9. Security and privacy by default

Cryptex treats imported memory, Markdown, attachment metadata and AI output as untrusted input.

Security principles include:

- explicit device trust
- device private keys stored in operating-system secure storage
- attributable and tamper-evident canonical events
- least-privilege renderer/webview capabilities
- no arbitrary shell or unrestricted filesystem access from UI code
- no custom cryptography
- no public Cryptex Internet API required for V1
- security/system namespaces remain daemon-authoritative
- logs do not contain memory bodies, secrets, recovery material or credentials by default

Privacy boundaries must remain intact even when integrations or automation are added later.

## 10. AI is controlled, optional and non-authoritative

AI is an integration layer, not the canonical memory authority.

Supported AI clients use controlled local MCP/adapters and do not require direct access to canonical event folders or the generated Obsidian vault path.

Classification filters are enforced by Cryptex.

Restricted memory is not exposed to AI by default.

Purge and recovery operations are never exposed as autonomous AI tools.

Core capture and core search must remain useful without an LLM.

A provider outage must not prevent durable memory capture.

## 11. Transport does not define semantics

Replication transport is not the application model.

Syncthing is the accepted V1 transport direction, but Cryptex remains responsible for:

- trust
- validation
- signatures/integrity
- knowledge consolidation
- purge semantics
- stale-device handling
- recovery

Transport success must not be treated as semantic acceptance of data.

## 12. Honest deletion semantics

Cryptex distinguishes Forget from Purge.

Forget removes content from normal retrieval and AI context while remaining reversible.

Purge is a deliberate content-removal workflow and wins semantically over stale replicas.

Cryptex may retain minimal opaque integrity/ledger data where required to preserve integrity verification and stale-device suppression without retaining purged content.

Physical purge is best-effort across reachable Cryptex-controlled replicas, snapshots and backups.

The product must not promise deletion from inaccessible lost devices or provider-controlled historical versions outside Cryptex control.

## 13. Premium desktop product quality

Cryptex must feel like a deliberately designed premium desktop application, not a generic AI-generated web dashboard.

Tauri provides the cross-platform shell; native quality means native-feeling behavior and conventions rather than requiring literal SwiftUI or WinUI controls.

macOS and Windows may differ in menus, tray/menu-bar behavior, window lifecycle, keyboard conventions, typography and platform polish while preserving one Cryptex design language.

## 14. Runtime health must be truthful

Configured, Ready, Active, Verified and Attention are distinct runtime states.

Configuration presence must not be promoted into a stronger runtime claim without evidence.

Doctor, health surfaces and integrations must report what is actually working rather than what is merely configured.

## 15. Source repository and runtime data are separate

Runtime memory and application state must never be stored inside the source repository.

The repository contains source code, tests, synthetic fixtures, schemas, documentation and repository-controlled development configuration.

It must not contain real personal brains, real Cortex memory, private logs, secrets, credentials or Recovery Kits.

Development uses dedicated `Cryptex-Dev` runtime roots outside the source tree.

## 16. Deterministic, testable implementation

Prefer simple, deterministic and testable implementations.

A feature is not complete because it compiles or launches.

Relevant unit, integration, cross-platform, failure, security, recovery and visual checks must pass according to the active milestone and Definition of Done.

Tests must not be weakened merely to make CI green unless the specification explicitly changes.

Corruption, signature failures and schema incompatibility must never be silently swallowed.

## 17. Milestone discipline

Work proceeds in the order defined by the Milestone Roadmap and Implementation Readiness Plan.

Only the currently selected milestone may be implemented.

Later milestone features must not be implemented opportunistically.

No milestone is complete until its acceptance criteria, required tests and completion gate are green.

## 18. Architecture changes require evidence and process

Implementation agents, Cursor and other tools implement the written specification; they do not invent architecture.

If implementation evidence shows an accepted decision is wrong:

1. stop rather than silently changing the architecture
2. record the evidence and tradeoffs in the relevant ADR
3. update the Master Spec if the accepted architecture changes
4. only then implement the newly accepted direction

Chat history is not long-term architectural authority.

Significant decisions must land in repository-controlled Markdown or ADRs.

## 19. Source basis

This document consolidates existing principles from:

- `docs/00_MASTER_SPEC.md`
- `AGENTS.md`
- `docs/18_IMPLEMENTATION_READINESS_PLAN.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/21_DEFINITION_OF_DONE.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`
- `docs/33_CURSOR_HANDOFF.md`

It does not independently redesign or extend Cryptex architecture.
