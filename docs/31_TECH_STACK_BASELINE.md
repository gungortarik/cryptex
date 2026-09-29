# 31 — Technology Stack Baseline

**Status:** ACCEPTED  
**Baseline date:** 2026-09-28  
**Purpose:** Convert the validated technology sources in `docs/29_SOURCE_VALIDATION_2026-09-28.md` into an explicit Cryptex implementation baseline without changing the architecture.

## Authority

This document is subordinate to:

1. `docs/00_MASTER_SPEC.md`
2. accepted ADRs
3. milestone contracts
4. `docs/18_IMPLEMENTATION_READINESS_PLAN.md`
5. `docs/20_MILESTONE_ROADMAP.md`

If implementation evidence conflicts with this baseline, stop and raise the conflict. Do not silently substitute another technology or version.

## Selection principles

- Prefer exact, reproducible repository pins.
- Prefer already verified workstation versions when they satisfy the project requirements.
- Do not upgrade merely because a newer version exists.
- macOS and Windows remain first-class.
- Do not add a cloud dependency.
- Runtime data remains outside the source repository.
- Do not resolve P0 architecture questions in this document.
- Keep M0 dependencies minimal.
- Add dependencies only when they serve the current milestone.

## Repository toolchain baseline

| Component | Exact baseline | Decision |
|---|---:|---|
| Rust | `1.98.1` | PIN |
| Node.js | `24.19.0` | PIN |
| pnpm | `11.28.0` | PIN |
| Cargo resolver | `3` | PIN |

### Rust — `1.98.1`

Retain the version already verified on both development platforms and already pinned by `rust-toolchain.toml`.

Required components:

- `rustfmt`
- `clippy`

No rolling `stable` toolchain is permitted for the repository baseline.

### Node.js — `24.19.0`

Retain the exact version already installed and verified on both development machines.

`24.21.0` is a newer release in the same Node 24 LTS line, but there is no current Cryptex requirement forcing an immediate patch upgrade. Avoid changing the just-verified cross-platform baseline during readiness work.

Node upgrades must be explicit repository changes with macOS + Windows verification.

### pnpm — `11.28.0`

Pin exactly `11.28.0`.

This preserves the explicit pre-implementation audit correction that Cryptex uses the pnpm 11.x baseline rather than automatically moving to pnpm 12.

## Desktop application baseline

Cryptex Desktop uses the architecture already established by the Master Spec:

- Tauri 2 desktop shell
- React UI
- TypeScript
- Vite SPA build pipeline
- platform-native behavior implemented through Tauri/platform adapters
- no server-side-rendering runtime requirement
- no cloud runtime dependency

### Tauri

Pin the core Tauri M0 baseline as:

| Package | Exact version |
|---|---:|
| Rust `tauri` crate | `2.12.0` |
| Rust `tauri-build` crate | `2.7.0` |
| npm `@tauri-apps/api` | `2.12.0` |
| npm `@tauri-apps/cli` | `2.12.0` |

Rationale:

- these versions are from the same 2026-09-26 Tauri release set;
- Rust `1.98.1` satisfies the Tauri 2.12 MSRV requirement;
- the release includes desktop-relevant Windows/macOS improvements;
- exact pins keep M0 reproducible.

Do not add Tauri plugins until a current-milestone requirement needs them. Every plugin must be individually pinned and must use least-privilege capabilities.

## Frontend baseline

Pin:

| Package | Exact version |
|---|---:|
| `react` | `19.3.0` |
| `react-dom` | `19.3.0` |
| `@types/react` | `19.3.0` |
| `@types/react-dom` | `19.3.0` |
| `typescript` | `7.0.2` |
| `vite` | `8.3.1` |
| `@vitejs/plugin-react` | `6.1.1` |

### React — `19.3.0`

React is selected as the shared desktop UI framework anticipated by the readiness plan's TypeScript/React tooling direction.

This is an implementation-stack selection, not a change to the Master Spec's UX requirements. Cryptex must still look and behave like a premium desktop application rather than a generic web dashboard.

Do not enable experimental React features merely because the framework exposes them.

### TypeScript — `7.0.2`

Use the current stable TypeScript 7 compiler for the new codebase.

Rationale:

- Cryptex has no legacy TypeScript configuration requiring migration;
- TypeScript 7 is a stable release, not a preview;
- it is intended to preserve TypeScript 6 type-checking semantics while using the native compiler implementation;
- starting the new repository on the stable native compiler avoids deliberately creating an immediate 6-to-7 migration task.

If M0 scaffold validation exposes a concrete incompatibility with required Cryptex tooling, stop and document the evidence before changing this pin.

### Vite — `8.3.1`

Use Vite for the Tauri SPA frontend.

Tauri officially recommends Vite for SPA frameworks such as React, and Vite 8.3 is the actively supported line at the baseline date.

### `@vitejs/plugin-react` — `6.1.1`

Use the standard Vite React plugin.

Do **not** enable the experimental React Compiler integration during M0. M0 needs a deterministic blank desktop shell, not optional compiler experimentation.

## Styling baseline

### Tailwind CSS — NOT SELECTED FOR M0

Do not add Tailwind during readiness or the blank M0 shell.

Reason:

- the readiness plan explicitly treats Tailwind tooling as conditional;
- M0 does not require a finalized styling framework;
- release UI/design-system implementation belongs to later UI work;
- avoiding an unnecessary dependency now preserves freedom without affecting the architecture.

This does not prohibit selecting Tailwind later if the appropriate UI/design-system milestone explicitly accepts it.

## Formatting and linting

### Rust

Required:

- `rustfmt` from the pinned Rust toolchain
- `clippy` from the pinned Rust toolchain

### TypeScript / frontend

Do not pin Biome yet.

`docs/18_IMPLEMENTATION_READINESS_PLAN.md` explicitly says the exact Biome version is pinned after M0 scaffold validation. Follow that order.

## SQLite

Architecture decision:

- SQLite
- FTS5
- derived/rebuildable local projection

No exact Rust SQLite binding is selected in this baseline.

The binding choice must be made when the relevant milestone requires it and must preserve:

- local-first operation;
- rebuildability;
- FTS5 availability;
- Windows/macOS support;
- deterministic migration/rebuild behavior.

Do not introduce SQLite-backed canonical storage.

## Syncthing

Development integration baseline:

- Syncthing `2.1.5`

This is a workstation/integration-test baseline only.

This document does **not** decide:

- bundled vs managed external binary details;
- process supervision;
- isolated config/home layout;
- update coordination;
- REST authentication implementation.

Those remain mandatory P0-4 M0 spike outputs and must be captured in the required ADR/addendum before the replication milestone.

## WebView2

Windows uses the installed Evergreen WebView2 Runtime.

Do not pin the current workstation WebView2 patch as a Cryptex application dependency. Fixed-Version WebView2 distribution may only be introduced by a later explicit packaging decision.

## Obsidian

Obsidian remains an external desktop application and local integration target.

No exact Obsidian application version is pinned for M0.

The Obsidian plugin/API minimum baseline is selected when the Obsidian milestone begins and after its required write-back spike. The plugin must never write canonical Cryptex memory directly.

## Secure storage

Device secrets must use OS secure storage through platform adapters.

The exact Rust secure-store library is not selected by this readiness baseline. Select and pin it in the device-identity milestone or an accepted ADR; do not introduce custom cryptography.

## Dependencies intentionally deferred

The following are **not missing Phase 6 work** because their selection belongs to later explicit milestone/spike decisions:

- exact SQLite Rust binding;
- secure-store Rust library;
- IPC Rust library/implementation;
- managed Syncthing packaging/supervision mechanism;
- backup encryption/container library;
- Obsidian plugin minimum API/app version;
- Tailwind or another final styling system;
- Biome exact version before M0 scaffold validation;
- Tauri plugins not required by the current milestone.

## Exact-pin policy

Repository manifests should use exact versions for the baseline above rather than broad caret/range declarations where practical.

Examples:

- npm/pnpm: `"react": "19.3.0"` rather than `"^19.3.0"`
- Cargo workspace dependency: `tauri = "=2.12.0"`
- Cargo build dependency: `tauri-build = "=2.7.0"`

Lockfiles are committed:

- `Cargo.lock`
- `pnpm-lock.yaml`

Dependency upgrades are focused changes with:

1. source/release-note review;
2. lockfile update;
3. macOS verification;
4. Windows verification;
5. relevant format/lint/test/build checks.

## Phase 6 repository changes required by this baseline

Before Phase 6 is complete, repository manifests must reflect:

- Rust `1.98.1`
- Node `24.19.0`
- pnpm `11.28.0`
- Tauri pins listed above
- React/TypeScript/Vite pins listed above
- committed `Cargo.lock`
- committed `pnpm-lock.yaml`

The existing base toolchain pin satisfies the first three items. Tauri/frontend dependency pins still need to be written into repository manifests and the lockfiles regenerated.

## Acceptance condition

This document may be marked **ACCEPTED** when:

1. the human project owner approves these baseline selections;
2. repository manifests match the exact pins;
3. lockfiles regenerate successfully;
4. the dependency graph resolves on macOS;
5. the dependency graph resolves on Windows;
6. no authoritative Cryptex document conflicts with the resulting baseline.

Only then may Phase 6 — Toolchain Pinning be reported complete.
