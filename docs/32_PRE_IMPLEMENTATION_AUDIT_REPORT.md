# 32 — Pre-Implementation Architecture Audit Report

**Audit date:** 2026-09-28

## Result

No fatal product-architecture blocker was found. The high-level model — local-first immutable memory, local derived indexes, controlled AI access, peer replication and rebuildable projections — remains sound.

However, the audit found several issues that could have caused major mid-project rework if left implicit. They are now either resolved in the specification or promoted to mandatory P0 spikes.

## Issues found and corrected

1. **Tiny-file scaling risk** — one JSON file per event is no longer assumed. Physical packaging requires benchmark before M2.
2. **Daemon lifecycle/update ambiguity** — background supervision and atomic Desktop/daemon update is a P0 M0 decision.
3. **Google Drive OAuth risk** — V1 uses encrypted filesystem backup targets; Drive Desktop can provide cloud sync without API coupling.
4. **Recovery trust gap** — Recovery Kit now includes a trust-bootstrap signing root, not only backup decryption material.
5. **Purge/hash-chain conflict** — minimal content-free purge bridge preserves integrity verification.
6. **Revocation ordering** — device sequence cutoff, not wall clock, decides post-revocation acceptance.
7. **Clock skew** — clocks are display/timeline data, never security authority.
8. **Syncthing versioning and purge** — purge scrubs Cryptex-controlled version history on reachable peers.
9. **Mass-deletion overclaim** — guard is documented as best-effort containment, not transactional prevention.
10. **AI raw-vault privacy bypass** — supported adapters use MCP; arbitrary user-granted filesystem access is outside classification enforcement.
11. **Memory poisoning** — authority tiers/protected namespaces prevent untrusted content from becoming security/global policy.
12. **Offline capture dependency** — Quick Capture writes durable Inbox data before any AI extraction.
13. **Agent health semantics** — Configured/Ready/Active/Verified/Attention are distinct.
14. **Grok integration surface split** — audit initially found the web Custom Connector requires public reachability/tunnel, then fresh xAI docs confirmed Grok Build supports local stdio MCP. Plan now supports Grok Build locally without adding a public endpoint; Grok web connector remains non-required.
15. **Webview/Tauri content security** — CSP, sanitization and least privilege are explicit requirements.
16. **IPC authorization** — role/capability handshake and current-user ACL are explicit.
17. **Obsidian write-loop/path edge cases** — desktop-only bridge, safe Vault API, origin tracking, attachment projection rules.
18. **Backup verification semantics** — upload is not verification; periodic sandbox restore is required.
19. **Backup multi-writer race** — one designated scheduled backup writer in V1.
20. **Schema drift** — Rust domain types generate JSON Schema/TypeScript bindings; CI detects drift.
21. **Diagnostics privacy** — local-only/redacted logs and opt-in diagnostic bundle.
22. **Supply-chain/licensing** — lockfiles, audits, notices and Syncthing MPL obligations added.
23. **Artifact extraction scope** — storage/integrity is V1; universal OCR/PDF/DOCX extraction is not silently assumed.
24. **Native UI expectation** — Tauri target is native-feeling, not literal native control toolkit.
25. **Toolchain stale assumption** — prior pnpm 12 statement was corrected; current audited baseline is pnpm 11.x.

## Remaining unknowns

Only implementation-sensitive questions remain, and each has an explicit gate:
- physical event packaging
- daemon supervision/update mechanism
- exact local IPC implementation
- exact managed Syncthing packaging/supervision
- later feature-specific crypto/container and Obsidian bridge details

These are not hidden areas; they are documented experiments with acceptance criteria.

## Authorization rule

The project is **planning complete but M1 is not authorized** until M0 passes the four P0 spikes and updates ADR-0012/0013 plus implementation addenda for IPC/Syncthing.
