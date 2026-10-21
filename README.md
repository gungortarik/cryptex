# Cryptex

**Markdown is a human projection, not canonical truth.**

Cryptex is a **local-first, event-sourced personal memory and knowledge engine** built in **Rust** (`cryptexd`) and **Tauri 2 + React 19** for **macOS** and **Windows 11**.

Instead of letting AI agents overwrite raw Markdown files or forcing you to run heavy Docker/Postgres/vector containers, Cryptex records canonical memory as **cryptographically signed, hash-chained immutable events**, derives a disposable local **SQLite + FTS5** search and knowledge graph (`Entity / Fact / Relation`), materializes a deterministic **Obsidian-compatible vault**, and exposes strict privacy-filtered tools to AI coding assistants via **MCP**.

---

## Quickstart: Connect to Cursor & Claude (30 Seconds)

You can run the lightweight **`cryptex-mcp`** server right now to give Cursor or Claude Desktop persistent, offline-first memory powered by SQLite FTS5.

### 1. Build the MCP Server
```bash
git clone https://github.com/gungortarik/cryptex.git
cd cryptex
cargo build --release -p cryptex-mcp
```
The binary will be compiled to `./target/release/cryptex-mcp`.

### 2. Add to Claude Desktop (`claude_desktop_config.json`)
Add the following to your Claude configuration file:
- **macOS:** `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Windows:** `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "cryptex": {
      "command": "/absolute/path/to/cryptex/target/release/cryptex-mcp"
    }
  }
}
```

### 3. Add to Cursor (`.cursor/mcp.json` or Cursor Settings)
In Cursor, go to **Settings > Features > MCP**, or add to `.cursor/mcp.json`:
```json
{
  "mcpServers": {
    "cryptex": {
      "command": "cargo",
      "args": ["run", "--release", "--manifest-path", "/absolute/path/to/cryptex/Cargo.toml", "-p", "cryptex-mcp"]
    }
  }
}
```

### 4. Available MCP Tools
- `cryptex_search`: Full-text search (FTS5) over personal memory, notes, and events.
- `cryptex_status`: Instant health check of the local engine and SQLite database.

---

## Why Cryptex?

| Approach | Canonical Source | Offline & No Docker? | Multi-Device Conflict-Free? | Tamper-Evident History? | AI Privacy Tiers & Poisoning Defense? |
| :--- | :--- | :---: | :---: | :---: | :---: |
| **Raw Markdown MCPs** (`mcp-obsidian`, etc.) | Mutable `.md` files | Yes | ❌ Merge conflicts | ❌ Overwrites in place | ❌ Full vault exposed |
| **Cloud / Container Stacks** (`OpenMemory`, `Graphiti`) | Postgres / Graph DB | ❌ Heavy Docker + LLM | Cloud-dependent | Partial | ❌ |
| **Lightweight SQLite MCPs** (`Memora`, etc.) | Single SQLite file | Yes | ❌ Single machine | ❌ Mutable rows | ❌ |
| **Cryptex** | **Signed Immutable Events (`.seg`)** | **✅ Single Rust Daemon** | **✅ Set-Union Convergence** | **✅ Ed25519 + Hash Chain** | **✅ 4-Tier Classification & Authority Gates** |

---

## Architecture at a Glance

```text
  Cursor / Claude Code / Codex / Antigravity / Obsidian / Desktop UI
                                  │
                     Local IPC (UDS / Named Pipe)
                     Role & Capability Handshake
                                  │
                                  ▼
               ┌──────────────────────────────────────┐
               │       cryptexd (Single Writer)       │
               │  Validate • Authorize • Sign • Seal  │
               └──────────────────┬───────────────────┘
                                  │
          ┌───────────────────────┴───────────────────────┐
          ▼                                               ▼
┌───────────────────────────────────┐   ┌───────────────────────────────────┐
│   CANONICAL STORAGE (Synced)      │   │  DERIVED LOCAL STATE (Disposable) │
│ • Signed Immutable Event Segments │   │ • SQLite + FTS5 Full-Text Index   │
│ • Content-Addressed SHA-256 Blobs │   │ • Entity / Fact / Relation Graph  │
│ • Content-Free Purge Bridge       │   │ • Deterministic Obsidian Vault    │
│ • Brain & Device Trust Metadata   │   │ • Health & Diagnostics Cache      │
└───────────────────────────────────┘   └───────────────────────────────────┘
```

### Core Engineering Invariants
1. **Single Local Writer (`cryptexd`):** Desktop UI, CLI, MCP bridges, and Obsidian never write canonical files directly.
2. **History Over Destructive Overwrite:** Corrections and updates emit new signed events that `supersede` prior events. If you delete your local SQLite database and Obsidian vault, Cryptex reconstructs both deterministically from canonical event segments.
3. **Purge Without Breaking the Hash Chain:** Physical content removal (`Purge`) replaces the event payload with a minimal content-free cryptographic bridge so per-device hash chains remain verifiable and stale offline peers cannot resurrect purged memories.
4. **Zero Cloud or LLM Dependency for Capture & Search:** Core capture, FTS5 retrieval, and entity consolidation work 100% offline with zero external API keys.
5. **Memory Poisoning & Privacy Defense:** Every memory carries a classification (`public`, `internal`, `private`, `restricted`) and authority tier. Untrusted external documents cannot silently install global rules or alter security namespaces.

---

## Implementation Progress (Milestones M0–M17)

Cryptex is engineered against a rigorous, pre-audited 34-document specification ([`DOCUMENT_INDEX.md`](DOCUMENT_INDEX.md) and [`docs/00_MASTER_SPEC.md`](docs/00_MASTER_SPEC.md)).

- [x] **M0 — Foundation & P0 Technical Spikes:** Cross-platform Rust/Tauri workspace, sealed segment storage benchmark (`ADR-0013`), daemon supervision (`ADR-0012`), Unix domain socket & Windows named-pipe IPC with strict peer ACLs (`ADR-0007`).
- [x] **M1 — Brain & Device Identity:** Ed25519 device identity, monotonic sequence allocation with replay-safe initialization markers, native OS SecureStore (`security-framework` on macOS, `windows-sys` Credential Manager on Windows), Recovery Kit metadata.
- [x] **M2 — Immutable Event Store:** Canonical event envelopes, generated JSON Schema/TypeScript bindings, atomic sealed `.seg` persistence, BLAKE3/SHA-256 integrity chains, automatic corruption quarantine.
- [x] **M3 — Local Projection & Search:** Rebuildable bundled SQLite + FTS5 external-content index, integrity-checked startup recovery, metadata conjunction filters, fractional-instant UTC ordering.
- [x] **M4 — Knowledge Engine:** Deterministic union-find entity resolution, aliases, typed `knowledge.assertion` v2 schemas, active/superseded facts, directional relations, and `RetrievalSafetyState` gates.
- [x] **M5 — Capture & Timeline:** Offline Quick Capture Inbox, deduplication acknowledgement & archive workflow, post-commit derived state catch-up worker, bounded keyset Timeline pagination with source provenance.
- [x] **M6 — Obsidian Materializer:** Deterministic Markdown note & entity-link (`[[WikiLink]]`) materialization under `Cryptex/`, copy-only attachment projection boundary, manifest verification, and Desktop "Open in Obsidian / Graph" handoff.
- [ ] **M7 — Controlled Obsidian Write-back** *(Ready for contributors — see [`docs/milestones/M07_OBSIDIAN_WRITEBACK.md`](docs/milestones/M07_OBSIDIAN_WRITEBACK.md))*
- [ ] **M8 — Device Trust & Syncthing Replication** *(See [`docs/milestones/M08_TRUST_REPLICATION.md`](docs/milestones/M08_TRUST_REPLICATION.md))*
- [ ] **M9 — Content-Addressed Attachment Store** *(See [`docs/milestones/M09_ATTACHMENTS.md`](docs/milestones/M09_ATTACHMENTS.md))*
- [ ] **M10 — Forget & Physical Purge Workflow** *(See [`docs/milestones/M10_FORGET_PURGE.md`](docs/milestones/M10_FORGET_PURGE.md))*
- [ ] **M11 — Encrypted Backup (`age`) & Disaster Recovery Bootstrap** *(See [`docs/milestones/M11_BACKUP_RESTORE.md`](docs/milestones/M11_BACKUP_RESTORE.md))*
- [ ] **M12 — Local MCP Bridge (`cryptex-mcp`) & AI Agent Adapters** *(High Priority — see [`docs/milestones/M12_MCP_AGENTS.md`](docs/milestones/M12_MCP_AGENTS.md))*
- [ ] **M13 — Desktop Product UI & Design System** *(React 19 + Tauri 2 — see [`docs/14_DESKTOP_UX_AND_DESIGN_SYSTEM.md`](docs/14_DESKTOP_UX_AND_DESIGN_SYSTEM.md))*
- [ ] **M14–M17 — Platform Polish, Cortex Re-ingestion, Hardening & Beta RC**

---

## Repository Structure

```text
apps/
  desktop/              # Tauri 2 + React 19 + TypeScript 7 desktop shell
crates/
  core/                 # Shared domain types, identifiers, and error contracts
  events/               # Canonical event schemas (v1/v2), hashing, and signatures
  storage/              # Atomic sealed segment store, quarantine, and locks
  projection/           # Rebuildable SQLite + FTS5 index & Obsidian vault materializer
  knowledge/            # Deterministic Entity / Fact / Relation reducer engine
services/
  cryptexd/             # Per-user single-writer daemon & authenticated IPC server
docs/                   # 34 authoritative architecture & milestone specifications
evidence/               # Cross-platform (macOS + Windows) verification & audit records
```

---

## Contributing (Human + AI-Agent Friendly)

We are actively looking for **Rust systems engineers**, **Tauri / React / TypeScript frontend builders**, and **MCP / Obsidian enthusiasts** to help build M7 through M17!

Because Cryptex has a complete, ambiguity-free specification in [`docs/`](docs/00_MASTER_SPEC.md) and pre-configured agent instructions in [`AGENTS.md`](AGENTS.md) and [`.cursor/agents/`](.cursor/agents/), contributing with **Claude Code, Cursor, Codex, or Antigravity** is extraordinarily fast.

👉 **Read [`CONTRIBUTING.md`](CONTRIBUTING.md) and check out the open GitHub Issues to claim a module!**

---

## Development & Build Quickstart

Authoritative project documentation lives under `docs/`. Runtime data must remain outside this source repository.

### Current implementation handoff

Before starting implementation or review work, read [`docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md`](docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md). It records the current milestone, verified evidence, open blockers, and the exact next task. Authoritative specifications and accepted ADRs still take precedence.

### Standalone Desktop build

Use `pnpm --filter @cryptex/desktop tauri:package-smoke` for a standalone probe
or `pnpm --filter @cryptex/desktop tauri:package --bundles app` for the macOS
application bundle. These commands build frontend assets and stage the sidecar.

Rust `--release` alone does not select Tauri's embedded frontend. A release
compiled without `tauri/custom-protocol` now exits with
`RELEASE_FRONTEND_MODE_REJECTED` before Tauri/daemon startup. Direct release
Desktop tests/builds must explicitly enable `--features tauri/custom-protocol`;
they do not replace frontend build or native package preparation. Debug
development through the existing Vite/Tauri commands remains supported.
