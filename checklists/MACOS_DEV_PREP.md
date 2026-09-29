# macOS Development Preparation Checklist

**Status:** PROPOSED  
**Baseline:** 2026-09-28

## Purpose

This checklist verifies that a macOS development workstation is ready for Cryptex work without making that Mac a canonical/master device.

A checkbox should be marked complete only with actual local verification.

## Platform baseline

- [ ] Supported/up-to-date macOS is installed.
  - Evidence:
- [ ] System architecture is recorded and supported by the pinned toolchain.
  - Evidence:
- [ ] FileVault is enabled/recommended according to the environment policy.
  - Evidence:
- [ ] Xcode Command Line Tools are installed.
  - Evidence:

## Core development tools

- [ ] Git is installed and usable.
  - Evidence:
- [ ] GitHub CLI is installed and authenticated for the Cryptex repository.
  - Evidence:
- [ ] Cursor is installed.
  - Evidence:
- [ ] Rust is installed through rustup and the repository-pinned toolchain resolves.
  - Evidence:
- [ ] `rustfmt` is available.
  - Evidence:
- [ ] `clippy` is available.
  - Evidence:
- [ ] Repository-pinned Node version is available.
  - Evidence:
- [ ] Repository-pinned pnpm version is available.
  - Evidence:
- [ ] Obsidian is installed for relevant development/testing.
  - Evidence:
- [ ] Syncthing stable baseline is installed for integration testing.
  - Evidence:
- [ ] Homebrew, if used, remains a developer convenience and not a Cryptex runtime dependency.
  - Evidence:

## Repository and runtime separation

- [ ] Cryptex source repository is cloned locally.
  - Evidence:
- [ ] `origin` points to the intended private Cryptex GitHub repository.
  - Evidence:
- [ ] Repository source is separate from runtime/application data.
  - Evidence:
- [ ] `~/Library/Application Support/Cryptex-Dev/` exists or can be created and is writable.
  - Evidence:
- [ ] No real personal Brain, Cortex data, Recovery Kit, secrets or private runtime logs are stored in the source repository.
  - Evidence:

## Repository-controlled preflight

- [ ] `scripts/preflight-macos.sh` runs successfully.
  - Evidence:
- [ ] Required checks report no FAIL items.
  - Evidence:
- [ ] Any WARN item is understood and explicitly accepted before implementation depends on it.
  - Evidence:

## Cross-platform/toolchain consistency

- [ ] Rust version matches the repository baseline.
  - Evidence:
- [ ] Node version matches the repository baseline.
  - Evidence:
- [ ] pnpm version matches the repository baseline.
  - Evidence:
- [ ] Tauri/framework dependencies resolve from repository-controlled lockfiles.
  - Evidence:
- [ ] `git diff --check` is clean after local setup changes, if any.
  - Evidence:

## Completion gate

- [ ] macOS workstation is ready for the currently selected Cryptex milestone.
- [ ] No macOS-only assumption has been introduced into canonical architecture.
- [ ] This machine is not treated as a canonical master.

## Source basis

- `docs/18_IMPLEMENTATION_READINESS_PLAN.md`
- `docs/15_PLATFORM_MACOS_WINDOWS.md`
- `docs/19_REPOSITORY_CURSOR_AND_CI.md`
- `AGENTS.md`
