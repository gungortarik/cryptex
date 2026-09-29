# 22 — Cryptex Risk Register

**Status:** PROPOSED  
**Baseline:** 2026-09-28

## 1. Purpose

This document records active architectural and implementation risks already identified by the audited Cryptex planning set.

It does not create new architecture. It makes known risks, mitigations and required gates explicit so they cannot be forgotten during implementation.

## 2. Risk handling rule

A risk is not considered resolved because an implementation choice was made.

Where a mandatory spike, ADR, test or milestone gate is named, the risk remains open until that evidence exists and the required acceptance step is complete.

If new evidence contradicts an accepted decision, implementation stops and the ADR/specification process is used.

## 3. P0 architectural risks

### R-01 — Physical event packaging / tiny-file scale

**Risk:** one-file-per-event may create unacceptable filesystem/Syncthing/indexing/rebuild behavior at realistic scale.

**Mitigation:** M0 P0-1 benchmarks one-file-per-event, sealed segment/pack files and a hybrid only if justified at 10k/100k/500k events on macOS and Windows.

**Gate:** ADR-0013 accepted before M2.

### R-02 — Background daemon lifecycle/update ambiguity

**Risk:** incorrect supervision/update behavior could produce duplicate writers, unavailable engine behavior or canonical-data damage.

**Mitigation:** M0 P0-2 proves close-window behavior, startup, crash recovery, single instance, reconnect, coordinated update and rollback on both OSes.

**Gate:** ADR-0012 accepted before M1/M2.

### R-03 — Local IPC implementation/security

**Risk:** a weak or platform-fragile IPC choice could cause security, reconnect or portability failures.

**Mitigation:** M0 P0-3 proves user-only permissions/ACLs, short macOS socket path, Windows named-pipe security, protocol negotiation, client roles, reconnect and malformed/oversized request handling.

**Gate:** ADR-0007 implementation addendum before production daemon APIs.

### R-04 — Managed Syncthing integration

**Risk:** bundling/supervising Syncthing may conflict with user-global Syncthing, create lifecycle complexity or fail packaging/licensing requirements.

**Mitigation:** M0 P0-4 proves isolated config/home, process lifecycle on both OSes, REST authentication, pairing/status behavior, versioning visibility, upgrade compatibility and notices.

**Gate:** ADR-0004 implementation addendum before replication milestone.

## 4. Feature-specific P1 risks

### R-05 — Backup encryption/container

**Risk:** selecting an unsuitable format/library could compromise large-object handling, interruption safety, restore or key recovery.

**Mitigation:** mandatory P1 spike before backup feature productization.

### R-06 — Obsidian write-back loops/path behavior

**Risk:** managed note updates may create write loops, path corruption or unsafe filesystem behavior.

**Mitigation:** mandatory P1 spike proving safe event listening, Vault API processing, loop suppression and plugin IPC.

### R-07 — Attachment projection portability

**Risk:** hard-link assumptions may fail across filesystems/platforms and cleanup may leave inconsistent derived state.

**Mitigation:** benchmark hard-link vs copy fallback/cleanup on macOS and Windows before productization.

### R-08 — Native-feeling UI shell

**Risk:** a technically cross-platform shell may feel non-native or require late architectural rework.

**Mitigation:** P1 UI shell spike before broad screen work for menu/tray/titlebar/window/theme/system-font behavior on both OSes.

## 5. Security and trust risks

### R-09 — Recovery trust bootstrap gap

**Risk:** encrypted backups alone do not restore trust after all trusted devices are lost.

**Mitigation:** Recovery Kit includes both backup-decryption material and offline trust-bootstrap signing capability.

### R-10 — Revocation ordering / clock skew

**Risk:** wall-clock ordering is unreliable and unsafe for post-revocation acceptance.

**Mitigation:** device sequence cutoff is security authority; clocks remain display/timeline data.

### R-11 — IPC authorization bypass

**Risk:** local reachability could be mistaken for authorization.

**Mitigation:** current-user ACLs plus explicit role/capability handshake and protocol negotiation.

### R-12 — Memory poisoning

**Risk:** untrusted content or AI output could become global/security policy.

**Mitigation:** authority tiers/protected namespaces; security/system namespaces daemon-authoritative.

### R-13 — Webview/Tauri privilege expansion

**Risk:** UI content could gain shell/filesystem privilege or unsafe rendering behavior.

**Mitigation:** least-privilege capabilities, sanitization/escaping and no arbitrary shell/unrestricted filesystem access from UI.

## 6. Privacy and AI risks

### R-14 — Raw-vault privacy bypass

**Risk:** external AI tools with arbitrary user-granted filesystem access are outside Cryptex classification enforcement.

**Mitigation:** supported adapters use controlled local MCP; Cryptex does not claim to control unrelated external filesystem access.

### R-15 — Restricted-memory leakage

**Risk:** restricted memory may cross AI boundaries unintentionally.

**Mitigation:** restricted memory excluded from AI by default; classification filters enforced by Cryptex.

### R-16 — Destructive AI authority

**Risk:** autonomous AI could purge memory or perform recovery/trust operations.

**Mitigation:** purge and recovery operations are never autonomous AI tools.

## 7. Data lifecycle and recovery risks

### R-17 — Purge/hash-chain conflict

**Risk:** removing content may break integrity history.

**Mitigation:** minimal content-free purge/integrity bridge where required.

### R-18 — Stale replica reintroduction

**Risk:** an offline stale peer could restore purged content.

**Mitigation:** purge wins semantically; M10 implements stale-peer suppression and reachable version/snapshot cleanup.

### R-19 — Deletion overclaim

**Risk:** product may promise deletion from systems Cryptex cannot control.

**Mitigation:** physical purge documented as best-effort across reachable Cryptex-controlled state; no guarantee for lost/inaccessible/provider-internal history.

### R-20 — Backup false confidence

**Risk:** successful upload/write may be mistaken for recoverability.

**Mitigation:** verification requires restore/reconstruction; M11 includes sandbox restore rehearsal.

### R-21 — Backup multi-writer pruning race

**Risk:** multiple devices independently applying retention may race/destructively prune.

**Mitigation:** one explicitly designated scheduled backup writer in V1; explicit role transfer.

## 8. Cross-platform and filesystem risks

### R-22 — Platform-specific canonical assumptions

**Risk:** path, case, line-ending, filename or symlink behavior may work on one OS only.

**Mitigation:** macOS/Windows first-class, UTF-8 LF canonical text, no canonical symlink dependency, platform adapters for OS integration.

### R-23 — Runtime/source-data mixing

**Risk:** personal/runtime state may enter source control or visible user folders.

**Mitigation:** dedicated app-data roots, Cryptex-Dev development roots, Git ignore/safety rules and no visible home/Desktop/Documents runtime folders by default.

## 9. Schema and correctness risks

### R-24 — Schema drift

**Risk:** Rust domain model and generated JSON Schema/TypeScript bindings may diverge.

**Mitigation:** Rust domain types generate bindings; CI detects drift once implemented.

### R-25 — Silent corruption/incompatibility

**Risk:** corrupt, unsigned/invalid or incompatible data may be silently accepted.

**Mitigation:** typed/structured failures, explicit integrity/schema handling and quarantine according to M2.

## 10. Operations and diagnostics risks

### R-26 — False health reporting

**Risk:** configured state may be shown as actually working.

**Mitigation:** Configured/Ready/Active/Verified/Attention remain distinct; Doctor uses observed state.

### R-27 — Diagnostic privacy leakage

**Risk:** logs or diagnostic bundles may expose memory/secrets/recovery material.

**Mitigation:** local/redacted logs by default, opt-in diagnostic bundle, no sensitive bodies/secrets/credentials by default.

## 11. Supply-chain and licensing risks

### R-28 — Dependency/supply-chain drift

**Risk:** unpinned or casually upgraded dependencies may reduce reproducibility/security.

**Mitigation:** exact toolchain/dependency baseline, lockfiles, later dependency/security auditing.

### R-29 — Syncthing third-party notices

**Risk:** managed packaging may miss applicable third-party licensing/notices obligations.

**Mitigation:** P0-4 includes licensing/notices proof; final release includes third-party notices.

## 12. Scope and process risks

### R-30 — Architecture drift through implementation convenience

**Risk:** coding agents or local fixes silently redesign accepted architecture.

**Mitigation:** Master Spec/ADRs/milestone authority hierarchy, repository-controlled agent rules and stop/report requirement on conflicts.

### R-31 — Later milestone work pulled forward

**Risk:** opportunistic implementation increases coupling/rework.

**Mitigation:** work only the selected milestone; later behavior remains explicitly out of scope until its contract.

### R-32 — Real personal data used too early

**Risk:** privacy exposure and legacy-data quirks steer architecture before recovery confidence exists.

**Mitigation:** Golden Brain/synthetic Dev Brain through early milestones; real onboarding only after recovery confidence.

## 13. Current open implementation-sensitive risks

The audit identifies these remaining implementation-sensitive unknowns as explicitly gated rather than hidden:

- physical event packaging
- daemon supervision/update mechanism
- exact local IPC implementation
- exact managed Syncthing packaging/supervision
- later feature-specific crypto/container details
- later Obsidian bridge details

These remain open until their required evidence/ADR acceptance occurs.

## 14. Review rule

This register must be updated when:

- a risk is resolved by accepted evidence
- a new material risk is discovered
- an ADR changes architecture
- a milestone introduces a new operational/security/recovery dependency

Resolved risk status must be supported by actual evidence, not configuration presence.

## 15. Source basis

This register consolidates risks and mitigations from:

- `docs/00_MASTER_SPEC.md`
- `AGENTS.md`
- `docs/18_IMPLEMENTATION_READINESS_PLAN.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`

It does not independently add architecture.
