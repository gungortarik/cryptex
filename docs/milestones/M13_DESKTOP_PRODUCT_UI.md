# M13 — Desktop Product UI

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## Purpose

M13 productizes the broad Cryptex Desktop experience and finalized design system over already implemented core capabilities.

## Authority

Follow `AGENTS.md`, `docs/00_MASTER_SPEC.md`, `docs/14_DESKTOP_UX_AND_DESIGN_SYSTEM.md`, `docs/15_PLATFORM_MACOS_WINDOWS.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/21_DEFINITION_OF_DONE.md` and this contract.

## Prerequisite spike

Before broad screen work, the required P1 UI-shell/native-feeling spike must prove applicable menu/tray/titlebar/window, light/dark and system-font behavior on macOS and Windows.

## Scope

M13 includes:

- finalized design system
- Home/Health
- Memory/detail
- Global Search
- Timeline
- Devices
- AI/Integrations
- Backup/Recovery
- Doctor/Settings

## Required invariants

- UI must feel like a deliberate premium desktop application, not a generic AI dashboard.
- Runtime health must reflect actual observed state.
- Configured/Ready/Active/Verified/Attention remain distinct.
- Renderer/webview remains least privilege.
- Platform-native conventions are respected while preserving one Cryptex design language.

## Non-goals

M13 does not replace missing backend behavior with decorative/mock claims of functionality.

Platform polish/update packaging refinements continue in M14.

## Verification

M13 requires functional, visual/native-feeling, cross-platform, security and relevant accessibility/runtime-state review.

## Completion gate

M13 is complete only when prerequisite UI-shell evidence is resolved, product areas are implemented over real system behavior, required tests/reviews/CI pass, Definition of Done is satisfied and human review is complete.

Only then may M14 begin.
