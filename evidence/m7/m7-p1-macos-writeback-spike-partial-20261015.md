# M7 P1 Obsidian write-back spike — macOS partial result

**Date:** 2026-10-15
**Status:** Partial, owner-reported; P1 remains open
**Source checkout:** `codex/m5-capture-timeline` at
`87e9c4b0a49c5b670bdd7c6812d5a7c3fa768219`
**Relationship to current M6 product base:** source SHA is an ancestor of
`74bc3b14127962579ae03838bafea9336612a033`, but is not the same checkout tip.
**Host:** macOS 27.0.1 (26A434), arm64

## Scope and environment

The owner reports that the spike used a fresh synthetic vault only. No personal
vault was opened. Proof root:

`~/Library/Application Support/Cryptex-Dev/m7-p1-obsidian-038a331d135f423da4fe8a7473fea2f8`

The Obsidian version is unresolved: plugin plist reports `1.13.7`, while the
window title reports `1.14.4`. The plugin API types package/version and exact
commands were not included in the supplied result. Treat these as evidence gaps,
not as inferred values.

## Observations (owner-reported)

| Probe | Result | Limit |
|---|---|---|
| Manual editor conflict: `cachedRead()` → wait/edit → `Vault.process()` | **PASS** — current content was the manual edit; `manual_edit_remains=true`; stale plugin write absent | Synthetic note and reported run only; not a complete production conflict policy |
| Symlink boundary: `Vault.create()` through `Cryptex/Entities/…` | **Boundary risk observed** — write followed the symlink into `outside-symlink-target` | Obsidian Vault API did not itself enforce the managed physical boundary |
| Read-only ancestor metadata: `getBasePath` / `getFullPath` + `lstatSync` | Symlink detected before the Vault operation | Metadata check only; no race-free containment claim |
| `owner-note.md` sentinel | **PASS** — unchanged, reported SHA-256 prefix/suffix `bb9db33a…87a2276e` | Hash was supplied abbreviated, not independently recomputed here |

## Cleanup and decision

The owner reports that the plugin was disabled (`community-plugins.json=[]`),
the exact symlink and empty external directory were removed after path checks,
and the checkout's dirty tree was preserved. No production code, Actions,
commit, push, or merge was performed.

The macOS result agrees with the Windows synthetic observation that the Obsidian
Vault API follows a link below `Cryptex/`; a path-prefix check alone is not
physical containment. The `lstat` observation is useful detection evidence but
does not close the time-of-check/time-of-use gap. A safe, cross-platform
ownership-boundary policy remains unresolved. This report does not close P1 or
authorize M7 product implementation.
