# Contributing to Cryptex

Welcome! Cryptex is a local-first, event-sourced personal memory system in Rust and Tauri 2. Whether you are a **Rust systems developer**, a **React 19 / Tauri UI craftsman**, or an **AI tooling / MCP builder**, this repo is structured so you can ship high-impact PRs without guessing architectural intent.

---

## Why Contributing to Cryptex is Different

Most open-source projects have vague requirements buried in old chat threads. Cryptex has a **complete, pre-audited 34-document engineering specification**:
1. [`docs/00_MASTER_SPEC.md`](docs/00_MASTER_SPEC.md) — The authoritative product and architecture contract.
2. [`docs/30_ADR_INDEX.md`](docs/30_ADR_INDEX.md) — Accepted Architecture Decision Records (`ADR-0001` through `ADR-0018`).
3. [`docs/milestones/`](docs/milestones/) — Scoped contracts for every milestone (`M0` through `M17`).
4. [`AGENTS.md`](AGENTS.md) & [`.cursor/agents/`](.cursor/agents/) — Built-in guardrails so you can pair-program with **Cursor, Claude Code, Codex, or Antigravity** and stay 100% aligned with the architecture.

---

## High-Impact Areas Looking for Contributors

We have already completed and verified **M0 through M6** (Identity, OS Keychain/DPAPI, Immutable Sealed Event Store, SQLite + FTS5 Index, Deterministic Knowledge Engine, Capture/Timeline, and Obsidian Vault Materialization).

Here are the best self-contained tracks you can pick up right now:

### Track 1: AI & MCP Bridge (`M12` — Rust)
- **Spec:** [`docs/12_MCP_AND_AI_ADAPTERS.md`](docs/12_MCP_AND_AI_ADAPTERS.md) & [`docs/milestones/M12_MCP_AGENTS.md`](docs/milestones/M12_MCP_AGENTS.md)
- **Goal:** Build `services/cryptex-mcp` (stdio MCP server talking to `cryptexd` over local IPC) exposing `cryptex_search`, `cryptex_context`, `cryptex_remember`, `cryptex_correct`, `cryptex_forget`, `cryptex_timeline`, and `cryptex_status` with classification filtering (`public`/`internal`/`private`/`restricted`).

### Track 2: Desktop Product UI & Design System (`M13` — React 19 + TypeScript + Tauri 2)
- **Spec:** [`docs/14_DESKTOP_UX_AND_DESIGN_SYSTEM.md`](docs/14_DESKTOP_UX_AND_DESIGN_SYSTEM.md) & [`docs/milestones/M13_DESKTOP_UI.md`](docs/milestones/M13_DESKTOP_UI.md)
- **Goal:** Build a calm, high-density, native-feeling desktop utility interface (Home/Health dashboard, `Cmd/Ctrl+K` Global Search palette, Entity/Memory browser, Devices view, Backup/Recovery view, Doctor view). No Rust crypto experience required!

### Track 3: Content-Addressed Attachment Store (`M9` — Rust)
- **Spec:** [`docs/06_STORAGE_ARCHITECTURE.md`](docs/06_STORAGE_ARCHITECTURE.md) & [`docs/milestones/M09_ATTACHMENTS.md`](docs/milestones/M09_ATTACHMENTS.md)
- **Goal:** Implement SHA-256 deduplicated blob storage, reference counting, integrity verification, and copy-only projection into the local Obsidian vault.

### Track 4: Forget & Physical Purge Workflow (`M10` — Rust)
- **Spec:** [`docs/09_FORGET_PURGE_AND_DATA_LIFECYCLE.md`](docs/09_FORGET_PURGE_AND_DATA_LIFECYCLE.md) & [`docs/milestones/M10_FORGET_PURGE.md`](docs/milestones/M10_FORGET_PURGE.md)
- **Goal:** Implement reversible logical `Forget` and physical `Purge` with the content-free `purge_bridge` ledger so hash chains remain valid after content erasure.

### Track 5: Encrypted Backups & Disaster Recovery (`M11` — Rust)
- **Spec:** [`docs/10_BACKUP_RESTORE.md`](docs/10_BACKUP_RESTORE.md) & [`docs/milestones/M11_BACKUP_RESTORE.md`](docs/milestones/M11_BACKUP_RESTORE.md)
- **Goal:** Implement `age`-encrypted filesystem backup snapshots, retention policies, sandbox restore verification, and Recovery Kit first-device bootstrap.

### Track 6: Obsidian Controlled Write-back (`M7` — TypeScript / Rust)
- **Spec:** [`docs/13_OBSIDIAN_INTEGRATION.md`](docs/13_OBSIDIAN_INTEGRATION.md) & [`docs/milestones/M07_OBSIDIAN_WRITEBACK.md`](docs/milestones/M07_OBSIDIAN_WRITEBACK.md)
- **Goal:** Detect human edits or `Inbox/` drops in the local Obsidian vault and submit them as validated event proposals to `cryptexd` without creating write loops.

---

## Development Setup

### Prerequisites
- **Rust:** pinned via `rust-toolchain.toml` (`1.98.1`)
- **Node.js:** `24.x LTS` (see `.node-version`)
- **pnpm:** `11.28.0`
- **OS:** macOS (Apple Silicon / Intel) or Windows 11

### Running Checks Locally

```bash
# 1. Install frontend dependencies
pnpm install --frozen-lockfile

# 2. Check Rust formatting
cargo fmt --all -- --check

# 3. Run core workspace tests
cargo test --locked -p cryptex-core -p cryptex-events -p cryptex-storage -p cryptex-projection -p cryptex-knowledge

# 4. Run daemon tests
cargo test --locked -p cryptexd

# 5. Run strict Clippy
cargo clippy --locked -p cryptex-projection -p cryptexd --all-targets -- -D warnings
```

---

## Using AI Coding Agents (Cursor / Claude Code / Antigravity)

If you use an AI coding assistant to help build your PR, pass it the prompt contract from [`docs/33_CURSOR_HANDOFF.md`](docs/33_CURSOR_HANDOFF.md):

> *"Read `AGENTS.md`, `docs/00_MASTER_SPEC.md`, and the target milestone file under `docs/milestones/`. Implement only the scoped feature/issue without violating canonical single-writer or security invariants. Add unit/integration tests for all new behavior."*

### Non-Negotiable Rules for All PRs
- Never write runtime or personal data inside the Git checkout.
- `cryptexd` is the sole canonical writer; clients communicate via IPC.
- Preserve cross-platform portability (UTF-8, LF line endings, no canonical symlink assumptions, Windows/macOS path safety).
- All tests and `cargo fmt` / `clippy -D warnings` must pass before merging.
