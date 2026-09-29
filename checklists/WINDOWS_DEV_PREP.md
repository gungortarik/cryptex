# Windows Development Preparation Checklist

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## Purpose

This checklist verifies that the Windows development workstation is ready for Cryptex work without making Windows a canonical/master environment.

A checkbox should be marked complete only with actual local verification.

## Platform baseline

- [ ] Supported/up-to-date Windows 11 is installed.
  - Evidence:
- [ ] System architecture is recorded and supported by the pinned toolchain.
  - Evidence:
- [ ] BitLocker or Device Encryption is enabled where available.
  - Evidence:
- [ ] Windows Defender remains enabled under normal development conditions.
  - Evidence:

## Core development tools

- [ ] Git for Windows is installed and usable.
  - Evidence:
- [ ] GitHub CLI is installed and authenticated for the Cryptex repository.
  - Evidence:
- [ ] Cursor is installed.
  - Evidence:
- [ ] Visual Studio Build Tools / Desktop development with C++ is installed.
  - Evidence:
- [ ] WebView2 runtime is installed.
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

## Repository and filesystem

- [ ] Cryptex source repository is cloned locally.
  - Evidence:
- [ ] `origin` points to the intended private Cryptex GitHub repository.
  - Evidence:
- [ ] Git long-path behavior is configured as required for Windows development.
  - Evidence:
- [ ] Repository source is separate from runtime/application data.
  - Evidence:
- [ ] `%LOCALAPPDATA%\Cryptex-Dev\` exists or can be created and is writable.
  - Evidence:
- [ ] No real personal Brain, Cortex data, Recovery Kit, secrets or private runtime logs are stored in the source repository.
  - Evidence:

## Repository-controlled preflight

- [ ] `scripts/preflight-windows.ps1` runs successfully.
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

- [ ] Windows workstation is ready for the currently selected Cryptex milestone.
- [ ] No Windows-only assumption has been introduced into canonical architecture.
- [ ] This machine is not treated as a canonical master.

## Source basis

- `docs/18_IMPLEMENTATION_READINESS_PLAN.md`
- `docs/15_PLATFORM_MACOS_WINDOWS.md`
- `docs/19_REPOSITORY_CURSOR_AND_CI.md`
- `AGENTS.md`
