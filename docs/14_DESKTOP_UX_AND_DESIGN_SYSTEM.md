# 14 — Cryptex Desktop UX and Design System

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the accepted product-quality and interaction boundaries for Cryptex Desktop.

Cryptex must feel like a deliberately designed premium desktop system utility and knowledge application, not a generic AI-generated web dashboard.

This document does not freeze final visual tokens or screen designs before the dedicated UI milestone and required UI shell spike.

## 2. Product areas

The planned Desktop product includes:

- Health
- Memory
- Timeline
- Search
- Devices
- AI/Integrations
- Backup/Recovery
- Doctor/Settings

The exact navigation structure and visual composition are finalized during the Desktop Product UI milestone.

## 3. Home and health

Home is health/dashboard-centered.

Runtime status shown in the UI must reflect actual evidence, not optimistic configuration state.

Cryptex distinguishes:

- Configured
- Ready
- Active
- Verified
- Attention

These states must not be collapsed merely for visual simplicity.

## 4. Global search

Global Search is accessible through a magnifier and platform-appropriate `Cmd/Ctrl+K` convention.

Core search remains useful offline without an LLM.

Search UI must not imply that semantic/vector search or AI is required for V1 retrieval.

## 5. Engine controls

Normal UI does not expose a prominent Start/Stop Engine control.

Accepted product behavior includes:

- Restart Engine available
- Stop Engine as an Advanced control
- Pause Sync as an explicit control
- Pause AI Access as an explicit control
- Maintenance Mode as an explicit control

The product should manage its runtime rather than requiring normal users to manually supervise processes.

## 6. Native-feeling platform behavior

Tauri provides the shared cross-platform webview shell.

For Cryptex, “native” means native-feeling conventions and polish rather than literal SwiftUI/WinUI widgets.

macOS and Windows may differ appropriately in:

- menus
- tray/menu-bar behavior
- window lifecycle
- keyboard conventions
- typography
- titlebar/window behavior
- theme integration
- high-DPI/Retina behavior

One Cryptex design language must remain recognizable across both platforms.

## 7. First-class platforms

macOS and Windows are first-class UI targets.

A screen that technically renders on both platforms is not automatically considered polished or platform-correct.

Where relevant, visual and interaction review must cover both operating systems.

## 8. UI shell spike

Before broad screen implementation, the required P1 UI shell/native-feeling spike must prove applicable:

- menu behavior
- tray/menu-bar behavior
- titlebar/window behavior
- light/dark behavior
- system-font approach

The final shell decisions must follow evidence from both platforms.

## 9. Least privilege

Desktop UI runs within the security boundaries defined by the project.

The renderer/webview must not gain:

- arbitrary shell access
- unrestricted filesystem access
- direct canonical-memory mutation capability

Privileged operations flow through narrowly scoped native/runtime boundaries.

## 10. Untrusted content rendering

Imported memory, Markdown, attachment metadata and AI output are untrusted input.

Frontend rendering must escape/sanitize untrusted content.

The visual layer must not create a path for content to acquire privileged capabilities.

## 11. Runtime truthfulness

Visual polish must not come at the cost of truthfulness.

Examples:

- configured is not Active
- Active is not automatically Verified
- failed health checks must not be visually represented as healthy
- unavailable checks must not silently become success

Doctor and Health surfaces must consume real runtime evidence.

## 12. Information density

Cryptex is a desktop utility/knowledge application, not a marketing site.

Screens should prioritize useful information density, clear hierarchy and efficient desktop interaction.

Large decorative empty areas, gratuitous cards and generic AI-dashboard patterns should not replace task-focused information architecture.

Exact layouts remain a UI milestone concern.

## 13. AI presentation

AI is one integration surface, not the organizing metaphor for the entire product.

The Desktop product must not visually reduce Cryptex to a chat interface.

AI features must coexist with Memory, Search, Timeline, Devices, Backup/Recovery and Doctor as first-class product areas.

## 14. Destructive actions

Destructive workflows such as Purge require explicit intent and must not be styled or placed like ordinary reversible edits.

The exact confirmation UI belongs to the applicable feature milestone.

AI must not autonomously invoke Purge or recovery operations.

## 15. Accessibility and platform polish

M14 Platform Polish + Updates includes:

- macOS menu/window/background behavior
- Windows tray/window/background behavior
- autostart
- updater coordination
- DPI/Retina/theme/accessibility
- packaging smoke

Release quality requires accessibility and display-density behavior to be treated as product requirements, not optional cleanup.

## 16. M0 boundary

M0 requires only the blank Desktop foundation and the UI necessary to prove its platform/runtime foundations.

M0 must not opportunistically build broad product screens.

Temporary developer UI may exist before M13 when needed to exercise core functionality.

## 17. M13 boundary

M13 Desktop Product UI includes the finalized design system and broad product screens.

Release UI quality is implemented there and refined afterward.

M13 must consume real underlying system behavior rather than mock health/state as if it were implemented.

## 18. Definition of Done

UI work is not complete because it renders.

Where UI changes are relevant, completion requires applicable:

- functional verification
- cross-platform verification
- security review
- runtime-state truthfulness
- visual/native-feeling review
- accessibility/platform checks where required

## 19. Explicitly unresolved

This document does not select:

- final color palette
- final spacing scale
- final type scale
- final icon set
- final window chrome implementation
- final navigation layout
- final per-screen composition
- final design-token schema

These belong to the UI shell spike, M13 and later accepted design decisions.

## 20. Source basis

This document consolidates existing Desktop/UX requirements from:

- `docs/00_MASTER_SPEC.md`
- `AGENTS.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`

It does not independently freeze the final visual design.
