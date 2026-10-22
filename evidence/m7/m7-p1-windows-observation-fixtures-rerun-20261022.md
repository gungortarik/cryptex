# M7 P1 Windows observation fixture rerun

**Date:** 2026-10-22\
**Checkout:** `codex/m8-device-trust` @ `03d447d2b64120f16597687eec9df7f5c7b8e41f`\
**Scope:** local Node syntax and observation-only fixture tests; no Obsidian UI,
daemon, vault, or managed-note access.

## Source identity

- `scripts/spikes/m7-obsidian-observation/main.js` SHA-256:
  `0f1c024f48120d955c2edd6a0415f0b5e6d3dd4beeb49d17217a82384f081f24`
- `scripts/spikes/m7-obsidian-observation/manifest.json` SHA-256:
  `89a514688e69dfcd7824644a439d27676ee563456c43e20ebf03e47418c2fd26`

These match the source hashes in the prior Windows and owner-reported macOS
observation reports.

## Commands and result

```text
node --check scripts/spikes/m7-obsidian-observation/main.js
node --test scripts/spikes/m7-obsidian-observation/guard.test.cjs scripts/spikes/m7-obsidian-observation/real-socket.test.cjs
```

- Syntax check: **PASS**
- Node tests: **18/18 PASS**, 0 failed, cancelled, skipped, or todo
- `git diff --check`: **PASS**

The fixtures cover the observation-only command inventory, synthetic-scope
guard, bounded status response handling, and local socket close/unload behavior.
They do not connect to a real daemon and do not establish Obsidian runtime
behavior, peer authorization, physical containment, or proposal authorization.

## Disposition

This rerun confirms the current local Windows fixture gate only. Existing
owner-reported native Mac health IPC remains a separate scoped result; Mac
native lifecycle and real-daemon pending cleanup are still not established.
Physical containment remains unresolved, managed-note I/O remains disabled,
and M7 is **not complete**. No product code, Actions, commit, push, or merge was
performed.
