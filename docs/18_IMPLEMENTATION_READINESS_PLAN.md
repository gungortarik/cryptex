# 18 — Implementation Readiness Plan

**Purpose:** Prepare both development platforms, repository, toolchain, Cursor, CI and P0 proof-of-concepts before feature implementation.

## Phase 0 — Rules

- one private GitHub monorepo: `cryptex`
- runtime memory outside repository
- Windows + macOS first-class from first commit
- no VPS/Tailscale/cloud DB in V1
- Cursor implements the written specification; it does not invent architecture
- M1+ blocked until P0 spikes pass

## Phase 1 — Windows workstation

Required:
- supported/up-to-date Windows 11
- BitLocker/Device Encryption where available
- Git for Windows
- GitHub CLI
- Cursor
- Visual Studio Build Tools / Desktop development with C++
- WebView2 runtime
- Rust via rustup
- Node LTS baseline pinned by repo
- pnpm exact version pinned by repo
- Obsidian
- Syncthing stable baseline for integration testing

Optional convenience:
- Windows Terminal
- PowerShell 7
- SQLite viewer

Do not disable Defender as a normal development requirement.

## Phase 2 — macOS workstations

Required:
- supported/up-to-date macOS
- FileVault recommended
- Xcode Command Line Tools
- Git
- GitHub CLI
- Cursor
- Rust via rustup
- Node LTS pinned by repo
- pnpm pinned by repo
- Obsidian
- Syncthing stable baseline

Homebrew is useful but not a Cryptex runtime dependency.

## Phase 3 — Preflight

Create:
- `scripts/preflight-windows.ps1`
- `scripts/preflight-macos.sh`

Validate tool presence, versions, architecture, OS, WebView2/build tools, filesystem permissions and key prerequisites.

## Phase 4 — Repository creation

Create private `gungortarik/cryptex` monorepo. Keep `main` buildable and tested. Use feature branches and PR review for significant milestones even as a solo project.

Recommended source checkout:
- Windows: `C:\Users\Tarik\github_repos\cryptex`
- macOS: `~/github_repos/cryptex`

This is developer source, not runtime data.

## Phase 5 — Monorepo skeleton

```text
apps/
  desktop/
  obsidian-plugin/
crates/
  core/
  events/
  storage/
  search/
  knowledge/
  security/
  sync/
  backup/
  projection/
services/
  cryptexd/
  cryptex-mcp/
tools/
  cryptex-cli/
packages/
  schemas/
  ui/
  design-tokens/
tests/
  golden-brain/
  fixtures/
  failure/
docs/
scripts/
.cursor/
  rules/
  agents/
```

Final crate/package splitting may be simplified during M0 if it creates unnecessary boundaries; the architecture separation matters more than the number of packages.

## Phase 6 — Toolchain pinning

Pin exact versions in repository files and lockfiles. Baseline candidates from the 2026-09-28 audit are recorded in `29_SOURCE_VALIDATION_2026-09-28.md`.

Important audit correction: use the current pnpm 11.x baseline, not the previously mentioned pnpm 12.

Repository should pin:
- Rust toolchain
- Node major/exact policy
- pnpm exact version
- Tauri/framework dependencies
- Cargo.lock
- pnpm-lock.yaml

## Phase 7 — Git/text policy

Use `.gitattributes` to enforce LF/source normalization where appropriate. Keep generated/runtime data ignored. Never commit secrets, Recovery Kits, real personal brains or private logs.

## Phase 8 — Runtime development data

Use separate local app-data roots:
- macOS `~/Library/Application Support/Cryptex-Dev/`
- Windows `%LOCALAPPDATA%\Cryptex-Dev\`

Start with Golden Brain and synthetic Dev Brain. Do not use real Cortex/personal memory during early milestones.

## Phase 9 — Cursor

Open repository root in Cursor.

Repository-controlled instructions:
- root `AGENTS.md`
- `.cursor/rules/` for focused architecture/Rust/UI/security/testing rules
- `.cursor/agents/` for verifier/auditor/test roles

Initial subagents:
- verifier
- security-auditor
- test-runner
- UI-quality-reviewer when UI work begins

Read-only agents may run in parallel. Multiple write-capable implementation agents must use separate Git branches/worktrees.

## Phase 10 — Editor extensions

Keep minimal:
- rust-analyzer
- TOML support
- Tauri tooling where useful
- TypeScript/React support
- Tailwind IntelliSense only if Tailwind remains selected
- trusted SQLite viewer for development

Optional GitLens/Error Lens. Avoid unnecessary AI/plugin sprawl.

## Phase 11 — Formatting/linting

Rust:
- `rustfmt`
- `clippy`

TypeScript/web:
- Biome candidate for formatting/linting, exact version pinned after M0 scaffold validation

Fast checks may run pre-commit; heavy failure/recovery suites belong in CI/milestone verification.

## Phase 12 — CI

GitHub Actions matrix on Windows + macOS:
- formatting/lint
- Rust tests
- TypeScript tests
- schema generation/drift check
- Golden Brain
- builds
- later Tauri packaging smoke builds

Add dependency/security auditing and third-party notice checks before beta.

## Phase 13 — Machine roles

Windows desktop:
- Windows development/QA
- named IPC/tray/installer tests

MacBook:
- macOS development/UI/Obsidian tests

Mac/iMac:
- second macOS peer
- long soak
- replication
- designated scheduled-backup-writer candidate

No machine is canonical master.

## Phase 14 — M0 Foundation

M0 output must include:
- repository and CI green
- blank Tauri Desktop launches on Windows/macOS
- Rust workspace builds
- cryptexd proof-of-concept
- local IPC proof-of-concept
- Golden Brain fixture foundation
- Doctor foundation
- background-runtime spike resolved
- physical event-layout benchmark resolved
- managed Syncthing spike resolved
- exact P0 ADRs accepted

Only then proceed to M1.
