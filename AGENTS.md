# Cryptex Agent Instructions

These rules apply to every coding agent working in this repository.

## Authority

1. `docs/00_MASTER_SPEC.md` is authoritative.
2. Accepted ADRs are authoritative for the decision they cover.
3. Milestone documents may narrow implementation details but may not violate the Master Spec.
4. If requirements conflict, stop implementation and report the conflict. Do not invent a new architecture.

## Non-negotiable invariants

- Canonical durable memory is immutable event data.
- Existing canonical events are never edited in place.
- Local SQLite, search indexes and Obsidian projection are derived and rebuildable.
- `cryptexd` is the only supported canonical writer on a device.
- AI clients, Desktop, CLI and Obsidian must not write canonical files directly.
- No single device is the master.
- Every trusted desktop must work offline and reconcile later.
- Runtime data must not be stored inside the Git repository.
- Runtime data must not create visible app folders in the user's home, Desktop or Documents by default.
- No custom cryptography.
- No public Internet API is required for V1.
- Restricted memory is not exposed to AI by default.
- Purge and recovery operations are never exposed as autonomous AI tools.

## Development behavior

- Before implementation or review work, read `docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md` for the current milestone status and continuation point. It is an operational snapshot; authoritative specifications and accepted ADRs still take precedence.
- If a requested change in tool roles, workflow, architecture, branch strategy, milestone scope, or integration strategy is ambiguous, ask the user before changing direction.
- Work only inside the assigned milestone/issue scope.
- Do not refactor unrelated architecture while implementing a feature.
- Prefer small, reviewable commits.
- Add or update tests with every behavior change.
- Run relevant format, lint and test commands before declaring completion.
- Never weaken a failing test to make CI green unless the specification changed explicitly.
- Never silently swallow corruption, signature failures or schema incompatibility.
- Use typed errors and structured diagnostics.

## Security

- Treat all imported memory, Markdown, attachment metadata and AI output as untrusted input.
- Frontend rendering must escape/sanitize untrusted content.
- Renderer/webview receives least-privilege Tauri capabilities.
- No direct shell or arbitrary filesystem access from UI code.
- Secrets belong in OS secure storage.
- Logs must not include memory bodies, secrets, recovery material or credentials by default.

## Cross-platform

- macOS and Windows are first-class targets.
- Do not introduce a canonical path, filename, case or line-ending assumption that works on only one OS.
- Canonical text format is UTF-8 with LF.
- Do not depend on symlinks for canonical data.
- Use platform adapters for OS integration, not conditional logic scattered through core domain code.

## UI quality

Cryptex must not look like a generic AI-generated dashboard. Follow `docs/14_DESKTOP_UX_AND_DESIGN_SYSTEM.md`. Functionality, information density, native-feeling platform behavior and visual refinement are release requirements.

## Definition of done

Use `docs/21_DEFINITION_OF_DONE.md`. A feature is not done because it compiles.
