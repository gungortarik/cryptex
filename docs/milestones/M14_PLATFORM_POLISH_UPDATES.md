# M14 — Platform Polish + Updates

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## Purpose

M14 productizes platform-specific polish, autostart/update coordination and packaging smoke after the shared product UI and M0 lifecycle architecture are established.

## Authority

Follow `AGENTS.md`, `docs/00_MASTER_SPEC.md`, accepted ADRs, `docs/11_DAEMON_IPC_RUNTIME.md`, `docs/14_DESKTOP_UX_AND_DESIGN_SYSTEM.md`, `docs/15_PLATFORM_MACOS_WINDOWS.md`, `docs/26_VERSIONING_MIGRATIONS_AND_UPDATES.md`, `docs/27_RELEASE_DISTRIBUTION_AND_SIGNING.md`, `docs/21_DEFINITION_OF_DONE.md` and this contract.

## Scope

M14 includes:

- macOS menu/window/background behavior
- Windows tray/window/background behavior
- autostart
- updater coordination
- DPI/Retina/theme/accessibility
- packaging smoke

## Required invariants

- Platform mechanisms may differ but product guarantees remain equivalent.
- Accepted M0 daemon/update architecture must not be silently redefined.
- Packaging/update behavior must not damage canonical data.
- Runtime data remains outside the source repository.
- No device becomes canonical master.

## Verification

M14 must verify platform lifecycle behavior, update/reconnect/rollback paths as applicable, DPI/Retina/theme/accessibility behavior and packaged-build smoke tests on macOS and Windows.

## Completion gate

M14 is complete only when scope items and relevant cross-platform/failure/UI tests pass, required CI is green, Definition of Done is satisfied and human review is complete.

Only then may M15 begin.
