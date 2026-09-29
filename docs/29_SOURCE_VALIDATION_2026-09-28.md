# 29 — Source Validation — 2026-09-28

**Status:** PROPOSED source-audit record  
**Audit date:** 2026-09-28  
**Purpose:** Record the official-source technology baseline used to prepare Cryptex implementation. This document is evidence and version validation; it does not override `docs/00_MASTER_SPEC.md`, accepted ADRs, or milestone contracts.

## Authority boundary

- `docs/00_MASTER_SPEC.md` remains the primary product/architecture authority.
- Accepted ADRs remain authoritative for their decisions.
- `docs/18_IMPLEMENTATION_READINESS_PLAN.md` defines pre-implementation preparation order.
- `docs/20_MILESTONE_ROADMAP.md` defines milestone order and gates.
- This document records source facts and candidate versions.
- Final Cryptex implementation selections are recorded in `docs/31_TECH_STACK_BASELINE.md`.
- Unresolved P0 architecture choices remain unresolved until their required M0 spikes and ADRs pass.

## Existing Cryptex constraints relevant to this audit

The current authoritative Cryptex documents already establish:

- Tauri is the cross-platform desktop webview shell.
- macOS and Windows are first-class platforms.
- Rust is the core implementation language/toolchain.
- Node LTS and pnpm are used for the JavaScript/TypeScript toolchain.
- SQLite FTS5 is the V1 local search mechanism.
- Syncthing is the V1 replication transport direction, with exact managed packaging/supervision deferred to the M0 P0 spike.
- Obsidian integration is local and derived.
- TypeScript/React tooling is anticipated by the readiness plan, but the Master Spec does not independently lock React as an architectural invariant.
- Tailwind remains conditional and must not be assumed selected.

## Official-source validation

### Rust

**Observed stable release:** `1.98.1`  
**Release date:** 2026-09-03

Official Rust release notes identify 1.98.1 as a point release fixing a vtable-generation miscompilation.

Cryptex development machines have already verified Rust/Cargo 1.98.1 and the repository currently pins 1.98.1.

Sources:
- https://blog.rust-lang.org/2026/09/03/Rust-1.98.1/
- https://doc.rust-lang.org/releases.html

### Node.js

**Supported line:** Node.js `24.x` (Krypton)  
**Status:** LTS  
**Latest 24.x listed at audit time:** `24.21.0`  
**Repository/machine version currently pinned and verified:** `24.19.0`

Node.js officially lists the 24.x line as LTS. The official archive validates 24.19.0 as a real Krypton release and lists newer 24.x patches through 24.21.0.

This creates no architecture conflict, but `docs/31_TECH_STACK_BASELINE.md` must explicitly decide whether Cryptex stays on the already-verified 24.19.0 patch or advances the pin to 24.21.0 before Phase 6 is closed.

Sources:
- https://nodejs.org/en/about/previous-releases
- https://nodejs.org/en/download/archive/v24.19.0
- https://nodejs.org/en/download/archive/v24.21.0

### pnpm

**Validated Cryptex baseline:** `11.28.0`  
**Release date:** 2026-09-25

The pnpm project published 11.28.0 as a signed release. Cryptex's pre-implementation audit explicitly corrected the earlier pnpm-12 assumption and requires the pnpm 11.x baseline. A newer major must therefore not be adopted merely because it exists.

Source:
- https://github.com/pnpm/pnpm/releases/tag/v11.28.0

### Tauri

**Current stable core line at audit time:** `2.12.0`  
**Release date:** 2026-09-26

The Tauri release index lists:
- `tauri` crate: 2.12.0
- `tauri-cli` crate: 2.12.0
- `@tauri-apps/api`: 2.12.0
- `@tauri-apps/cli`: 2.12.0

Tauri 2.12 raised its Minimum Supported Rust Version to 1.90. Cryptex's pinned Rust 1.98.1 satisfies that minimum.

Tauri's official frontend guidance remains framework-agnostic and recommends Vite for SPA frameworks such as React. Tauri expects a static frontend output and does not natively depend on a server-side SSR runtime for the desktop application.

Sources:
- https://v2.tauri.app/blog/tauri-2.12/
- https://v2.tauri.app/release/
- https://v2.tauri.app/start/prerequisites/
- https://v2.tauri.app/start/frontend/

### React

**Latest stable at audit time:** `19.3.0`  
**Release date:** 2026-09-09

React's official version page identifies 19.3 as the latest stable line.

Cryptex readiness material anticipates TypeScript/React editor support, but React is not stated as a Master Spec invariant. Therefore 19.3.0 is a validated candidate, not silently promoted to an architectural decision by this source-audit document.

Sources:
- https://react.dev/versions
- https://react.dev/blog/2026/09/09/react-19-3

### TypeScript

**Latest release found at audit time:** `7.0.2`  
**Stable prior line:** `6.0.3`

Microsoft describes TypeScript 6.0 as the transition release from the former JavaScript compiler implementation toward the native TypeScript 7 generation. The TypeScript release feed lists 7.0.2 as the latest release and 6.0.3 as a stable 6.x release.

The exact Cryptex TypeScript pin must be selected deliberately in `docs/31_TECH_STACK_BASELINE.md`; this document does not silently choose 7.x merely because it is newer.

Sources:
- https://devblogs.microsoft.com/typescript/announcing-typescript-6-0/
- https://github.com/microsoft/TypeScript/releases

### Vite

**Latest stable found at audit time:** `8.3.1`  
**Release date:** 2026-09-24

Tauri recommends Vite for SPA frameworks such as React. Vite's release documentation identifies the 8.3 line as actively supported, and the release feed lists 8.3.1 as the latest release at the audit date.

Vite is a candidate build tool if the React/TypeScript frontend baseline is accepted in `docs/31_TECH_STACK_BASELINE.md`.

Sources:
- https://vite.dev/releases
- https://github.com/vitejs/vite/releases
- https://v2.tauri.app/start/frontend/

### SQLite / FTS5

**Latest SQLite release shown at audit time:** `3.53.4`  
**Release date:** 2026-07-24

SQLite's official documentation confirms FTS5 as its full-text-search virtual-table module. FTS5 has been included in the SQLite amalgamation since SQLite 3.9.0 and is the search mechanism already required by the Cryptex Master Spec.

The exact Rust SQLite binding/library selection is not decided by this document unless already covered by an accepted ADR or later stack baseline.

Sources:
- https://sqlite.org/
- https://sqlite.org/fts5.html

### Syncthing

**Latest stable release found at audit time:** `2.1.5`  
**Release date:** 2026-09-08

The official Syncthing release feed lists 2.1.5 as the latest stable release. The Windows development machine has independently verified `syncthing v2.1.5`.

This does **not** close the Cryptex managed-Syncthing architecture question. Exact binary packaging, isolated config/home, supervision, REST authentication, lifecycle, upgrade strategy and user-global-config isolation remain mandatory M0 P0-4 spike work.

Source:
- https://github.com/syncthing/syncthing/releases/tag/v2.1.5

### WebView2

Microsoft recommends the **Evergreen WebView2 Runtime** for release applications so security and reliability updates flow automatically. The Cryptex Windows workstation preflight already verifies presence of the runtime.

Cryptex should not treat the workstation's current WebView2 patch as an application-level fixed pin unless a later packaging decision explicitly requires Fixed Version distribution.

Sources:
- https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/evergreen-vs-fixed-version
- https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/runtime/

### Obsidian developer API

Obsidian's official developer documentation defines plugins as TypeScript extensions. Its Vault guidance recommends `Vault.process()` for read-modify-write operations because it prevents the file from changing between the read and write portions of the operation.

This supports the existing Cryptex requirement that controlled Obsidian write-back be implemented carefully and tested in its dedicated milestone/spike. It does not authorize direct canonical-memory mutation by the plugin.

Sources:
- https://docs.obsidian.md/
- https://docs.obsidian.md/Plugins/Vault

## Compatibility observations

1. Rust 1.98.1 satisfies Tauri 2.12's documented Rust MSRV of 1.90.
2. Node 24.x is an LTS line and satisfies Tauri's recommendation to use Node LTS when a JavaScript frontend framework is used.
3. Tauri officially supports framework-agnostic static frontend output and recommends Vite for React-style SPA projects.
4. SQLite FTS5 remains directly aligned with the frozen Cryptex V1 search architecture.
5. Syncthing 2.1.5 is a valid current integration-test baseline, but managed packaging remains an M0 spike rather than a pre-decided implementation detail.
6. Evergreen WebView2 aligns with ordinary Windows desktop deployment, while the exact Cryptex distribution/update behavior remains subject to the desktop/update implementation work.

## Items that must be resolved in `31_TECH_STACK_BASELINE.md`

The following are intentionally **not decided here**:

- exact Node patch: retain verified 24.19.0 or update to current 24.21.0;
- exact Tauri 2.x version to pin for M0;
- whether React is the accepted Cryptex desktop frontend framework;
- exact React version if accepted;
- exact TypeScript line/version (6.0.3 vs 7.0.2 or another explicitly justified stable pin);
- exact Vite version if Vite is selected;
- whether Tailwind is selected at all;
- exact Rust SQLite crate/binding;
- exact Tauri plugins and their versions;
- exact managed Syncthing packaging/supervision, which remains P0-4 work;
- exact Obsidian minimum app/API baseline for the plugin milestone.

## Audit result

No official-source finding in this review invalidates the current Cryptex architecture.

The principal version-selection questions are now explicit rather than implicit. They must be converted into a controlled implementation baseline in `docs/31_TECH_STACK_BASELINE.md` before Phase 6 is declared complete.
