# 00 — Cryptex Master Specification

**Status:** AUDITED / IMPLEMENTATION-READY AFTER P0 SPIKES  
**Baseline:** 2026-09-28

## 1. Product definition

Cryptex is a personal, local-first, cross-platform memory system for one user across trusted macOS and Windows devices. It provides durable structured memory, search, history, device replication, an Obsidian-compatible knowledge view, encrypted backups and controlled access for AI clients.

Cryptex is not a chat application, cloud database, team knowledge base, or raw conversation archive.

## 2. Core product promise

A user should be able to lose or reinstall any one computer and reconstruct that device from canonical replicated memory plus attachments. No local database, cache, generated Markdown vault or UI state is required for recovery.

## 3. Canonical architecture

```text
User / AI / Obsidian / Desktop
            |
            v
        cryptexd
            |
            +--> validate / authorize / sign
            |
            v
   immutable canonical events + blobs
            |
        replication
            |
            v
     trusted peer devices

Local derived state on every device:
- SQLite
- FTS search
- entity/fact/relation state
- caches
- Obsidian projection
- diagnostics
```

## 4. Canonical versus derived

Canonical:
- immutable durable events
- attachment blobs
- immutable brain identity metadata
- minimal purge ledger / integrity bridge data

Derived and disposable:
- SQLite database
- search index
- materialized active facts
- relationship graph cache
- Obsidian vault projection
- thumbnails/previews
- diagnostics cache

## 5. Durable concepts

- **EVENT** — something happened or changed.
- **ENTITY** — a person, project, course, device, organization, concept, etc.
- **FACT** — a scalar property known about an entity.
- **RELATION** — an entity-to-entity relationship.
- **ARTIFACT** — attachment or external content object.

Memory states: `ACTIVE`, `SUPERSEDED`, `FORGOTTEN`, `PURGED`.
Confidence: `confirmed`, `inferred`, `uncertain`.
Classification: `public`, `internal`, `private`, `restricted`.

## 6. Offline-first rule

Every trusted desktop may create memory while disconnected. Reconnection converges by immutable event union plus deterministic knowledge consolidation. No primary server and no primary desktop is required.

The iMac may be the preferred always-on replication and scheduled-backup peer, but it is never the canonical owner.

## 7. Single local writer

`cryptexd` is the only supported writer to canonical local Cryptex storage on a device. Desktop UI, CLI, MCP clients, Obsidian and AI adapters request writes through the daemon.

## 8. Replication

Syncthing is the V1 transport candidate and accepted architecture direction. It transports canonical files/blobs but does not define Cryptex semantics. Trust, schema validation, signatures, entity consolidation, purge semantics and recovery are Cryptex responsibilities.

The **v1 logical** event model is frozen. The **physical packaging** of canonical events is immutable sealed per-device segment/pack files (ADR-0013). One-file-per-event is rejected. Hybrid packaging is not used. Seal size and on-disk names are implementation parameters for M2, not frozen by the P0-1 benchmark profile.

The canonical v1 event contract, including existing `memory.note` bytes and
verification behavior, remains frozen under ADR-0016. Accepted ADR-0017 defines
a separate strict v2 schema for structured knowledge assertions and its
cross-version integrity chain. M4 may implement and verify v2 reading and
derived knowledge materialization, but production v2 writing remains disabled
until ADR-0017's independent trusted-device compatibility and downgrade gate
passes. No legacy v1 validator is widened.

## 9. Search

V1 search uses SQLite FTS5 plus metadata, relationships and filters. Semantic/vector search is optional post-V1. Core search must remain useful offline with no LLM.

Search availability is subject to the daemon's current `RetrievalSafetyState`.
A Brain-wide hold overrides normal search availability across M3 FTS results
and snippets, Desktop retrieval, and future controlled adapters. Missing,
corrupt, or stale safety state fails closed.

## 10. Obsidian

Each trusted desktop can materialize a complete local Obsidian-compatible vault from canonical memory. The vault is local and derived; it is not the replication source of truth.

Any daemon-controlled Obsidian generation or refresh must honor the same
`RetrievalSafetyState`. The daemon cannot revoke plaintext files, exports,
backups, or copies already written to locations accessible to the operating
system or user.

Manual edits to managed notes are captured as proposals and become canonical only after daemon validation and, where required, user review. User-owned Notes/Inbox content can be ingested as events.

V1 uses Obsidian Graph for full graph visualization. Cryptex may show compact relationship summaries but does not need to reimplement a graph engine.

## 11. AI access

Supported V1 local integration targets:
- Codex
- Cursor
- Claude Code
- Antigravity
- Grok Build (local stdio MCP)

AI uses local MCP/adapters and never requires direct access to canonical event folders or the generated vault path. Classification filters are enforced by Cryptex.

Grok product surfaces must be distinguished: Grok Build currently supports local stdio MCP and can integrate locally; Grok web Custom Connectors currently require a publicly reachable MCP URL/tunnel. Cryptex V1 does not require or expose a public connector endpoint.

## 12. Capture without AI dependency

Quick Capture is durable without an LLM. A raw Inbox capture is written first. Semantic extraction may occur later. No provider outage may prevent a user from capturing memory.

## 13. Security

- device trust is explicit
- device private keys live in OS secure storage
- canonical events are attributable and tamper-evident
- local at-rest protection relies primarily on FileVault/BitLocker
- no custom cryptography
- renderer/webview is least privilege
- no remote navigation or public Cryptex API in V1
- security/system namespaces are daemon-authoritative
- untrusted documents/AI output cannot silently install global rules or trust changes

## 14. Recovery root

Disaster recovery must restore both encrypted content and trust. The Recovery Kit therefore includes standardized recovery secret material for:
1. decrypting encrypted backups, and
2. authorizing the first new trusted device after all devices are lost.

The offline recovery signing secret is not routinely stored on normal devices.

## 15. Backup

V1 backup is an encrypted filesystem-target abstraction. A user may choose a Google Drive for Desktop folder, USB drive or another local filesystem path. Cryptex does not require Google OAuth/API access in V1.

One explicitly designated device performs scheduled backup/retention in V1 to avoid multi-writer pruning races. Other devices may take manual emergency backups or take over the role through an explicit role change.

## 16. Forget and purge

Forget removes content from normal retrieval and AI context but is reversible.

Purge is a deliberate content-removal workflow. Purge wins semantically over stale replicas. Minimal opaque integrity/ledger data may remain without the purged content so a hash chain and stale-device suppression remain verifiable.

Physical purge is best-effort across reachable Cryptex-controlled replicas, snapshots and backups. Cryptex cannot promise deletion from inaccessible lost devices or provider-internal historical versions it does not control.

## 17. Desktop product

Cryptex Desktop is a premium system utility + knowledge application, not a generic web dashboard. It provides Health, Memory, Timeline, Search, Devices, AI/Integrations, Backup/Recovery and Settings.

Search is globally accessible through a magnifier and `Cmd/Ctrl+K`. Home is health/dashboard-centered. Runtime health distinguishes configured, ready, active, verified and attention states.

Normal UI does not expose a prominent Start/Stop control. Restart Engine is available; Stop Engine is Advanced. Pause Sync, Pause AI Access and Maintenance Mode are explicit controls.

## 18. Platform goal

Tauri provides a cross-platform webview shell, so “native” means **native-feeling behavior and conventions**, not literal SwiftUI/WinUI controls. macOS and Windows receive platform-appropriate menus, tray/menu-bar behavior, window lifecycle, keyboard conventions, typography and polish while sharing one product design system.

## 19. Runtime footprint

Default app-managed runtime paths:
- macOS: `~/Library/Application Support/Cryptex/`
- Windows: `%LOCALAPPDATA%\Cryptex\`

Development uses `Cryptex-Dev` equivalents. Cryptex creates zero application/runtime folders in the visible home/Desktop/Documents by default.

## 20. Cortex transition

Cortex is not directly migrated. It becomes a read-only legacy source for controlled fresh re-ingestion into a new Cryptex brain. Legacy structure and runtime machinery are not inherited.

## 21. Quality contract

A feature is complete only after relevant unit, integration, cross-platform, failure, security, recovery and visual checks pass. Backup is not considered verified merely because upload succeeded; restore/reconstruction must be tested.

## 22. P0 gates before core implementation

M0 must resolve:
1. physical immutable event packaging at realistic scale
2. daemon/background-supervision and atomic update lifecycle on both OSes
3. exact local IPC transport/library and ACL model
4. Syncthing managed/bundled integration proof

See `23_REQUIRED_TECHNICAL_SPIKES.md`.
