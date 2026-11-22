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

## Local staged Desktop updates

The native `cryptex-updater` command runs separately from Desktop. It updates a closed per-user installation at `%LOCALAPPDATA%\Programs\Cryptex` on Windows or `~/Applications/Cryptex.app` on macOS. The updater works with complete, locally staged package images that support the Desktop update probe. Canonical data lives outside these directories.

The packaging step records the output of `cryptex-updater fingerprint PATH` for the trusted source image. The operator places the image beside the installation with the suffix `.pending` and runs `cryptex-updater apply APPROVED_SHA256`. The command checks the staged image against that independently supplied fingerprint. It probes the installed Desktop and its own daemon before changing the installation.

On a correlated protocol rejection the updater puts the rejected image back in the staging directory, restores the working installation, checks its fingerprint, probes its daemon again and launches Desktop at its usual path. On success the displaced image occupies the staging directory. The next package must be staged explicitly. Retrying after an interrupted selection recovers the recorded working image when both package fingerprints and their locations can be verified. An unknown image or a busy profile produces an error.

If the activated candidate exceeds its 30-second probe deadline, the updater terminates its own probe and daemon. In the same transaction it checks the recorded package fingerprints, verifies the retained working image, restores it at the installation path and reopens Desktop after a successful connection check. The candidate remains staged. If cleanup or verification fails, the transition journal stays available for recovery. A timeout during the initial check leaves the current package in place.

This flow covers a local package handover. Publisher signing, delivery channels and release distribution remain subject to the M17 gates.

## Completion gate

M14 is complete only when scope items and relevant cross-platform/failure/UI tests pass, required CI is green, Definition of Done is satisfied and human review is complete.

Only then may M15 begin.
