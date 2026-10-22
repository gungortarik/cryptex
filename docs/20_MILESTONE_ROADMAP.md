# 20 — Milestone Roadmap

The order minimizes architectural rework. Milestones may be split into smaller PRs but should not be reordered casually.

**Owner-approved scheduling exception (2026-10-19):** M7 remains OPEN /
DEFERRED with managed-note write-back disabled while its physical-containment
prerequisite is unresolved. M6 generated Markdown and Obsidian visualization
remain available. M8 is the next active implementation milestone; its existing
prerequisites and completion gates apply. M7's scope and eventual completion
requirements remain in force. Decision:
`evidence/m7/m7-temporary-deferral-and-m8-sequencing-proposal-20261019.md`.

## M0 — Foundation and P0 Spikes

- monorepo/toolchains/CI
- blank desktop on both OSes
- daemon + IPC proof
- background lifecycle/update proof
- event physical-layout benchmark
- Syncthing managed integration proof
- Golden Brain skeleton
- Doctor skeleton

**Gate:** all P0 ADRs accepted.

## M1 — Brain + Device Identity

- Brain identity metadata
- install/device identity
- OS secure-store integration
- sequence allocation foundation
- pairing data structures
- Recovery Kit data model/public recovery metadata

## M2 — Immutable Event Store

- finalized physical packaging from P0
- schemas generated from Rust types
- atomic writes
- hashes/signatures/integrity chain
- quarantine
- schema version behavior

## M3 — Local Projection + Search

- SQLite rebuildable projection
- FTS5
- metadata filters
- rebuild/checkpoint behavior
- corruption rebuild tests

## M4 — Knowledge Engine

- entity resolution
- aliases
- facts
- relations
- confidence/state
- correction/supersede
- semantic consolidation

## M5 — Capture + Timeline

- offline Quick Capture Inbox
- durable raw capture
- timeline/history/sources
- optional later extraction proposals

## M6 — Obsidian Materializer

- complete local generated vault
- safe names/frontmatter/links
- rebuild
- attachment projection
- Open Vault/Graph integration

## M7 — Obsidian Controlled Write-back

- plugin bridge
- edit detection
- loop prevention
- proposal/review/apply/reject

## M8 — Device Trust + Replication

- pairing UI/flow
- Syncthing transport controller
- incoming validation
- offline convergence
- revoke/retire/compromise
- sequence cutoff
- replication protection status

## M9 — Attachments

- SHA-256 blob store
- dedup
- reference tracking
- integrity repair
- projection copy/hard-link behavior

## M10 — Forget + Purge

- logical forget/restore
- purge confirmation
- purge ledger/hash bridge
- stale peer suppression
- version/snapshot cleanup
- attachment reference-aware purge

## M11 — Backup + Disaster Restore

- filesystem target
- encryption
- designated backup writer
- retention
- verification
- sandbox restore rehearsal
- Recovery Kit first-device bootstrap
- manual export bundle

## M12 — MCP + AI Integrations

- local MCP bridge
- role/capability policy
- classification filters
- Codex
- Cursor
- Claude Code
- Antigravity
- Grok Build local MCP
- runtime smoke/health verification

## M13 — Desktop Product UI

- finalized design system
- Home/Health
- Memory/detail
- Global Search
- Timeline
- Devices
- AI/Integrations
- Backup/Recovery
- Doctor/Settings

Core functionality may have temporary developer UI before this milestone, but release UI quality is implemented here and refined continuously afterward.

## M14 — Platform Polish + Updates

- macOS menu/window/background behavior
- Windows tray/window/background behavior
- autostart
- updater coordination
- DPI/Retina/theme/accessibility
- packaging smoke

## M15 — Cortex Fresh Re-ingestion

- read-only inventory
- category extraction/import
- dedup/conflict review
- provenance
- acceptance/cutover

## M16 — Hardening

- complete failure matrix
- security review
- large-scale tests
- sync stress
- backup/restore drills
- sleep/wake/offline/reconnect soak

## M17 — Private Beta / Release Candidate

- signed/notarized packaging as appropriate
- final third-party notices
- release checklist
- real personal brain onboarding only after backup/recovery confidence
